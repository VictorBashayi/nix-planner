# Nix Planner (Rust) — design

Native rewrite of the Electron planner in `~/planner-app`. Goal: same workflow, far lower
RAM/CPU, a calmer and less generic look, one `.exe` with no Node/npx.

## Behaviour (carried over)
- Frameless window, hidden to tray on close. `Alt+P` toggles it; tray left-click toggles,
  tray menu has Open / Quit.
- Buckets: Today (`now`), This week (`soon`), Someday (`later`), Done (`done`).
- A sidebar lists the profiles, then the buckets with counts. Each bucket is a page;
  `Ctrl+1/2/3/4` open Today / This week / Someday / Done. The add box adds to the page
  on show (Done has none). `Esc` hides the window.
- Each task: checkbox (the whole left gutter checks it), title, optional note, menu
  (Add note, Move to, Send to), delete. Click the title or note to edit with the caret at
  the end; `Enter` commits, `Esc` reverts, empty title deletes.
- Checking a task sends it to Done and remembers its bucket; unchecking restores it.
- Done is its own page with a two-click Clear; deletes and clears offer Undo for 6 s.
- Pin button toggles always-on-top (on by default).
- Drag a task by press-and-move: a ghost follows the pointer. Dropping in the list
  reorders; dropping on a sidebar bucket moves it there, on a profile sends it there.

## Data
Same file and JSON shape as the Electron app, so tasks carry over and either app can read it:
`%APPDATA%\nix-planner\planner-tasks.json`
(`tasks[] {id,text,desc,bucket,completed,createdAt,completedAt?,previousBucket?,due?,reminded?}`,
see `due-dates.md` for the last two),
`selectedBucket`, `page`, `collapsed`, `nextId`, `widget`, `profiles[] {id,name,colour?}`, `profile`).
`page` is the bucket page on show; `selectedBucket` mirrors it for open buckets only, so an
older build never adds tasks into Done. `collapsed` is no longer used but kept for older builds. Each
task has a `profile` id; files from before profiles load with everything in profile 0,
"Personal". Saves are debounced (300 ms) and atomic
(write temp file, rename).

## Profiles
- One profile is on show at a time; buckets, search, counts, Clear and the widget all work
  on that profile only. Reminders and the midnight roll-over cover every profile.
- Profiles are switched in the sidebar (`Ctrl+Tab` cycles). "New profile" opens a name box
  there. Right-click a profile for Rename, its colour, and Delete (two clicks within 5 s).
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
NectarLink-inspired; full style note in `nectar-style.md`.
- Tonal surfaces, no hairline-heavy look; pills and 32 px+ targets everywhere.
- Every colour is derived from the current profile's seed (`src/theme.rs`), in light or dark
  following Windows' app mode. Swatches are tested for contrast (text 4.5:1, accents 3:1).
- The bucket fade (Today strongest → Someday palest, Done grey) colours the sidebar icons,
  page title and checkboxes.
- Young Serif (OFL, embedded) for the app name and page titles; Segoe UI Variable elsewhere.
- Sentence case, no emoji, no helper text.
