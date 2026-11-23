# Nectar style — design note

A study of NectarLink's Windows app (v0.1.5, Qt Quick, "Glass" theme), written as a style we can
reuse in our own apps. Nix Planner's redesign is the first user. We copy the *design language*
(structure, proportions, behaviour), never NectarLink's icons, artwork or code.

Colour values were sampled from screenshots at 100 % scale; sizes are measured from the same
shots and rounded to the 4 px grid, so treat them as close, not exact.

## What makes it feel good

1. **Tonal, not lined.** Areas are told apart by small shifts in surface tint, almost never by
   borders or shadows. Each level is a few points lighter or darker than the one beneath it.
2. **One hue drives everything.** Every surface, chip, pill and toggle is a tint of a single
   accent hue. Change the accent and the whole window re-colours coherently (it follows the
   album art of whatever is playing, see *Dynamic colour*).
3. **Round and soft.** Large radii on cards, full pills on buttons, chips and the nav indicator.
   Nothing sharp except text.
4. **Icon + label, always.** Every action and nav item has a line icon in a soft chip with a
   short sentence-case label. No icon-only buttons except window controls.
5. **Calm density.** Generous padding, but cards are packed in a tight grid with small gaps,
   so screens read as "a dashboard of tiles", not a form.
6. **Every dead end explains itself.** Disabled controls carry a small lock badge; empty pages
   show an icon, one line saying what's missing, and one line saying how to fix it.
7. **Keyboard is first-class.** A command palette (`Ctrl+Alt+Space`) reaches every action,
   with key hints in its footer.

## Colour roles

Light and dark values from the Glass theme with a blue accent (`#657EA9`-ish). Name tokens by
role, so a different accent can be dropped in.

| Role | Light | Dark | Use |
|---|---|---|---|
| `chrome` | `#EAEBEE` | `#303539` | Title bar and nav rail: one continuous neutral frame |
| `canvas` | `#EEEFF5` | `#23282D` | Content area, inset from the chrome with a large top-left radius |
| `card` | `#F1F2F7` | `#44494D` | Default tile |
| `card-alt` | `#E8E9F2` | `#292F35` | Group cards (settings groups, right-column panels) |
| `card-hero` | `#ECEFFB` | `#15262F` | The one featured card per screen; tinted towards the accent |
| `accent-soft` | `#D9E3F9` | `#374956` | Selected row, active setting, icon chip, nav-selected |
| `accent` | `#657EA9` | `#719BBA` | Toggle on, primary button, links ("View all") |
| `nav-pill` | `#DCE1E9` | `#737C83` | Selected nav item background (neutral, not accent) |
| `hairline` | `#C1C6D6` | — | The rare divider inside a group card |
| `text` | ≈ `#1C2230` | ≈ `#E8EAED` | Titles and labels |
| `text-muted` | ≈ `#5A6070` | ≈ `#A0A7AE` | Second line, metadata |

Rules seen across the app:
- Hover lightens a tile a step towards white (dark: towards `card` + 6 %); press darkens.
- Disabled = the whole control at ~40 % opacity **plus** a lock badge, never opacity alone.
- Links are plain accent-coloured text, no underline (Graphite theme is the exception).
- Status uses small neutral chips ("LTE · 2 ms", "Muted · 100 %", "Live"), not coloured dots.

## Shape and spacing

| Thing | Value |
|---|---|
| Grid | 4 px |
| Canvas corner (where it meets the chrome) | 16 px |
| Cards | 16 px radius, 16 px padding, 12–16 px gap between cards |
| Group card rows | 48–56 px tall, 1 px `hairline` between rows, inset 16 px |
| Buttons, chips, segmented controls, nav indicator | full pill (radius = height / 2) |
| Button height | 32 px (small chip 22–24 px) |
| Icon chip | 32 px circle or squircle in `accent-soft`, 18 px line icon |
| Nav rail | 80 px wide; item = 20 px icon over 11 px label; selected pill ≈ 52 × 30 px behind the icon |
| Title bar | 52 px; page title on the left, header actions (icon + label) on the right |

## Typography

Segoe UI Variable throughout (system font, nothing embedded).

| Use | Size / weight |
|---|---|
| Page title ("Home", "Settings") | 20 px semibold |
| Hero / section title ("Appearance", device name) | 18–20 px semibold |
| Card title, row label | 14 px semibold |
| Body, button label | 13–14 px regular / semibold for buttons |
| Second line, metadata | 12 px regular, `text-muted` |
| Nav label, chip | 11 px regular (semibold when selected) |

