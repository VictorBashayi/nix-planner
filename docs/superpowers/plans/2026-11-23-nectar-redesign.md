# Nix Planner NectarLink-style Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restyle Nix Planner in NectarLink's style: a labelled sidebar of profiles and bucket pages, 32 px+ targets, per-profile colour tinting every surface, light/dark following Windows, and easier note editing.

**Architecture:** `src/theme.rs` derives a token set from a profile's seed colour and the Windows mode; `main.rs` pushes it into a Slint `Theme` global on both windows. The store gains a per-profile `colour`, a `page` field, and by-id profile operations. The UI is split into `theme.slint`, `task_row.slint`, `sidebar.slint` and `app.slint`; the main window shows one bucket page at a time.

**Tech Stack:** Rust 2024, Slint 1.18 (software renderer, winit), serde_json, windows-sys.

**Spec:** `docs/superpowers/specs/2026-11-23-nectar-redesign-design.md` (style reference: `docs/nectar-style.md`).

## Global Constraints

- Build env (Git Bash): `export PATH="$HOME/.cargo/bin:/c/Users/victo/AppData/Local/Microsoft/WinGet/Packages/BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe/mingw64/bin:$PATH"`; test with `cargo test`, check with `cargo build` (debug). Release builds take ~20 min — only in Task 7.
- Work on branch `feature/nectar-redesign`. Commit messages: no `Co-Authored-By` trailer.
- Every clickable thing is at least 32 × 32 px (hit area).
- Data file `%APPDATA%\nix-planner\planner-tasks.json` keeps its shape; new fields are optional, and an older build reading the file must still behave correctly.
- No helper text or taglines in the UI; the footer hint "Alt+P shows and hides" is removed.
- Sentence case, no emoji. Young Serif only for the page title; Segoe UI Variable elsewhere.
- Contrast, every swatch, both modes: `text` and `text-muted` ≥ 4.5:1 on `card` and `canvas`; `accent` and fade `now/soon/later` ≥ 3:1 on `card` and `canvas`.
- Slint gotchas: no `wrap: word-wrap` in list rows; Flickable needs `max-height`; focus from a PopupWindow click needs a 30 ms deferred Timer; each top-level window (AppWindow, WidgetWindow) has its **own** instance of a global, so set Theme on both.
- Idle CPU stays ~0 (no new polling beyond the existing 5 s tick).

## Review Focus

1. A file saved while the Done page was open, then opened by an older build → `selectedBucket` is never `"done"`, so new tasks still go to an open bucket (Task 1 test `page_done_keeps_selected_bucket_open`).
2. Deleting a profile that isn't the current one (right-click) → only that profile and its tasks go; the current profile stays shown (Task 1 test `delete_other_profile_keeps_current`).
3. A profile file from before colours, then deleting the first profile → the others keep their colours, not reshuffled (Task 1 test `colours_assigned_once_and_kept`).
4. Windows switches light ↔ dark while the app is open or hidden → next 5 s tick re-themes both windows without restart (Task 3 manual check, step 6).
5. Search typed while on the Done page, then Esc → returns to the Done page, not Today (Task 4 manual check).

---

### Task 1: Store — profile colours, page field, by-id profile operations

**Files:**
- Modify: `src/store.rs` (Profile struct ~78, State ~85, profile fns ~145–203, `heal` ~205, tests)

**Interfaces:**
- Produces:
  - `Profile { id: u32, name: String, colour: Option<String> }` — serde field `colour`, `#[serde(default, skip_serializing_if = "Option::is_none")]`, hex `"#RRGGBB"`.
  - `State.page: Bucket` — serde `page`, default `Bucket::Now`.
  - `State::set_page(&mut self, b: Bucket)` — sets `page`; if `b != Done` also sets `selected_bucket = b`.
  - `State::rename_profile_id(&mut self, id: u32, name: &str)`, `State::delete_profile_id(&mut self, id: u32)`, `State::set_profile_colour(&mut self, id: u32, hex: &str)`.
  - `heal()` assigns `colour` to every profile without one: the first entry of `theme::SWATCHES` not used by another profile (wrapping to index `position % len` if all are used). Declare `pub const SWATCHES: [&str; 8]` in `src/theme.rs` now (values in Task 2) so the store can use it.
  - Existing `rename_profile(name)` / `delete_profile()` become thin wrappers over the `_id` versions with `self.profile`.

