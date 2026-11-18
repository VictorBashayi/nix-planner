#![windows_subsystem = "windows"]

mod due;
mod icon;
mod notify;
mod store;

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use slint::winit_030::{WinitWindowAccessor, winit};
use slint::{ComponentHandle, ModelRc, SharedString, Timer, TimerMode, VecModel};
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;
use due::{DAYS, MONTHS};
use store::{Bucket, History, State};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

slint::include_modules!();

struct App {
    state: RefCell<State>,
    history: RefCell<History>,
    path: PathBuf,
    save_timer: Timer,
    window: slint::Weak<AppWindow>,
    widget: slint::Weak<WidgetWindow>,
    visible: Cell<bool>,
    placed: Cell<bool>,
    /// Live search text; session only.
    query: RefCell<String>,
    /// The date at the last clock check, to spot midnight.
    day: Cell<Option<due::Date>>,
    /// How urgent each due task was at the last refresh, to spot labels going stale.
    urgencies: RefCell<Vec<(u64, due::Urgency)>>,
    /// The widget's size and inked height when its click-through shape was last set.
    shape: Cell<[i32; 3]>,
}

impl App {
    fn ui(&self) -> AppWindow {
        self.window.upgrade().expect("window outlives app state")
    }

    /// Pushes state into the UI and schedules a save.
    fn changed(self: &Rc<Self>) {
        self.state.borrow_mut().tidy();
        self.history.borrow_mut().record(&self.state.borrow().tasks);
        self.refresh();
        self.schedule_save();
    }

    /// Puts back the task list from undo (`redo` false) or redo history.
    fn step_history(self: &Rc<Self>, redo: bool) {
        let mut history = self.history.borrow_mut();
        let Some(tasks) = (if redo { history.redo() } else { history.undo() }) else { return };
        drop(history);
        self.state.borrow_mut().restore(tasks);
        self.ui().set_offer_undo(false);
        self.refresh();
        self.schedule_save();
    }

    /// Runs at launch and on the 5 s tick. On a new day, moves tasks whose date has come
    /// closer up a bucket. Redraws when the day changes or a task turns due soon or overdue,
    /// and not otherwise, since a redraw ends any edit in progress.
    fn check_clock(self: &Rc<Self>) {
        let (today, minute) = now_local();
        self.remind(today, minute);
        let new_day = self.day.replace(Some(today)) != Some(today);
        if new_day && self.state.borrow_mut().roll_over(today) {
            return self.changed();
        }
        if new_day || urgencies(&self.state.borrow(), today, minute) != *self.urgencies.borrow() {
            self.refresh();
        }
    }

    /// Shows a toast for each reminder that has come due, or one summary toast when
    /// several come due at once, as after the PC was off.
    fn remind(self: &Rc<Self>, today: due::Date, minute: u32) {
        let tasks = self.state.borrow_mut().take_reminders(today, minute);
        let (title, body, note, id) = match tasks.as_slice() {
            [] => return,
            [t] => {
                let d = t.due().expect("only tasks with a date remind");
                let when = if d.date == today {
                    format!("Due {}", d.time.map(due::time_name).unwrap_or_else(|| "today".into()))
                } else {
                    due::label(d, today, minute)
                };
                // Say which profile it is from when that isn't the one on show.
                let state = self.state.borrow();
                let when = if t.profile == state.profile { when } else { format!("{when} · {}", state.profile_name(t.profile)) };
                (t.text.clone(), when, t.desc.lines().next().unwrap_or_default().to_string(), Some(t.id))
            }
            _ => {
                let titles: Vec<&str> = tasks.iter().map(|t| t.text.as_str()).collect();
                (format!("{} tasks are due", tasks.len()), titles.join(", "), String::new(), None)
            }
        };
        self.schedule_save();
        let w = self.window.clone();
        notify::show(&title, &body, &note, id.is_some(), move |click| {
            w.upgrade_in_event_loop(move |ui| match (click, id) {
                (notify::Click::Done, Some(id)) => ui.invoke_complete_task(id as i32),
                _ => ui.invoke_show_requested(),
            })
            .ok();
        });
    }

    /// After the profile on show changes. Undo starts afresh, so it never changes
    /// tasks that are out of sight.
    fn switched(self: &Rc<Self>) {
        *self.history.borrow_mut() = History::new(&self.state.borrow().tasks);
        self.ui().set_offer_undo(false);
        self.refresh();
        self.schedule_save();
    }

