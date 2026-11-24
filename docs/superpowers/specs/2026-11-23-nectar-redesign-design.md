# Nix Planner redesign — NectarLink style

Date: 2026-11-23. Status: draft for approval.
Style reference: `docs/nectar-style.md` (tokens, shapes, components). Current behaviour: `docs/design.md`.

## Intent

The owner finds Nix Planner hard to use comfortably: controls are too small to click easily
(18 px checkbox, 24–28 px buttons and menu rows), and some actions are awkward or hidden. They
admire NectarLink's Windows app and want its style — tinted layers instead of lines, pills,
icon chips, roomy targets, a sidebar — brought to Nix Planner. Nix Planner is the first app to
get this house style; later projects (FullLock GUI and others) will copy what we settle here.

Success: everything is easy to hit, switching profile or bucket is one big click in a sidebar,
editing a note feels natural, and the app looks as calm and coherent as NectarLink.

## Goals

1. Every clickable thing is at least 32 × 32 px; rows are generous.
2. A labelled sidebar holds the profiles and the buckets; each bucket is a page.
3. Each profile has its own colour that tints the whole window; light and dark mode follow Windows.
4. Notes are easy to add and edit; the text cursor lands where expected.
5. Visual language from `docs/nectar-style.md`: tonal surfaces, no hairline-heavy look, pill
   controls, icon chips, empty states that say what to do.

## Non-goals

- Command palette, extra themes (Hive/Graphite etc.), Mica or blur, wallpaper-derived colour.
- A sidebar that folds into an icon rail at narrow widths.
- New task features (repeat, snooze, etc.). Behaviour not mentioned here stays as it is.

## Anti-goals (a failure even if it works)

- Any task data lost or rewritten in a way older builds can't read.
- Idle CPU or RAM noticeably above today (~0 % idle, ~30 MB working set).
- Low-contrast text in any profile colour, in either mode.
- Taglines or helper text added to explain the UI (fix the layout instead). The current footer
  hint "Alt+P shows and hides" goes.

## Constraints

- Rust + Slint 1.18, software renderer, winit backend (no GPU, so no blur).
- Same JSON file and shape (`%APPDATA%\nix-planner\planner-tasks.json`); new fields are
  additive and optional so an older build still loads the file.
- Known Slint gotchas from earlier work still apply: don't wrap text in list rows (scroll height
  breaks); Flickable needs `max-height`; focusing a TextInput from a PopupWindow click needs a
  30 ms deferred focus; native windows exist only after the event loop starts.
- Window keeps: frameless, tray, `Alt+P`, pin (always on top), single instance, DWM rounded corners.

## Approaches considered

1. **Hand-made palettes** — every swatch gets ~11 hand-picked tones per mode. Best control, but
   each swatch costs ~22 values and the bucket fade must be hand-tuned for each.
2. **Generated from one seed colour, offered as curated swatches (chosen)** — Rust derives all
   tones from a seed by stepping lightness; the user only sees ~8 swatches pre-checked for
   contrast in both modes. One code path; adding a swatch is one line; leaves room for a custom
   colour later.
3. **Slint's built-in Fluent widgets** — rejected: Windows-blue, can't re-tint per profile,
   would replace every custom control.

Chosen by the owner's go-ahead on the recommendation: approach 2.

## Design

### Window and layout

```
┌──────────────┬──────────────────────────────────────┐
│ Nix Planner  │ Today                  ⌕   📌   –     │  title bar (drag area)
│              │                                      │
│ ● Personal   │ ┌ Add a task ───────────────────────┐ │
│ ○ Work       │ └───────────────────────────────────┘ │
│ ○ School     │                                      │
│ +            │ ┌──────────────────────────────────┐ │
│ ──────────── │ │ ○  Backup documents…              │ │  group card of task rows
│ ☀ Today    6 │ │    Phone backup: WuWa already…    │ │
│ ◷ This week 1│ │ ○  Digital footprint erasing      │ │
│ ∞ Someday  4 │ └──────────────────────────────────┘ │
│ ✓ Done    29 │                                      │
└──────────────┴──────────────────────────────────────┘
```

- Default size ~540 × 640 (was 400 × 640). Minimum width keeps the list at least ~300 px.
  Saved placement/size behaviour unchanged.