- [ ] **Step 0: Create the branch** `git checkout -b feature/nectar-redesign`.

- [ ] **Step 1: Write failing tests** in `store.rs` tests:

```rust
#[test]
fn page_done_keeps_selected_bucket_open() {
    let mut s = State::default();
    s.set_page(Bucket::Soon);
    assert_eq!((s.page, s.selected_bucket), (Bucket::Soon, Bucket::Soon));
    s.set_page(Bucket::Done);
    assert_eq!((s.page, s.selected_bucket), (Bucket::Done, Bucket::Soon));
    let json = serde_json::to_string(&s).unwrap();
    assert!(json.contains("\"page\":\"done\"") && json.contains("\"selectedBucket\":\"soon\""));
}

#[test]
fn delete_other_profile_keeps_current() {
    let mut s = State::default();           // profiles 0,1,2; current 0
    s.profile = 1; s.add("work task", Bucket::Now);
    s.profile = 0; s.add("home task", Bucket::Now);
    s.delete_profile_id(1);
    assert_eq!(s.profile, 0);
    assert_eq!(s.profiles.iter().map(|p| p.id).collect::<Vec<_>>(), vec![0, 2]);
    assert!(s.tasks.iter().all(|t| t.profile == 0));
}

#[test]
fn colours_assigned_once_and_kept() {
    let mut s: State = serde_json::from_str(r#"{"profiles":[{"id":0,"name":"A"},{"id":1,"name":"B"},{"id":2,"name":"C"}]}"#).unwrap();
    s.heal();
    let before: Vec<_> = s.profiles.iter().map(|p| p.colour.clone().unwrap()).collect();
    assert_eq!(before[0], crate::theme::SWATCHES[0]);
    s.delete_profile_id(0);
    s.heal();
    assert_eq!(s.profiles.iter().map(|p| p.colour.clone().unwrap()).collect::<Vec<_>>(), before[1..]);
}

#[test]
fn rename_and_colour_by_id() {
    let mut s = State::default();
    s.rename_profile_id(2, "  Uni  ");
    s.set_profile_colour(2, "#4F6FA8");
    let p = s.profiles.iter().find(|p| p.id == 2).unwrap();
    assert_eq!((p.name.as_str(), p.colour.as_deref()), ("Uni", Some("#4F6FA8")));
}
```

Also extend `save_then_load_round_trips` to assert `page` and `colour` survive. (`heal` is private; the tests live in the same module so they can call it.)

- [ ] **Step 2: Run `cargo test store::` — expect compile errors** (`set_page`, `page`, `colour`, `_id` fns, `theme` missing).

- [ ] **Step 3: Implement.** Add `mod theme;` in `main.rs` with `src/theme.rs` holding only `pub const SWATCHES: [&str; 8] = ["#B0703A", "#4F6FA8", "#4E8A5A", "#2F8A8A", "#7A5FA8", "#B0506A", "#5F6B7A", "#B07A1E"];` for now. `delete_profile_id` on the current profile shows the first remaining profile (existing behaviour); on another profile leaves `profile` unchanged; refuses when only one profile is left. `set_profile_colour` ignores unknown ids. Call `heal()` wherever it is called today (load) and after `add_profile` so new profiles get a colour.

- [ ] **Step 4: Run `cargo test` — all pass** (previous 31 + 4 new).

- [ ] **Step 5: Commit** `git add src/store.rs src/theme.rs src/main.rs && git commit -m "Store profile colours, the shown page, and by-id profile edits"`

---

### Task 2: Theme maths

**Files:**
- Modify: `src/theme.rs`

**Interfaces:**
- Consumes: `SWATCHES` (Task 1).
- Produces:
  - `pub struct Rgb(pub u8, pub u8, pub u8)` with `pub fn parse(hex: &str) -> Option<Rgb>`.
  - `pub struct Tokens { chrome, canvas, card, card_alt, accent_soft, accent, text, text_muted, hairline, hover, danger, now, soon, later, done: Rgb }`.
  - `pub fn tokens(seed: Rgb, dark: bool) -> Tokens`.
  - `pub fn contrast(a: &Rgb, b: &Rgb) -> f32` (WCAG 2 relative-luminance ratio).
  - `pub fn system_dark() -> bool` — reads `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\AppsUseLightTheme` (DWORD, 0 ⇒ dark) via `windows-sys` Registry (`RegGetValueW`); missing ⇒ `false`.