    /// Keeps the widget a well-behaved part of the desktop; see `tend_widget`. Runs on
    /// every tick because the widget loses its owner when Explorer restarts, and hiding
    /// then showing it makes a new native window.
    fn tend(self: &Rc<Self>) {
        let Some(widget) = self.widget.upgrade() else { return };
        if !widget.window().is_visible() {
            return;
        }
        let Some(h) = widget.window().with_winit_window(hwnd).flatten() else { return };
        tend_widget(h);
        // Off every monitor after a display change: back to the default corner.
        widget.window().with_winit_window(|w| {
            if w.outer_position().is_ok_and(|p| !on_screen(w, p.x + 40, p.y + 10)) {
                place_widget(w, None);
            }
        });
        let scale = widget.window().scale_factor();
        let size = widget.window().size();
        let shape = [size.width as i32, size.height as i32, (widget.get_ink_height() * scale).ceil() as i32];
        if self.shape.replace(shape) != shape {
            shape_widget(h, shape, (24.0 * scale).ceil() as i32);
        }
    }

    fn schedule_save(self: &Rc<Self>) {
        let app = Rc::downgrade(self);
        self.save_timer.start(TimerMode::SingleShot, Duration::from_millis(300), move || {
            if let Some(app) = app.upgrade() {
                app.flush();
            }
        });
    }

    fn flush(&self) {
        self.save_timer.stop();
        if let Err(err) = store::save(&self.path, &self.state.borrow()) {
            eprintln!("could not save tasks: {err}");
        }
    }

    fn refresh(self: &Rc<Self>) {
        let ui = self.ui();
        ui.set_today(today().into());
        let state = self.state.borrow();
        let now = store::now_ms();
        let days = local_day(now).zip(local_day(now - 86_400_000));
        let (date, minute) = now_local();
        let query = self.query.borrow();
        let filtering = !query.is_empty();
        let buckets: Vec<BucketItem> = Bucket::ALL
            .into_iter()
            .filter_map(|b| {
                let tasks: Vec<TaskItem> = state
                    .tasks_in(b)
                    .filter(|t| t.matches(&query))
                    .map(|t| TaskItem {
                        id: t.id as i32,
                        title: t.text.as_str().into(),
                        desc: t.desc.as_str().into(),
                        done: t.completed,
                        when: match (t.completed_at.and_then(local_day), days) {
                            (Some(d), Some((today, yesterday))) if t.completed => {
                                completed_label(d, today, yesterday).into()
                            }
                            _ if !t.completed => {
                                t.due().map(|d| due::label(d, date, minute)).unwrap_or_default().into()
                            }
                            _ => SharedString::new(),
                        },
                        overdue: urgency(t, date, minute) == Some(due::Urgency::Overdue),
                        soon: urgency(t, date, minute) == Some(due::Urgency::Soon),
                    })
                    .collect();
                // While searching, buckets without matches are hidden and the rest shown open.
                if filtering && tasks.is_empty() {
                    return None;
                }
                Some(BucketItem {
                    key: b.key().into(),
                    title: bucket_title(b).into(),
                    collapsed: state.is_collapsed(b) && !filtering,
                    empty_text: empty_text(b).into(),
                    tasks: ModelRc::new(VecModel::from(tasks)),
                })
            })
            .collect();
        ui.set_buckets(ModelRc::new(VecModel::from(buckets)));
        ui.set_selected(match state.selected_bucket {
            Bucket::Soon => 1,
            Bucket::Later => 2,
            _ => 0,
        });
        ui.set_status(status_text(state.counts()).into());
        ui.set_has_done(state.counts().1 > 0);
        ui.set_profiles(ModelRc::new(VecModel::from(
            state
                .profiles
                .iter()
                .map(|p| ProfileItem {
                    id: p.id as i32,
                    name: p.name.as_str().into(),
                    open: state.tasks.iter().filter(|t| t.profile == p.id && !t.completed).count() as i32,
                    current: p.id == state.profile,
                })
                .collect::<Vec<_>>(),
        )));
        ui.set_profile_name(state.profile_name(state.profile).into());
        ui.set_profile_tasks(state.tasks.iter().filter(|t| t.profile == state.profile).count() as i32);

        // The widget shows today's open tasks, as many as fit its height.
        let w = self.widget.upgrade().expect("widget outlives app state");
        w.set_tasks(ModelRc::new(VecModel::from(
            state
                .tasks_in(Bucket::Now)
                .filter(|t| !t.completed)
                .map(|t| TaskItem {
                    id: t.id as i32,
                    title: t.text.as_str().into(),
                    when: t.due().map(|d| due::short_label(d, date, minute)).unwrap_or_default().into(),
                    overdue: urgency(t, date, minute) == Some(due::Urgency::Overdue),
                    soon: urgency(t, date, minute) == Some(due::Urgency::Soon),
                    ..Default::default()
                })
                .collect::<Vec<_>>(),
        )));
        w.set_today(today().into());
        w.set_profile(if state.profiles.len() > 1 { state.profile_name(state.profile).into() } else { SharedString::new() });
        *self.urgencies.borrow_mut() = urgencies(&state, date, minute);
        drop(state);
        // The rows just changed, and so has the part of the widget that takes clicks.
        self.tend();
    }

