# Quick Planner (Rust) — design

Native rewrite of the Electron planner in `~/planner-app`. Goal: same workflow, far lower
RAM/CPU, a calmer and less generic look, one `.exe` with no Node/npx.

## Behaviour (carried over)
- Frameless window, hidden to tray on close. `Alt+P` toggles it; tray left-click toggles,
  tray menu has Open / Quit.
- Buckets: Today (`now`), This week (`soon`), Someday (`later`), Done (`done`).
- Quick-add box at the top; target bucket picked with the chips or `Ctrl+1/2/3`.
  `Esc` in the box hides the window.
- Each task: checkbox, title, optional description, move menu, delete.
  Click the title/description to edit; `Enter` commits, `Esc` reverts, empty title deletes.
- Checking a task sends it to Done and remembers its bucket; unchecking restores it.
- Buckets collapse; Done starts collapsed.
- Pin button toggles always-on-top (on by default).
- Drag a task by press-and-move: a ghost follows the pointer, the bucket under it
  highlights, and the drop inserts the task at the hovered position (reorder works too).

## Data
Same file and JSON shape as the Electron app, so tasks carry over and either app can read it:
`%APPDATA%\quick-planner\planner-tasks.json`
(`tasks[] {id,text,desc,bucket,completed,createdAt,completedAt?,previousBucket?,due?,reminded?}`,
see `due-dates.md` for the last two),
`selectedBucket`, `collapsed`, `nextId`, `widget`, `profiles[] {id,name}`, `profile`). Each
task has a `profile` id; files from before profiles load with everything in profile 0,
"Personal". Saves are debounced (300 ms) and atomic
(write temp file, rename).

## Profiles
- One profile is on show at a time; buckets, search, counts, Clear and the widget all work
  on that profile only. Reminders and the midnight roll-over cover every profile.
- The switcher is the profile name in the title bar. New and Rename turn the name into a
  text box in place. Delete is two clicks: the menu, then the footer's "Delete X and its N
  tasks?" within 5 seconds.
- Undo history restarts on a switch, so Ctrl+Z never changes tasks that are out of sight.

## Desktop widget
A second window that `tend_widget` keeps in line on every 5 s tick, since winit rewrites
window styles and the desktop goes away when Explorer restarts: owned by Progman (survives
Win+D), tool window (no Alt+Tab), no minimise/maximise box (no snap), bottom of the
z-order, excluded from capture. `shape_widget` sets a window region covering the header,
rows and grip, so the empty part of its rectangle passes clicks to the desktop.

## Tech
- Rust + Slint, software renderer, winit backend. No GPU context, no web engine.
- `tray-icon` + `global-hotkey` deliver events through callbacks that hop onto the Slint
  event loop — nothing polls, so idle CPU is ~0.
- Windows 11 rounded corners via DWM; single instance via a named mutex.
- Release build: LTO, 1 codegen unit, stripped, `windows_subsystem = "windows"`.

## Look
Calm, minimal, "coffee in a porcelain cup".
- Porcelain `#FAF9F7` background, espresso `#2E221B` text, coffee `#B0703A` accent,
  crema `#EADBC8` hairlines/selection, muted `#8C8079` secondary text.
- Bucket colour weakens with time distance: Today full coffee, This week medium,
  Someday pale latte, Done grey. That gradient is the one expressive element.
- Young Serif (OFL, embedded) for bucket names only; Segoe UI Variable for everything else.
- Sentence case, no emoji, no all-caps labels.