Derivation (HSL, keep the seed's hue `h`; `s`/`l` in %):

| Token | Light (s, l) | Dark (s, l) |
|---|---|---|
| chrome | 12, 93 | 8, 19 |
| canvas | 18, 95 | 10, 14 |
| card | 22, 98 | 8, 22 |
| card_alt | 20, 92 | 10, 17 |
| accent_soft | 55, 89 | 28, 28 |
| hairline | 18, 82 | 10, 31 |
| hover | 25, 99 | 8, 26 |
| text | 20, 12 | 10, 92 |
| text_muted | 12, 36 | 8, 70 |
| done (fade) | 6, 62 | 6, 45 |

`accent` = the seed; then step `l` down (light) / up (dark) by 2 until it reaches 3:1 on both `canvas` and `card`. `now` = `accent`; `soon` and `later` = `accent` mixed 25 % / 45 % towards `canvas`, then stepped back towards `accent` lightness until each meets 3:1 on `canvas` and `card`. `danger` fixed: light `#B4442F`, dark `#E07A62`.

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn every_swatch_is_readable_in_both_modes() {
    for hex in SWATCHES { for dark in [false, true] {
        let t = tokens(Rgb::parse(hex).unwrap(), dark);
        for bg in [&t.card, &t.canvas] {
            assert!(contrast(&t.text, bg) >= 4.5, "{hex} dark={dark} text");
            assert!(contrast(&t.text_muted, bg) >= 4.5, "{hex} dark={dark} muted");
            for (n, c) in [("accent", &t.accent), ("now", &t.now), ("soon", &t.soon), ("later", &t.later)] {
                assert!(contrast(c, bg) >= 3.0, "{hex} dark={dark} {n}");
            }
        }
    }}
}

#[test]
fn contrast_matches_wcag_reference() {
    let r = contrast(&Rgb(0, 0, 0), &Rgb(255, 255, 255));
    assert!((r - 21.0).abs() < 0.01);
}

#[test]
fn fade_steps_get_paler_towards_someday() {
    let t = tokens(Rgb::parse("#B0703A").unwrap(), false);
    let d = |c: &Rgb| contrast(c, &t.canvas);
    assert!(d(&t.now) >= d(&t.soon) && d(&t.soon) >= d(&t.later));
}

#[test]
fn parse_rejects_bad_hex() {
    assert!(Rgb::parse("#12345").is_none() && Rgb::parse("zzzzzz").is_none());
    assert!(Rgb::parse("#b0703a").is_some());
}
```

- [ ] **Step 2: Run `cargo test theme::` — expect FAIL** (fns missing).
- [ ] **Step 3: Implement** per the table. Add the `Win32_System_Registry` feature only if missing (it is already in `Cargo.toml`).
- [ ] **Step 4: Run `cargo test` — all pass.** If a swatch fails, adjust that swatch's hex in `SWATCHES` (not the rule).
- [ ] **Step 5: Commit** `git commit -am "Derive theme tokens from a seed colour with contrast checks"`

---

### Task 3: Split the UI and drive it from a Theme global

Same layout as today; only colours and control sizes change. This isolates the file split and token wiring from the layout rewrite.

**Files:**
- Create: `ui/theme.slint` — `export global Theme` (one `in-out property <color>` per `Tokens` field, kebab-case: `chrome, canvas, card, card-alt, accent-soft, accent, text, text-muted, hairline, hover, danger, now, soon, later, done`, plus `pure function strength(key: string) -> color` returning the fade for `"now"|"soon"|"later"|"done"`) and the shared small components moved from `app.slint`: `Icon`, `IconButton` (now 32 × 32), `Check` (drawn 20 px, see Task 6 for its gutter), `MenuRow` (min 32 px tall), `MenuLabel`, `Chip`.
- Create: `ui/task_row.slint` — `TaskRow` and `Editor` moved as-is.
- Modify: `ui/app.slint` — imports the above; re-exports `export { Theme } from "theme.slint";`; every `Palette.*` replaced by the matching `Theme.*` role (`porcelain→canvas`, `surface→card`, `espresso→text`, `coffee→accent`, `crema→accent-soft`, `foam→card-alt`, `muted→text-muted`, `faint→text-muted`, `hairline→hairline`, `hover→hover`, `danger→danger`); `Palette` global deleted.
- Modify: `src/main.rs` — `fn apply_theme(&self)` on the app state: builds `theme::tokens(seed_of_current_profile, self.dark.get())` and sets every Theme property on **both** `ui.global::<Theme>()` and `widget.global::<Theme>()`. Called from `refresh()` (cheap) and when `dark` changes. `dark: Cell<bool>` initialised from `theme::system_dark()`; the existing 5 s tick (`on_tend_requested`) re-reads it and calls `apply_theme` on change.

**Interfaces:**
- Consumes: `theme::{tokens, Rgb, system_dark}`, `Profile.colour` (Tasks 1–2).
- Produces: Slint `Theme` global (names above) used by Tasks 4–7; `App::apply_theme`.

- [ ] **Step 2: Move components and swap Palette → Theme.** `cargo build` — expect success, no Slint warnings about unknown `Palette`.
- [ ] **Step 3: Add `apply_theme` and the dark-mode tick.** `cargo build && cargo test` — pass.
- [ ] **Step 4: Run the debug build** (`target/debug/nix-planner.exe`, after quitting the installed one from the tray) and screenshot with `Alt+P`. Expected: same layout, coffee-tinted surfaces, header buttons visibly larger.
- [ ] **Step 5: Switch profile with Ctrl+Tab.** Expected: whole window re-tints to that profile's swatch.
- [ ] **Step 6: Toggle Windows Settings → Personalization → Colors → app mode to Dark.** Expected within ~5 s: both main window and widget switch to dark tokens; switch back restores light.
- [ ] **Step 7: Commit** `git add ui src && git commit -m "Split the UI files and colour it from a per-profile Theme"`

---

### Task 4: Sidebar and one-bucket pages

**Files:**
- Create: `ui/sidebar.slint` — `export component Sidebar` (width 150 px, `Theme.chrome` background).
- Modify: `ui/app.slint` — AppWindow layout, page model, search view.
- Modify: `src/main.rs` — `refresh()` builds the page instead of all buckets; new callbacks.

**Interfaces:**
- Consumes: `State::set_page`, `State.page` (Task 1), `Theme` (Task 3).
- Produces:
  - Slint structs: `SideBucket { key: string, title: string, count: int, current: bool }`; `ProfileItem` gains `colour: color` (keeps `id, name, open, current`).
  - AppWindow properties: `in property <[SideBucket]> side-buckets`, `in property <string> page` (bucket key), `in property <[BucketItem]> buckets` (now: one item = the current page; while searching, one item per bucket with matches).
  - Callback `select-page(string)` (replaces `select-bucket(int)`), Rust handler calls `set_page`, clears search, `refresh()`.
  - Sidebar component callbacks (wired through AppWindow): `select-profile(int)`, `select-page(string)`, `add-profile-requested()`, `profile-menu-requested(int, length, length)` (id + pointer position), and drop targets for Task 5.

Layout (from spec):
- AppWindow `preferred-width: 540px; min-width: 460px; preferred-height: 640px`.
- Sidebar: top 52 px app name "Nix Planner" (drag area), profile rows (36 px pills: 10 px colour dot + name; current = `accent-soft`), "+" row (36 px), 1 px `hairline` gap, bucket rows (36 px: 18 px line icon in that bucket's `Theme.strength(key)` + title + right-aligned count; current = `accent-soft`). Counts: open tasks for Today/This week/Someday, completed for Done.
- Main area on `Theme.canvas` with 16 px top-left radius: title bar 52 px (page title in Young Serif, 20 px, coloured `Theme.strength(page)`; right: search, pin, hide IconButtons); add box (hidden on Done); one group card (`Theme.card`, 16 px radius) with rows separated by inset 1 px `hairline`.
- Removed: bucket chips under the add box, collapse arrows and section headings on pages, the footer (status text and "Alt+P shows and hides"), the profile switcher in the title bar.
- Done page header: "Clear" two-click button (existing behaviour) at the right of the title row.
- Empty page: centred 32 px bucket icon + one line: Today "Nothing for today", This week "Nothing this week", Someday "Nothing for someday", Done "Nothing done yet". (Replace `empty_text`.)
- Search: field replaces the add box; results grouped by bucket with small headings in `Theme.strength(key)`; Esc closes and returns to `state.page`.
- Keys: `Ctrl+1/2/3/4` → `select-page("now"|"soon"|"later"|"done")`; `Ctrl+Tab` cycles profiles; `Ctrl+F`, `Esc`, undo/redo as today. Add box adds to `state.page` (never Done).

- [ ] **Step 1: Write failing test** in `main.rs` tests for a pure helper `fn side_counts(state: &State) -> [usize; 4]` (open now, open soon, open later, done — current profile only):

```rust
#[test]
fn sidebar_counts_follow_the_current_profile() {
    let mut s = State::default();
    s.add("a", Bucket::Now); s.add("b", Bucket::Later);
    let id = s.add("c", Bucket::Soon).unwrap(); s.toggle(id);
    s.profile = 1; s.add("other", Bucket::Now); s.profile = 0;
    assert_eq!(side_counts(&s), [1, 0, 1, 1]);
}
```

- [ ] **Step 2: `cargo test sidebar_counts` — FAIL** (missing fn).
- [ ] **Step 3: Implement `side_counts`, the Sidebar component, the page layout and the Rust wiring above.** Remove `toggle-collapsed`/`select-bucket` callbacks and their handlers; keep `State::collapsed` and its store tests untouched.
- [ ] **Step 4: `cargo test && cargo build` — pass.**
- [ ] **Step 5: Manual check (debug build, screenshots):** window ~540 wide; clicking each sidebar bucket shows only its tasks; adding on This week lands in This week; Done shows Clear and no add box; empty Someday shows its empty state; `Ctrl+4` opens Done; search on Done then Esc returns to Done (Review Focus 5); counts match.
- [ ] **Step 6: Commit** `git commit -am "Add the sidebar and show one bucket per page"`

---

### Task 5: Sidebar actions — profile menu, colours, drag onto sidebar

**Files:**
- Modify: `ui/sidebar.slint`, `ui/app.slint`, `src/main.rs`

**Interfaces:**
- Consumes: `rename_profile_id`, `delete_profile_id`, `set_profile_colour`, `send_to`, `move_to` (Task 1 / existing), `theme::SWATCHES`.
- Produces: AppWindow callbacks `rename-profile-id(int, string)`, `delete-profile-id(int)`, `set-profile-colour(int, string)`, `drop-on-page(int /*task id*/, string /*bucket key*/)`, `drop-on-profile(int, int)`; property `in property <[color]> swatches` and `in property <[string]> swatch-hex` (same order).

Behaviour:
- "+" row → inline name box in the sidebar (Enter adds via existing `add-profile`, Esc cancels).
- Right-click a profile row (TouchArea `pointer-event`, `PointerEventButton.right`, kind `down`) → PopupWindow at the pointer: "Rename" (row turns into an inline name box, Enter → `rename-profile-id`), a row of 8 swatch circles (28 px drawn, 32 px hit; current one ringed) → `set-profile-colour`, "Delete" → becomes "Delete N tasks?" in place; a second click within 5 s → `delete-profile-id`. Delete hidden when only one profile exists. Use the 30 ms deferred focus for inline boxes.
- Dragging a task (existing drag code) over a sidebar bucket row highlights it with `accent-soft`; drop → `drop-on-page` → `move_to(id, bucket, 0)` (dropping on Done completes, dropping a done task on an open bucket reopens — existing `move_to` semantics). Over a profile row → `drop-on-profile` → `send_to(id, profile)`. Each drop calls `changed()` so undo works.
- Remove the profile PopupWindow from the old title-bar switcher and the footer delete confirmation.

- [ ] **Step 1: Write failing test** (store-level, covers drop semantics used by `drop-on-page` from Done):

```rust
#[test]
fn dropping_a_done_task_on_someday_reopens_it_on_top() {
    let mut s = State::default();
    s.add("x", Bucket::Later);
    let id = s.add("y", Bucket::Now).unwrap(); s.toggle(id);
    s.move_to(id, Bucket::Later, 0);
    let t = s.tasks.iter().find(|t| t.id == id).unwrap();
    assert!(!t.completed && t.completed_at.is_none());
    assert_eq!(s.tasks_in(Bucket::Later).next().unwrap().id, id);
}
```

- [ ] **Step 2: `cargo test dropping_a_done` — expected PASS already** (existing `move_to`); if it fails, fix `move_to` first. This pins the behaviour the sidebar relies on.
- [ ] **Step 3: Implement the menu, swatches, inline boxes and drop targets.**
- [ ] **Step 4: `cargo test && cargo build` — pass.**
- [ ] **Step 5: Manual check:** right-click Work → Colour → green: window re-tints when Work is shown; right-click School while on Personal → Delete → confirm: Personal still shown (Review Focus 2); drag a Today task onto This week and onto Work; Ctrl+Z undoes each.
- [ ] **Step 6: Commit** `git commit -am "Add profile menu, colour swatches and sidebar drop targets"`

---

### Task 6: Task row — sizes, notes, cursor

**Files:**
- Modify: `ui/task_row.slint`, `ui/theme.slint` (`Check`), `src/main.rs` (only if a new callback is needed)

**Interfaces:**
- Consumes: `Theme`, existing TaskRow callbacks (`toggled`, `edit-live`, `edit-cancelled`, etc.).
- Produces: TaskRow callback `add-note-requested()` handled inside TaskRow (sets `editing-desc = true` and focuses), no Rust change.

Behaviour:
- Row min height 44 px (52 px with a note line); padding 12 px horizontal.
- Left gutter 40 px wide is one TouchArea that toggles the task; `Check` drawn at 20 px, ring in `Theme.strength(bucket-key)`, filled when done.
- Hover: `⋯` and `×` as 32 px IconButtons; row background `Theme.hover`.
- Row menu: "Add note" entry when `task.desc == ""` (above "Move to"); "Move to" stays; "Send to" stays in the menu as well (spec: builder's call — cheap, keeps a non-drag path for accessibility).
- Cursor: after `title-editor.focus()` / `desc-editor.focus()` call `set-selection-offsets(n, n)` where `n` is the byte length of the text (Slint selection offsets are UTF-8 byte offsets). If Slint 1.18 exposes no way to get the byte length in .slint, pass it from Rust as a `TaskItem` field (`title-len: int`, `desc-len: int`) set with `str::len()`. Click-position mapping is not required.

- [ ] **Step 1: Implement sizes, gutter, menu entry and cursor placement.**
- [ ] **Step 2: `cargo build` — pass.**
- [ ] **Step 3: Manual check:** click a long title → caret at end, typing appends; click a note → caret at end; task without a note → `⋯` → "Add note" opens an empty note line focused; title with an emoji or accented letter → caret still at end, no panic; clicking anywhere in the 40 px gutter toggles.
- [ ] **Step 4: Commit** `git commit -am "Roomier task rows, Add note, caret at end when editing"`

---

### Task 7: Widget, docs, release

**Files:**
- Modify: `ui/app.slint` (WidgetWindow), `docs/design.md`, `README.md` (screenshot/wording only if it describes the old look)

**Interfaces:**
- Consumes: `Theme` on the widget instance (set by `apply_theme`, Task 3).

- [ ] **Step 1: Widget:** checkbox/date ink from `Theme.now`/`Theme.accent` on light backdrops; on dark backdrops keep the existing light ink (`#E8C9A6`-style) but derive it as `Theme.accent-soft` of the light token set so it follows the profile colour. Checkbox drawn 20 px with a 32 px hit area. Content and window behaviour unchanged.
- [ ] **Step 2: `cargo build`; manual check:** widget follows profile colour; Win+D keeps it; dark wallpaper still readable.
- [ ] **Step 3: Update `docs/design.md`:** replace the "Look" section with a pointer to `docs/nectar-style.md` plus the per-profile colour and light/dark rules; replace "Buckets collapse; Done starts collapsed" and the chips/footer lines with the sidebar + pages behaviour; add `page` and `profiles[].colour` to the Data section.
- [ ] **Step 4: Release build** `cargo build --release` (~20 min), quit the installed app from the tray, copy `target/release/nix-planner.exe` to `%LOCALAPPDATA%\Programs\Nix Planner\`, relaunch with `-WorkingDirectory` set to that folder. Check RAM (Task Manager working set ≈ 30–40 MB) and idle CPU ≈ 0.
- [ ] **Step 5: Commit** `git add -A && git commit -m "Theme the widget and document the redesign"` (include `docs/nectar-style.md` and the spec if not yet committed).