    fn show(self: &Rc<Self>) {
        let ui = self.ui();
        // Completion labels say "today"/"yesterday", so refresh them across midnight.
        self.refresh();
        if ui.show().is_err() {
            return;
        }
        self.visible.set(true);
        ui.window().with_winit_window(|w| {
            if !self.placed.replace(true) {
                center(w);
            }
            w.focus_window();
        });
        ui.invoke_focus_input();
    }

    fn hide(&self) {
        self.visible.set(false);
        self.ui().hide().ok();
    }

    fn toggle(self: &Rc<Self>) {
        if self.visible.get() { self.hide() } else { self.show() }
    }

    /// Stores the widget's place when it has been moved or resized.
    fn remember_widget(self: &Rc<Self>) {
        let Some(widget) = self.widget.upgrade() else { return };
        if !widget.window().is_visible() {
            return;
        }
        let place = widget
            .window()
            .with_winit_window(|w| {
                let (p, s) = (w.outer_position().ok()?, w.inner_size());
                Some([p.x, p.y, s.width as i32, s.height as i32])
            })
            .flatten();
        if place.is_some() && place != self.state.borrow().widget {
            self.state.borrow_mut().widget = place;
            self.schedule_save();
        }
    }
}

/// How urgent an open task's due date is; `None` for done tasks and tasks without one.
fn urgency(t: &store::Task, today: due::Date, minute: u32) -> Option<due::Urgency> {
    t.due().filter(|_| !t.completed).map(|d| d.urgency(today, minute))
}

fn urgencies(state: &State, today: due::Date, minute: u32) -> Vec<(u64, due::Urgency)> {
    state.tasks.iter().filter_map(|t| Some((t.id, urgency(t, today, minute)?))).collect()
}

fn bucket_title(b: Bucket) -> &'static str {
    match b {
        Bucket::Now => "Today",
        Bucket::Soon => "This week",
        Bucket::Later => "Someday",
        Bucket::Done => "Done",
    }
}

fn empty_text(b: Bucket) -> &'static str {
    match b {
        Bucket::Now => "Nothing for today yet.",
        Bucket::Soon => "Nothing planned this week.",
        Bucket::Later => "Ideas for someday go here.",
        Bucket::Done => "Finished tasks land here.",
    }
}

fn status_text((open, done): (usize, usize)) -> String {
    match (open, done) {
        (0, 0) => "No tasks yet".into(),
        (o, 0) => format!("{o} open"),
        (0, d) => format!("All done, {d} finished"),
        (o, d) => format!("{o} open, {d} done"),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Day {
    year: u16,
    month: u16,
    day: u16,
    weekday: u16,
}

/// The local calendar day of a Unix timestamp in milliseconds.
fn local_day(ms: i64) -> Option<Day> {
    use windows_sys::Win32::Foundation::{FILETIME, SYSTEMTIME};
    use windows_sys::Win32::System::Time::{FileTimeToSystemTime, SystemTimeToTzSpecificLocalTime};
    let ticks = ms.checked_add(11_644_473_600_000)?.checked_mul(10_000)? as u64;
    let ft = FILETIME { dwLowDateTime: ticks as u32, dwHighDateTime: (ticks >> 32) as u32 };
    // SAFETY: both calls only read and write the structs they are given.
    let t = unsafe {
        let mut utc: SYSTEMTIME = std::mem::zeroed();
        let mut local: SYSTEMTIME = std::mem::zeroed();
        if FileTimeToSystemTime(&ft, &mut utc) == 0
            || SystemTimeToTzSpecificLocalTime(std::ptr::null(), &utc, &mut local) == 0
        {
            return None;
        }
        local
    };
    Some(Day { year: t.wYear, month: t.wMonth, day: t.wDay, weekday: t.wDayOfWeek })
}

/// "Completed today", "Completed yesterday" or "Completed Mon 20 Oct", with the year
/// only when it isn't this year.
fn completed_label(done: Day, today: Day, yesterday: Day) -> String {
    if done == today {
        return "Completed today".into();
    }
    if done == yesterday {
        return "Completed yesterday".into();
    }
    let date = format!(
        "{} {} {}",
        &DAYS[done.weekday as usize % 7][..3],
        done.day,
        &MONTHS[(done.month as usize).saturating_sub(1) % 12][..3]
    );
    if done.year == today.year { format!("Completed {date}") } else { format!("Completed {date} {}", done.year) }
}

fn today() -> String {
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;
    // SAFETY: GetLocalTime only writes the struct it is given.
    let t = unsafe {
        let mut t = std::mem::zeroed();
        GetLocalTime(&mut t);
        t
    };
    format!(
        "{}, {} {}",
        DAYS[t.wDayOfWeek as usize % 7],
        t.wDay,
        MONTHS[(t.wMonth as usize).saturating_sub(1) % 12]
    )
}

/// Today's local date and the minutes since local midnight.
fn now_local() -> (due::Date, u32) {
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;
    // SAFETY: GetLocalTime only writes the struct it is given.
    let t = unsafe {
        let mut t = std::mem::zeroed();
        GetLocalTime(&mut t);
        t
    };
    let date = due::Date { year: t.wYear as i32, month: t.wMonth as u32, day: t.wDay as u32 };
    (date, t.wHour as u32 * 60 + t.wMinute as u32)
}

fn hwnd(w: &winit::window::Window) -> Option<windows_sys::Win32::Foundation::HWND> {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match w.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get() as _),
        _ => None,
    }
}