- Sidebar ~150 px on the `chrome` surface; main area on `canvas` with a 16 px top-left radius
  (NectarLink's inset look). Sidebar items are 36 px tall pills; selected = `accent-soft` pill.
- Title bar: page title in Young Serif on the left; search, pin and hide as 32 px icon buttons
  on the right. The profile switcher leaves the title bar (it's in the sidebar now).
- No footer. The open/done counts move to the sidebar badges.

### Sidebar

- **Profiles** (top): one row per profile, a colour dot + name. Click switches profile (undo
  history restarts, as today). `Ctrl+Tab` still cycles.
  - "+" row under the list adds a profile (inline name box, as today's New).
  - Right-click a profile → menu: Rename (inline), Colour (row of ~8 swatches), Delete
    (two-step confirm as today, shown in place of the menu or inline, no footer).
- **Buckets**: Today, This week, Someday, Done, each with a line icon and a count (open tasks;
  Done shows completed count). Click shows that page. `Ctrl+1/2/3` go to Today/This week/
  Someday; Done gets `Ctrl+4`.
- **Drag targets**: dragging a task over a bucket item highlights it; dropping moves the task
  there (to the top of that bucket). Dropping on another profile sends it there (replaces the
  row menu's "Send to"). Dropping on Done completes it.

### Pages

- One bucket per page. The add box adds to the page shown; on Done the add box is hidden.
- The bucket chips under the add box are removed (the page is the target).
- Tasks sit in one group card per page: rows separated by inset hairlines, card on `card`.
- Reorder inside a page by dragging, as today.
- **Done page**: completed tasks, newest first, with "Completed today / yesterday / Mon 20 Oct"
  as today; "Clear" (two-click) sits in the page header.
- **Empty page**: centred icon + one line ("Nothing for today") — no helper sentence.
- **Search** (`Ctrl+F` or the header button): replaces the add box with a search field; results
  from all buckets of the current profile show grouped under small bucket headings; Esc closes
  and returns to the page. Drag/Clear off while searching, as today.
- The page shown is stored in a **new** optional field `page` (`"now" | "soon" | "later" |
  "done"`). `selectedBucket` stays limited to now/soon/later and mirrors `page` when it is one
  of those (Done leaves it unchanged), so an older build never adds tasks straight into Done.
  `collapsed` is no longer used by the UI but is kept in the file untouched. (Builder-check call:
  older builds must still *behave* correctly with the file, not just load it.)

### Task row

- Row min height 44 px (52 px with a note line). Checkbox drawn at 20 px but its hit area is the
  whole 40 px left gutter.
- Title single line with "…"; note line below in `text-muted`, also single line.
- Due line as today (overdue in `danger`).
- On hover: `⋯` (menu) and `×` (delete) as 32 px buttons; row background steps lighter.
- Row menu keeps: Move to (buckets), due-date entries as today; "Send to" moves to the sidebar
  drag (keep it in the menu too if a menu entry is cheap — builder's call).
- **Adding a note**: the row menu gets "Add note" when the task has none; when editing a title,
  `Enter` still opens the note line (existing shortcut kept).
- **Cursor**: entering edit mode places the cursor where the user clicked if Slint can map it,
  otherwise at the end of the text. Never at the start.

### Colour

- Each profile stores a seed colour: new optional field `colour` (hex string) on `profiles[]`.
  Missing → assign by position from the swatch list (Personal = coffee `#B0703A`).
- Rust computes the token set from the seed + mode and pushes it into a Slint `Theme` global:
  `chrome, canvas, card, card-alt, accent-soft, accent, text, text-muted, hairline, hover,
  danger`, plus the bucket fade `now, soon, later, done`. Roles as in `docs/nectar-style.md`.
- Derivation: keep the seed's hue, set surface tones by lightness steps (light mode: very light,
  low-saturation tints; dark mode: dark, low-saturation shades), `accent` = seed adjusted to
  meet contrast on `canvas`. Exact maths is the builder's, but each swatch must pass WCAG AA
  (4.5:1) for `text` and `text-muted` on `card` and `canvas`, and 3:1 for `accent` on `canvas`,
  in both modes — enforced by a unit test over the swatch list.
- **Where the bucket fade shows** (the one expressive element, kept): the sidebar bucket icons,
  the page title, the checkbox rings on that page, and the bucket headings in search results.
  Selected/hover pills, toggles and links use `accent`. Because the fade colours checkbox rings
  (a control boundary) and the title, every fade tone except `done` must also reach 3:1 on
  `card` and `canvas` in both modes; the same unit test covers it. Today's pale "later"
  (`#C49A74` on white) would fail, so the fade steps get closer together than today's.
  (Builder-check call.)
- ~8 curated swatches: coffee plus blue, green, teal, purple, rose, slate, amber (final values
  picked by the builder to pass the test).
- **Mode**: follow Windows' "app mode" (registry `AppsUseLightTheme`) at start and when it
  changes (WM_SETTINGCHANGE or the existing 5 s tick). No in-app toggle.
- Switching profile re-tints immediately (no animation needed; a short colour fade is fine if free).

### Widget

- Stays transparent on the wallpaper with today's light/dark text sampling.
- Uses the current profile's `accent`/fade for checkboxes and the date; checkboxes grow to the
  20 px drawn / larger hit area like the main window.
- Content unchanged: up to the rows that fit of the current profile's open Today tasks.

### Code layout

- Split `ui/app.slint` (1,558 lines) into a few files while redrawing: `theme.slint` (Theme
  global + shared small components), `sidebar.slint`, `task_row.slint`, `app.slint` (window,
  pages, widget). Exact split is the builder's.
- Palette maths in a new `src/theme.rs` with its contrast test.

## Left to the builder

- Exact token maths and swatch values (within the contrast rule).
- Exact pixel sizes beyond the 32 px minimum and the numbers given above.
- Icons for buckets (line icons drawn as Slint paths, like the existing ones).
- Whether "Send to" also stays in the row menu.
- Smaller calls the reviewer flagged as theirs to make: clamp a saved 400 px width up to the new
  minimum; save colours assigned by position so deleting a profile doesn't reshuffle the others;
  how a drop from Done restores a task; the widget's accent on dark wallpapers.
- Order of work; tests for new store behaviour (Done page selection, colour field round-trip,
  drop-on-sidebar moves).