Sentence case everywhere. Numbers and metadata use ` · ` as separator.

## Layout skeleton

```
┌──────┬──────────────────────────────────────────────┐
│ logo │ Page title                 [⌕ Commands] [⟲] _ □ ✕ │  chrome (title bar)
│      ├──────────────────────────────────────────────┤
│ nav  │ ╭ canvas ──────────────────────────────────╮ │
│ rail │ │  hero card            │ group card       │ │
│      │ │  tile  tile           │ tile  tile       │ │
│      │ │  list card            │ list card        │ │
│  +   │ ╰──────────────────────────────────────────╯ │
└──────┴──────────────────────────────────────────────┘
```

- Two-pane pages (Calls, Photos, Settings): a list pane on the left (~250 px) separated by a
  single vertical hairline, content on the right.
- The rail's bottom holds one round "+" action (add device). Everything else is top-aligned.

## Components worth copying

- **Nav rail item** — icon, label below, neutral pill behind the icon when selected.
- **Action tile** — icon chip top-left, optional secondary icon top-right, title bottom-left.
  Whole tile is the button.
- **Group card** — titled rows with a leading line icon, label + muted second line, control on
  the right (toggle, segmented control, chevron).
- **Selected row** — `accent-soft` fill with a 1 px `accent` outline (e.g. "Phone as speaker").
- **Segmented control** — pill track in `card`, selected segment a lighter pill with semibold text.
- **Pill tabs** ("Recent / Contacts", filter chips) — same as segmented, or outlined pills for
  multi-choice filters.
- **Empty state** — centred 32 px line icon, one semibold line, one muted line with the fix.
- **Command palette** — centred modal over a dimmed window, search field with an `Esc` key-cap,
  rows of icon + title + muted description + a context chip on the right, footer with
  `↑↓ to navigate · Enter to run · Esc to close`.
- **Tooltips** — small pill in `accent-soft` naming the action and its shortcut.

## Themes and dynamic colour

NectarLink ships four themes on the same skeleton, which shows how far the tokens stretch:
- **Glass** (default): the tonal look above, optional Mica wallpaper tint.
- **Bloom**: Glass with a stronger accent tint on every surface.
- **Hive**: warm honey palette plus a faint honeycomb texture on the canvas.
- **Graphite**: paper-white, square-ish outlined controls, monospace ALL-CAPS labels,
  underlined links — a deliberately "technical" skin.

Colour source can be a fixed swatch, **the current media artwork**, or the phone's wallpaper
colours (Material You style). The whole token set is regenerated from one seed hue, which is
why every theme still looks coherent.

## Smaller things you might not have noticed

- Header actions show both icon and word ("Commands", "Timeline") rather than icon-only.
- Cards have no shadow at all in light mode; depth is tint alone.
- Media card blurs the album art into its own background.
- Badges count only what needs attention (Devices `1`), never decoration.
- Setting descriptions are one short line ("Tint window with desktop wallpaper.").
- A floating "Nectar Island" pill window (like a phone's dynamic island) shows live status at
  the top of the screen.
- The palette footer reports its own latency ("1.4 ms") — a nice touch for a fast-feeling app.

## Applying it to Nix Planner

Keep (already ours, and they fit): sentence case, no emoji, calm density, the coffee accent as
the seed hue, the Today → Someday fading gradient.

Take:
- Tonal surfaces generated from the coffee seed (`chrome`, `canvas`, `card`, `accent-soft`…)
  in place of the flat porcelain + hairlines look; add a dark set.
- Pill buttons, pill chips and a segmented control for the bucket picker.
- Group-card styling for buckets (rows separated by inset hairlines inside one tinted card).
- Selected/hover/disabled rules above, and the empty-state pattern for empty buckets.
- Icon + label on header actions.

Decide during the redesign brainstorm:
- Nav rail: Nix Planner is a single compact window; a rail may cost too much width. Profiles
  could become the rail, or stay in the title bar.
- Per-profile seed colour (Personal / Work / School each tint the whole window) — our version
  of NectarLink's dynamic colour.
- Young Serif for bucket names vs. Segoe UI Variable only.
- Command palette: we already have Ctrl+F search; a palette may be scope creep.

Constraints:
- Slint software renderer: no real blur, so no Mica or blurred backgrounds; tonal surfaces only
  (which is most of the look anyway).
- Keep the widget readable on any wallpaper; it should take the tokens but not the cards.