/// Makes the widget part of the desktop. Each step is skipped when already in place,
/// so this is cheap to repeat; winit rewrites the styles whenever it changes one.
/// - The desktop owns it, so Win+D leaves it showing.
/// - It is a tool window, so Alt+Tab and Task View don't list it.
/// - It has no minimise or maximise box, so dragging it to a screen edge or pressing
///   Win+arrow can't snap, maximise or minimise it.
/// - It is hidden from screen capture, so sampling the backdrop doesn't see its own text.
/// - Its frame is never painted (`quiet_frame`), so no title bar flashes up on click.
/// - It sits below every other window; clicking it would otherwise raise it over them.
fn tend_widget(hwnd: windows_sys::Win32::Foundation::HWND) {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let class: Vec<u16> = "Progman\0".encode_utf16().collect();
    // SAFETY: plain Win32 calls on a window this process owns; a null Progman is skipped.
    unsafe {
        let progman = FindWindowW(class.as_ptr(), std::ptr::null());
        if !progman.is_null() && GetWindow(hwnd, GW_OWNER) != progman {
            SetWindowLongPtrW(hwnd, GWLP_HWNDPARENT, progman as isize);
        }
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
        let wanted = style & !((WS_MINIMIZEBOX | WS_MAXIMIZEBOX) as isize);
        if style != wanted {
            SetWindowLongPtrW(hwnd, GWL_STYLE, wanted);
        }
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let wanted = ex & !(WS_EX_APPWINDOW as isize) | WS_EX_TOOLWINDOW as isize;
        if ex != wanted {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, wanted);
        }
        windows_sys::Win32::UI::Shell::SetWindowSubclass(hwnd, Some(quiet_frame), 1, 0);
        // Only when it changed: every call repaints the window, title bar included.
        let mut affinity = 0;
        if GetWindowDisplayAffinity(hwnd, &mut affinity) == 0 || affinity != WDA_EXCLUDEFROMCAPTURE {
            SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
        }
        SetWindowPos(hwnd, HWND_BOTTOM, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
    }
}

/// Screen position of the mouse in physical pixels.
fn cursor() -> (i32, i32) {
    let mut p = windows_sys::Win32::Foundation::POINT { x: 0, y: 0 };
    // SAFETY: GetCursorPos writes into the POINT we pass it.
    unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut p) };
    (p.x, p.y)
}

/// Stops Windows drawing a title bar over the widget when it gains or loses activation:
/// seen on click with the region set. Suppresses the repaint; the cause was not isolated.
unsafe extern "system" fn quiet_frame(
    hwnd: windows_sys::Win32::Foundation::HWND,
    msg: u32,
    wparam: usize,
    lparam: isize,
    _id: usize,
    _data: usize,
) -> isize {
    use windows_sys::Win32::UI::{Shell::DefSubclassProc, WindowsAndMessaging::{WM_NCACTIVATE, WM_NCPAINT}};
    // SAFETY: forwards the window's own message to the next handler in its subclass chain.
    unsafe {
        match msg {
            // 0xAE/0xAF: undocumented theme caption/frame draws. Slint re-sets the title on
            // every resize step, and each WM_SETTEXT draws the caption through these.
            WM_NCPAINT | 0xAE | 0xAF => 0,
            // lparam -1 = "don't repaint the frame"; the activation change itself still happens.
            WM_NCACTIVATE => DefSubclassProc(hwnd, msg, wparam, -1),
            _ => DefSubclassProc(hwnd, msg, wparam, lparam),
        }
    }
}

/// Cuts the widget's window down to the parts with something on them: the top `ink`
/// pixels (header and rows) and the resize grip. Clicks anywhere else in its rectangle
/// then reach the desktop icons underneath. `[width, height, ink]` in physical pixels.
fn shape_widget(hwnd: windows_sys::Win32::Foundation::HWND, [w, h, ink]: [i32; 3], grip: i32) {
    use windows_sys::Win32::Graphics::Gdi::*;
    // SAFETY: SetWindowRgn takes ownership of `shape`; the grip region is deleted here.
    unsafe {
        let shape = CreateRectRgn(0, 0, w, ink.min(h));
        let corner = CreateRectRgn(w - grip, h - grip, w, h);
        CombineRgn(shape, shape, corner, RGN_OR);
        DeleteObject(corner);
        if SetWindowRgn(hwnd, shape, 1) == 0 {
            DeleteObject(shape);
        }
    }
}

/// Puts the widget back where it was left, if that spot is still on a connected monitor;
/// otherwise top-right of the primary monitor.
fn place_widget(w: &winit::window::Window, saved: Option<[i32; 4]>) {
    if let Some([x, y, width, height]) = saved.filter(|&[x, y, ..]| on_screen(w, x + 40, y + 10)) {
        let _ = w.request_inner_size(winit::dpi::PhysicalSize::new(width as u32, height as u32));
        w.set_outer_position(winit::dpi::PhysicalPosition::new(x, y));
    } else if let Some(m) = w.primary_monitor() {
        let x = m.position().x + m.size().width as i32 - w.outer_size().width as i32 - 24;
        w.set_outer_position(winit::dpi::PhysicalPosition::new(x, m.position().y + 24));
    }
}

/// Average brightness (0..1) of what is on screen behind the widget, or `None` when
/// another window covers it and the reading would not be the wallpaper.
// shortcut: only one point in the header is checked for cover; check more if half-covered widgets misjudge.
fn backdrop_luma(hwnd: windows_sys::Win32::Foundation::HWND) -> Option<f32> {
    use windows_sys::Win32::Foundation::{POINT, RECT};
    use windows_sys::Win32::Graphics::Gdi::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindowRect, WindowFromPoint};
    const N: i32 = 8;
    // SAFETY: every GDI object created here is released before returning, and the DIB
    // pixel pointer is only read while its bitmap is alive.
    unsafe {
        let mut r: RECT = std::mem::zeroed();
        if GetWindowRect(hwnd, &mut r) == 0 {
            return None;
        }
        // The header is always part of the window's shape; the centre may be cut away.
        if WindowFromPoint(POINT { x: (r.left + r.right) / 2, y: r.top + 12 }) != hwnd {
            return None;
        }
        let screen = GetDC(std::ptr::null_mut());
        let mem = CreateCompatibleDC(screen);
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = N;
        info.bmiHeader.biHeight = -N;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        let mut bits = std::ptr::null_mut();
        let bmp = CreateDIBSection(mem, &info, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
        let mut luma = None;
        if !bmp.is_null() {
            let old = SelectObject(mem, bmp);
            SetStretchBltMode(mem, HALFTONE);
            let (w, h) = (r.right - r.left, r.bottom - r.top);
            if StretchBlt(mem, 0, 0, N, N, screen, r.left, r.top, w, h, SRCCOPY) != 0 {
                luma = Some(mean_luma(std::slice::from_raw_parts(bits as *const u32, (N * N) as usize)));
            }
            SelectObject(mem, old);
            DeleteObject(bmp);
        }
        DeleteDC(mem);
        ReleaseDC(std::ptr::null_mut(), screen);
        luma
    }
}

/// Mean perceived brightness of 0x00RRGGBB pixels, 0 (black) to 1 (white).
fn mean_luma(px: &[u32]) -> f32 {
    let sum: f32 = px
        .iter()
        .map(|p| {
            let (r, g, b) = ((p >> 16) & 0xff, (p >> 8) & 0xff, p & 0xff);
            0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32
        })
        .sum();
    sum / (px.len().max(1) as f32 * 255.0)
}

/// Whether the backdrop counts as dark. The gap between thresholds stops the text
/// flipping back and forth on mid-grey wallpapers.
fn is_dark(luma: f32, was_dark: bool) -> bool {
    if was_dark { luma < 0.55 } else { luma < 0.45 }
}

/// Whether a physical screen point lies on any connected monitor.
fn on_screen(w: &winit::window::Window, x: i32, y: i32) -> bool {
    w.available_monitors().any(|m| {
        let (p, s) = (m.position(), m.size());
        x >= p.x && y >= p.y && x < p.x + s.width as i32 && y < p.y + s.height as i32
    })
}

fn center(w: &winit::window::Window) {
    let Some(monitor) = w.current_monitor().or_else(|| w.primary_monitor()) else { return };
    let (m, ms, ws) = (monitor.position(), monitor.size(), w.outer_size());
    let x = m.x + (ms.width as i32 - ws.width as i32) / 2;
    let y = m.y + ((ms.height as i32 - ws.height as i32) / 2 - 40).max(40);
    w.set_outer_position(winit::dpi::PhysicalPosition::new(x, y));
}

/// Holds a named mutex for the life of the process; `None` if another copy runs.
fn single_instance() -> Option<isize> {
    use windows_sys::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
    use windows_sys::Win32::System::Threading::CreateMutexW;
    let name: Vec<u16> = "Local\\NixPlannerSingleInstance\0".encode_utf16().collect();
    // SAFETY: valid NUL-terminated name, default security attributes.
    let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
    let exists = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    (!handle.is_null() && !exists).then_some(handle as isize)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(_instance) = single_instance() else { return Ok(()) };

    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("software".into())
        .with_winit_window_attributes_hook(|attrs| {
            use winit::platform::windows::{CornerPreference, WindowAttributesExtWindows};
            let icon = winit::window::Icon::from_rgba(icon::rgba(), icon::SIZE, icon::SIZE).ok();
            attrs
                .with_window_icon(icon)
                .with_skip_taskbar(true)
                .with_undecorated_shadow(true)
                .with_corner_preference(CornerPreference::Round)
                .with_drag_and_drop(false)
        })
        .select()?;

    let ui = AppWindow::new()?;
    let widget = WidgetWindow::new()?;
    let path = store::data_path();
    let state = store::load(&path);
    if let Some(dir) = path.parent() {
        notify::register(dir);
    }
    let app = Rc::new(App {
        history: RefCell::new(History::new(&state.tasks)),
        state: RefCell::new(state),
        path,
        save_timer: Timer::default(),
        window: ui.as_weak(),
        widget: widget.as_weak(),
        visible: Cell::new(false),
        placed: Cell::new(false),
        query: RefCell::new(String::new()),
        day: Cell::new(None),
        urgencies: RefCell::new(Vec::new()),
        shape: Cell::new([0; 3]),
    });
    app.refresh();

    let on = |f: fn(&Rc<App>)| {
        let app = app.clone();
        move || f(&app)
    };
    {
        let a = app.clone();
        ui.on_add_task(move |text| {
            let (today, _) = now_local();
            let (title, due) = due::split(&text, today);
            let mut s = a.state.borrow_mut();
            let b = s.selected_bucket;
            if let Some(id) = s.add(title, b) {
                s.set_due(id, due, today);
            }
            drop(s);
            a.changed();
        });
    }
    ui.on_due_preview(|text| {
        let (today, minute) = now_local();
        let Some(d) = due::split(&text, today).1 else { return SharedString::new() };
        format!("{} → {}", due::label(d, today, minute), bucket_title(Bucket::for_due(d.date, today))).into()
    });
    {
        let a = app.clone();
        ui.on_select_bucket(move |i| {
            a.state.borrow_mut().selected_bucket = [Bucket::Now, Bucket::Soon, Bucket::Later][i.clamp(0, 2) as usize];
            a.refresh();
            a.schedule_save();
        });
    }
    {
        let a = app.clone();
        ui.on_toggle_task(move |id| {
            a.state.borrow_mut().toggle(id as u64);
            a.changed();
        });
    }
    {
        let a = app.clone();
        ui.on_complete_task(move |id| {
            a.state.borrow_mut().complete(id as u64);
            a.changed();
        });
    }
    {
        let a = app.clone();
        ui.on_delete_task(move |id| {
            a.state.borrow_mut().delete(id as u64);
            a.changed();
            a.ui().set_offer_undo(true);
        });
    }
    {
        let a = app.clone();
        ui.on_move_task(move |id, key, index| {
            if let Some(b) = Bucket::from_key(&key) {
                a.state.borrow_mut().move_to(id as u64, b, index.max(0) as usize);
                a.changed();
            }
        });
    }
    {
        // Live edits update state without rebuilding the list, so the editor keeps focus.
        let a = app.clone();
        ui.on_edit_live(move |id, is_desc, text| {
            let mut s = a.state.borrow_mut();
            if is_desc { s.set_desc(id as u64, &text) } else { s.set_text(id as u64, &text) }
            drop(s);
            a.schedule_save();
        });
    }
    ui.on_edit_finished(on(|a| a.changed()));
    {
        let a = app.clone();
        ui.on_edit_cancelled(move |id, title: SharedString, desc: SharedString| {
            let mut s = a.state.borrow_mut();
            s.set_text(id as u64, &title);
            s.set_desc(id as u64, &desc);
            drop(s);
            a.changed();
        });
    }
    {
        let a = app.clone();
        ui.on_toggle_collapsed(move |key| {
            if let Some(b) = Bucket::from_key(&key) {
                a.state.borrow_mut().toggle_collapsed(b);
                a.changed();
            }
        });
    }
    ui.on_clear_done(on(|a| {
        a.state.borrow_mut().clear_done();
        a.changed();
        a.ui().set_offer_undo(true);
    }));
    {
        let a = app.clone();
        ui.on_search(move |text| {
            *a.query.borrow_mut() = text.trim().to_string();
            a.refresh();
        });
    }
    {
        let a = app.clone();
        ui.on_select_profile(move |id| {
            a.state.borrow_mut().select_profile(id as u32);
            a.switched();
        });
    }
    ui.on_next_profile(on(|a| {
        a.state.borrow_mut().next_profile();
        a.switched();
    }));
    {
        let a = app.clone();
        ui.on_add_profile(move |name| {
            a.state.borrow_mut().add_profile(&name);
            a.switched();
        });
    }
    {
        let a = app.clone();
        ui.on_rename_profile(move |name| {
            a.state.borrow_mut().rename_profile(&name);
            a.refresh();
            a.schedule_save();
        });
    }
    ui.on_delete_profile(on(|a| {
        a.state.borrow_mut().delete_profile();
        a.switched();
    }));
    {
        let a = app.clone();
        ui.on_send_task(move |id, profile| {
            a.state.borrow_mut().send_to(id as u64, profile as u32);
            a.changed();
        });
    }
    ui.on_undo(on(|a| a.step_history(false)));
    ui.on_redo(on(|a| a.step_history(true)));
    ui.on_show_done(on(|a| {
        a.state.borrow_mut().expand(Bucket::Done);
        a.changed();
    }));
    ui.on_hide_window(on(|a| a.hide()));
    {
        let w = ui.as_weak();
        ui.on_start_move(move || {
            if let Some(ui) = w.upgrade() {
                ui.window().with_winit_window(|w| w.drag_window().ok());
            }
        });
    }
    {
        let w = ui.as_weak();
        ui.on_start_resize(move || {
            if let Some(ui) = w.upgrade() {
                ui.window().with_winit_window(|w| {
                    w.drag_resize_window(winit::window::ResizeDirection::SouthEast).ok()
                });
            }
        });
    }
    {
        let a = app.clone();
        widget.on_toggle_task(move |id| {
            a.state.borrow_mut().toggle(id as u64);
            a.changed();
        });
    }
    widget.on_open_app(on(|a| a.show()));
    {
        let w = widget.as_weak();
        widget.on_start_move(move || {
            if let Some(w) = w.upgrade() {
                w.window().with_winit_window(|w| w.drag_window().ok());
            }
        });
    }
    {
        let w = widget.as_weak();
        // Cursor and size when the grip was grabbed. Measured against the screen, since
        // the pointer position in the window shifts as the window changes size.
        let grab = Rc::new(Cell::new(((0, 0), (0, 0))));
        let g = grab.clone();
        let w2 = w.clone();
        widget.on_start_resize(move || {
            if let Some(w) = w2.upgrade() {
                let s = w.window().size();
                g.set((cursor(), (s.width as i32, s.height as i32)));
            }
        });
        widget.on_resize_drag(move || {
            if let Some(w) = w.upgrade() {
                let ((x0, y0), (w0, h0)) = grab.get();
                let (x, y) = cursor();
                let scale = w.window().scale_factor();
                let (min_w, min_h) = ((180.0 * scale) as i32, (90.0 * scale) as i32);
                w.window().set_size(slint::PhysicalSize::new((w0 + x - x0).max(min_w) as u32, (h0 + y - y0).max(min_h) as u32));
            }
        });
    }
    widget.on_reshape(on(|a| a.tend()));
    // Clicking the window's close affordances (Alt+F4) hides instead of quitting.
    {
        let a = app.clone();
        ui.window().on_close_requested(move || {
            a.hide();
            slint::CloseRequestResponse::KeepWindowShown
        });
    }

    // Tray icon and menu.
    let open_item = MenuItem::new("Open Nix Planner", true, None);
    let widget_item = MenuItem::new("Show or hide widget", true, None);
    let quit_item = MenuItem::new("Quit", true, None);
    let menu = Menu::new();
    menu.append_items(&[&open_item, &widget_item, &PredefinedMenuItem::separator(), &quit_item])?;
    let _tray = TrayIconBuilder::new()
        .with_icon(tray_icon::Icon::from_rgba(icon::rgba(), icon::SIZE, icon::SIZE)?)
        .with_tooltip("Nix Planner (Alt+P)")
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .build()?;

    // Global shortcut. If another app already owns Alt+P the tray still works.
    let hotkeys = GlobalHotKeyManager::new()?;
    let hotkey = HotKey::new(Some(Modifiers::ALT), Code::KeyP);
    let _ = hotkeys.register(hotkey);

    // These handlers run on the event loop's own thread; they forward to the
    // UI through the event loop queue, so nothing polls while idle.
    let w = ui.as_weak();
    GlobalHotKeyEvent::set_event_handler(Some(move |e: GlobalHotKeyEvent| {
        if e.id == hotkey.id() && e.state == HotKeyState::Pressed {
            w.upgrade_in_event_loop(|ui| ui.invoke_toggle_requested()).ok();
        }
    }));
    let w = ui.as_weak();
    TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
        if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
            w.upgrade_in_event_loop(|ui| ui.invoke_toggle_requested()).ok();
        }
    }));
    let w = ui.as_weak();
    let (open_id, quit_id, widget_id) = (open_item.id().clone(), quit_item.id().clone(), widget_item.id().clone());
    let wv = widget.as_weak();
    let wa = ui.as_weak();
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
        if e.id == open_id {
            w.upgrade_in_event_loop(|ui| ui.invoke_show_requested()).ok();
        } else if e.id == widget_id {
            let (wv, wa) = (wv.clone(), wa.clone());
            slint::invoke_from_event_loop(move || {
                if let Some(w) = wv.upgrade() {
                    if w.window().is_visible() { w.hide().ok(); } else { w.show().ok(); }
                }
                // Showing it makes a new native window, which has to be tended again.
                if let Some(ui) = wa.upgrade() {
                    ui.invoke_tend_requested();
                }
            })
            .ok();
        } else if e.id == quit_id {
            w.upgrade_in_event_loop(|ui| ui.invoke_quit_requested()).ok();
        }
    }));
    ui.on_toggle_requested(on(|a| a.toggle()));
    ui.on_show_requested(on(|a| a.show()));
    ui.on_tend_requested(on(|a| {
        a.shape.set([0; 3]);
        a.tend();
    }));
    ui.on_quit_requested(on(|a| {
        a.remember_widget();
        a.flush();
        slint::quit_event_loop().ok();
    }));

    // Slint only creates the native window once the event loop runs, so placing and
    // tending wait for it.
    widget.show()?;
    let saved = app.state.borrow().widget;
    let ww = widget.as_weak();
    let a = app.clone();
    slint::spawn_local(async move {
        let Some(widget) = ww.upgrade() else { return };
        let Ok(w) = widget.window().winit_window().await else { return };
        place_widget(&w, saved);
        a.tend();
    })?;

    // Re-check the wallpaper behind the widget now and then; it may move or the wallpaper change.
    let backdrop_timer = Timer::default();
    // The same tick saves the widget's place, so moves and resizes need no event hooks.
    let check_backdrop = {
        let w = widget.as_weak();
        let a = app.clone();
        move || {
            a.check_clock();
            a.tend();
            a.remember_widget();
            let Some(widget) = w.upgrade() else { return };
            if !widget.window().is_visible() {
                return;
            }
            if let Some(luma) = widget.window().with_winit_window(|w| hwnd(w).and_then(backdrop_luma)).flatten() {
                widget.set_dark_bg(is_dark(luma, widget.get_dark_bg()));
            }
        }
    };
    check_backdrop();
    backdrop_timer.start(TimerMode::Repeated, Duration::from_secs(5), check_backdrop);

    app.show();
    // `show` can't centre the window yet: the native window only exists once the loop runs.
    let a = app.clone();
    slint::spawn_local(async move {
        if let Ok(w) = a.ui().window().winit_window().await
            && !a.placed.replace(true)
        {
            center(&w);
        }
    })?;
    slint::run_event_loop_until_quit()?;
    app.flush();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(year: u16, month: u16, day: u16, weekday: u16) -> Day {
        Day { year, month, day, weekday }
    }

    #[test]
    fn completed_label_names_recent_days_and_dates_older_ones() {
        let today = day(2026, 10, 26, 1);
        let yesterday = day(2026, 10, 25, 0);
        assert_eq!(completed_label(today, today, yesterday), "Completed today");
        assert_eq!(completed_label(yesterday, today, yesterday), "Completed yesterday");
        assert_eq!(completed_label(day(2026, 10, 20, 2), today, yesterday), "Completed Tue 20 Oct");
        assert_eq!(completed_label(day(2025, 12, 31, 3), today, yesterday), "Completed Wed 31 Dec 2025");
    }

    #[test]
    fn backdrop_brightness_picks_text_tone() {
        assert_eq!(mean_luma(&[0x000000, 0xffffff]), 0.5);
        assert!(mean_luma(&[0x0000ff]) < 0.1, "pure blue reads as dark");
        assert!(is_dark(0.2, false) && !is_dark(0.8, true));
        // Between the thresholds the current tone is kept.
        assert!(is_dark(0.5, true) && !is_dark(0.5, false));
    }

    #[test]
    fn local_day_reads_a_timestamp() {
        // Noon UTC stays on the same calendar day in every time zone from UTC-11 to UTC+11.
        let d = local_day(1_793_016_000_000).unwrap(); // 2026-10-26 12:00 UTC
        assert_eq!((d.year, d.month, d.day), (2026, 10, 26));
    }
}
