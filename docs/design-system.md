# Eclipse's design system

One design language for the launcher, the website and the few overlays the game draws that
vanilla has no screen for (the on-screen keyboard a controller types with). Every game screen
and the HUD are not part of it: they are vanilla's, because parity is the point of the game, and
Eclipse's own settings are pages built from vanilla's widgets, the way mods add theirs.

The tokens live in one place, `crates/eclipse-theme`. The launcher (`launcher/src/theme.rs`)
and the game (`crates/eclipse-client/src/ui_theme.rs`) read them as Rust constants, and the
website reads them as CSS custom properties generated from the same crate
(`cargo run -p xtask -- tokens` writes `crates/eclipse-theme/tokens.css`, which the website's build
reads from main; a test fails when the file is out of date). Change a value in the crate and all three follow.

## Where it comes from

The direction was set by [mkdir.directory](https://mkdir.directory/), a directory site that is
almost nothing but type. What we take from it is its restraint rather than its content:

1. **Type is the interface.** One family, Geist, and its mono. Large, tight display type
   (medium weight, -2 % tracking) carries the hierarchy, so there is no need for boxes, icons or
   colour to say what matters. Instrument Serif is gone.
2. **Lists, not cards.** Content is a numbered list: a small mono index on the left (`01`,
   `02`…), the name large, a mono detail on the right, and a 1 px rule between rows. Versions,
   settings, worlds, servers, release notes and the parity table are all this one component.
3. **One accent, used once.** Neutrals do the work. The violet marks the single main action on a
   screen (Play, Download), focus, links and progress; selection is shown in ink, not in colour.
4. **Space instead of borders.** Generous margins (64 px on wide screens, 16 px on phones), no
   shadows, no gradients, no outlines around groups. A surface tint appears only under the
   pointer or behind something that must stand apart (a console, a dialog).
5. **Quiet motion.** Things arrive with a short fade and an 8 px rise, a few tens of
   milliseconds apart, eased out, never bouncy. Hovers fade a soft plate in behind the row.
6. **A signature.** A giant, faint wordmark: at the foot of the website, behind the launcher's
   opening and the game's title screen.

## Tokens

### Colour

Cool, slightly blue neutrals in two appearances with the same restraint, and Eclipse's violet.
Every text colour passes WCAG AA (4.5:1) on every background it is used on; the tests in
`eclipse-theme` check each pair in both palettes.

| Token        | Light     | Dark      | Use |
|--------------|-----------|-----------|-----|
| `page`       | `#fafafc` | `#0d0e13` | the background |
| `plate_soft` | `#f3f3f8` | `#181a22` | hover plate, fields, segmented track, console |
| `plate`      | `#ebecf4` | `#20222c` | pressed plate, switch off, the faint wordmark |
| `surface`    | `#ffffff` | `#16181f` | dialogs and menus that sit above the page |
| `rule`       | `#e2e3ea` | `#24262f` | 1 px rules between rows, tracks |
| `ink`        | `#16181f` | `#ecedf3` | text, selection, slider fill, the inverted segment |
| `ink_soft`   | `#3b3e49` | `#c5c7d1` | secondary text, values |
| `muted`      | `#646875` | `#8b8f9c` | labels, descriptions, mono indices and details |
| `accent`     | `#5b3fd6` | `#9a86ff` | the main action, links, focus, progress |
| `on_accent`  | `#ffffff` | `#0d0e13` | text on the accent |
| `danger`     | `#b4372a` | `#f07a68` | errors, the crash notice's dot |
| `success`    | `#1d7a48` | `#45c27a` | "Matches vanilla", "Ready" |

The game's menus draw over the live title backdrop, always in the dark palette, with the page
colour as a flat veil so text keeps its contrast whatever the world shows: `VEIL` (0.90) on
every page and in a world, `HOME_VEIL` (0.80) on the title's home page, which shows more of the
world and keeps to `ink` and `ink_soft`. Muted text over the backdrop is `BACKDROP_MUTED`
(`#9fa3ae`). A test holds every pair to AA over a backdrop of luminance 0.77 (the snowiest title
panorama measured reaches 0.49), on the bare veil and on a selected row's plate. Their plates
are the dark `ink` at 8 % and 14 % rather than `plate_soft` and `plate`: over the veil they come
out the same, and they stay visible over a bright backdrop.

### Type

Geist (400, 500, 600) and Geist Mono (400), bundled under the SIL Open Font License.

| Token      | App px | Web (min–max px) | Line | Tracking | Weight | Use |
|------------|--------|------------------|------|----------|--------|-----|
| `micro`    | 11     | 11               | 1.5  | +0.04em  | mono   | indices (`01`), counts, timestamps |
| `caption`  | 12     | 12               | 1.5  | 0        | 400    | footnotes, help under a control |
| `label`    | 13     | 13               | 1.5  | 0        | 400    | row labels, nav, buttons |
| `ui`       | 14     | 14               | 1.5  | 0        | 400    | body text in the apps |
| `body`     | 15     | 15–17            | 1.6  | 0        | 400    | paragraphs |
| `heading`  | 20     | 20–24            | 1.3  | -0.01em  | 500    | section heads |
| `title`    | 28     | 32–48            | 1.05 | -0.02em  | 500    | list item names |
| `display`  | 44     | 34–64            | 1.05 | -0.025em | 500    | page titles |
| `wordmark` | 220    | 18vw             | 0.8  | -0.04em  | 500    | the faint signature |

### Space, shape and lines

- Spacing scale: 4, 8, 12, 16, 24, 32, 48, 64, 96. Page gutter 64 (apps and wide web), 16 on
  phones. Text measure at most 56 characters for paragraphs.
- Radii: 8 for fields and small controls, 12 for plates (hover rows, console, dialogs), full for
  buttons, segmented controls and switches.
- Lines: 1 px, `rule`. No other borders.
- Heights: control 32, large control (Play) 40, segment 28, list row 64 (apps), 86 (web).

### Motion

| Token   | Duration | Easing                          | Use |
|---------|----------|---------------------------------|-----|
| `quick` | 150 ms   | `cubic-bezier(.4, 0, .2, 1)`    | hovers, presses, colour |
| `move`  | 240 ms   | `cubic-bezier(.23, 1, .32, 1)`  | selection sliding, switches, page changes |
| `enter` | 600 ms   | `cubic-bezier(.23, 1, .32, 1)`  | content arriving: opacity 0→1, rise 8 px |
| `stagger` | 40 ms  |                                 | delay between list rows arriving |

`prefers-reduced-motion` (web) turns `enter` into a plain fade.

## Components

The launcher's components live in `launcher/src/ui/` (one file per family: `buttons.rs`,
`items.rs`, `tiles.rs`, `surfaces.rs`, `hover.rs`, `press.rs`) and are reached as `ui::…`. Each
takes its text already translated and reads the frame's look itself; a screen is built from them,
never from hand-made copies. The look they draw is § Corona's.

- **Buttons** (`ui::button`, `ui::icon_button`, `ui::more_button`, `ui::split`): Monocraft,
  uppercase, at 18; 32, 36, 44 or 56 tall (`ButtonSize`). *Primary* is the amber block with its
  bevel, one per screen: Play on Home (Install Eclipse or Try again before the first install),
  Create on New instance, Start on a stopped server, Install on a mod's card. Empty and error
  states inside a screen that already has one use Secondary (`ui::error_state_with` takes the
  kind of its Try again). *Secondary*, a faint fill, is every other action, in rows, cards and
  headers. *Ghost*, text only, is for Cancel, Close, Dismiss and Not now; icon-only buttons (⋯,
  the bin, a folder, Close) are Ghost too, their label the tooltip. Work under way keeps its
  button and disables it with the label in progress ("Checking…", "Installing…"); Play fills its
  block instead (`ui::primary_busy`). A disabled control is 40 % opaque (`ui::disabled`).
- **Asking in place** (`ui::confirm`, `ui::menu_confirm`): Delete… and Remove… turn into a line
  that names the thing in danger text, a Ghost Cancel and a Secondary verb: "Delete" when files go
  to the trash, "Remove" when an entry leaves a list. One question at a time; Escape cancels it.
- **Tabs** (`ui::tab_strip`, `ui::tab`, `ui::count_tab`): Monocraft labels, the open one
  underlined in the mark, 16 above what they open. Home's strip shows counts when they fit and
  tightens its tabs (`TAB_PAD_TIGHT`) before it would wrap. **Segments** (`ui::segments`) pick one
  of a few values on an inset track.
- **Switches** (`ui::toggle`, `ui::toggle_row`, `ui::toggle_label`): a blocky switch; in a row
  the whole row is pressed.
- **Rows and selection** (`ui::row`, `ui::list_row`, `ui::choice`, `ui::pick_row`, `ui::chip`):
  selected is a raised fill with a hairline edge and the icon in the mark (`ui::selected`), never a
  bar at the left.
- **Fields** (`launcher/src/fields.rs`): one store holds every text field; a field takes the
  button sizes, an icon before the text and its button inside, and turns its ring red when
  invalid.
- **Menus** (`ui::popover`): one open at a time, 280 wide, 8 under its button; the keyboard moves
  into it, and Escape gives the focus back to the button.
- **States** (`ui::empty_state`, `ui::loading_state`, `ui::error_state`), centred in a card: an
  icon tile, a title, one sentence that ends with the next step, then the actions. An error shows
  its first line, Try again and Copy details (the whole error).
- **Notices** (`ui::notice`, `ui::notice_line`, `ui::error_note`, `ui::error_line`,
  `ui::error_card`): an icon or a dot and a line on a faint fill; an error is never red text
  alone.
- **Progress** (`ui::bar`, `ui::progress_bar`): a 4 px track; unknown progress shuttles, and both
  stand still under reduced motion.
- **Keyboard**: anything pressable is a Tab stop with Enter and Space (`.on_press`); the focus ring
  shows for the keyboard only; F6 moves between the page and the sidebar; Home opens with Play
  focused.
- **Element ids** are stable and descriptive: keyed by the enum (`home-tab-{tab:?}`) or by a list
  item's name, path or id, never by an English label or a position.

## Surfaces

- **Launcher** (`launcher/`): § Corona. A sidebar (the account at the top, the pages, the
  instances by group with New instance, Settings and the launcher's version at its foot) beside
  the page, which is centred on a maximum width over the eclipse scene. Home is the selected
  instance's head (its icon and name, Play, the version and the folder, a friend's invite code)
  over its tabs; Mods holds the mod, modpack, resource pack, shader pack and data pack tabs, the
  results beside a project's card on wide windows; Servers hosts worlds; Versions and Patch notes
  are lists of cards; Settings is its sections as cards beside a list of them (tabs under the
  title in a narrow window). Dark by default, Light or System in Settings (Appearance).
- **Website** (horizon-rust/horizon-site): the Blocks look below. The same page structure as before (what it
  is, screenshots, parity, launcher, patch notes, community), headings in Pixelify Sans, lists as
  stacks of panels, the stone texture behind the page and Eclipse's pixel landscape (a dawn or a
  sunset) behind the hero. Light and dark follow the system, with a toggle. A giant wordmark in
  pixels sits in the footer.
- **Game**: vanilla's screens. The on-screen keyboard is a rounded plate of keys in the page
  colour, the focused key in the accent. Eclipse's graphics settings screen takes the Blocks
  palette below in a lighter form (`graphics_style.rs`).

## Blocks

A blocky, Minecraft-like look in Eclipse's purple, for the website and the game's graphics
settings screen:
Minecraft's inventory rather than a document. Its colours are `eclipse_theme::Blocks`
(`BLOCKS_LIGHT`, `BLOCKS_DARK`), which `tokens.css` carries under the same names as before plus
`--panel`, `--raised`, `--button`, `--well`, `--bevel`, `--lip`, `--outline` and the `--face`
of the main action. The game's other menus keep `LIGHT` and `DARK`.

In the game the graphics settings screen draws the Blocks palette in GUI pixels, lighter than the
website's blocks so the world stays in view: square panels of the page colour, see-through, with a
1 px edge and no bevel or lip, flat controls, the violet face on Done, and vanilla's font rather
than Pixelify. Hover and focus are vanilla's white edge.

- **Pixel type for what names and what you act on.** Headings, navigation, buttons and the
  wordmark use Pixelify Sans (OFL; Medium, SemiBold, Bold); paragraphs and numbers stay in Geist
  and Geist Mono so long text reads easily. The bundled copies add the three uppercase Cyrillic
  letters upstream leaves out, О, П and І: the Latin O's glyph, the lowercase п lifted to that
  weight's cap height, and the Latin I's glyph, so a Russian or Ukrainian heading stays in the
  pixel face.
- **Blocks, not rounded cards.** Every surface is square: a 2 px dark outline, a 2 px light bevel
  along the top and a darker lip along the bottom. The main action is the violet face with its own
  bevel and a 4 px lip; it sinks a pixel when pressed. Code and fields are sunk wells.
- **Pixel art drawn in code.** The stone behind the pages is close shades of the page colour
  (`TILE_SHADES`), never Minecraft's stone; the landscape is a banded sky, a sun cut in slices like
  the mark, hills and square trees (a sunset in dark, a dawn in light). The website draws them in
  its `scripts/art.mjs`.
- **Icons**: Eclipse's 16 px pixel SVGs in `crates/eclipse-theme/icons/pixel/`, tinted with the
  text colour.
- **Contrast**: every text colour meets WCAG AA (4.5:1) on the panels, the page and the lightest
  stone shade in both palettes (`blocks_text_meets_wcag_aa_in_both_appearances`).

## Night

The colours of the scene behind the launcher's pages (§ Corona): `eclipse_theme::night`, `NIGHT`
for the dark appearance and `DAWN` for the light one. `launcher/src/scene.rs` paints the sky from
its six bands (`sky`, top to horizon), and in light the page and the sidebar take `DAWN`'s `page`
and `side`. `tokens.css` carries every value as `--night-*`, with the `night-*` type steps in
Bricolage Grotesque (SIL OFL 1.1, `fonts/OFL-BricolageGrotesque.txt`), for website pages that wear
the scene; the launcher sets no text in Bricolage. A test holds the module's text colours to AA
over every band (`night_text_meets_wcag_aa_over_the_sky`). The values are in
`crates/eclipse-theme/src/night.rs`.

## Corona

The launcher's look: quiet charcoal cards with hairline edges over a pixel eclipse scene, white
text at a few opacities, one amber accent, Monocraft for what is pressed and named, Geist for
what is read. Its tokens are `eclipse_theme::glass` (the module keeps that name: `tokens.css`
carries the colours as `--glass-*`); the launcher's components are `launcher/src/ui/`, its layout
numbers `launcher/src/layout.rs`.

- **The scene is the background**: a pixel eclipse (an eclipsed sun with an amber corona) over
  ridges, blocky terrain, trees and torches, night in the dark appearance and dawn in the light one
  (`launcher/src/scene.rs`, colours in `eclipse_theme::night`). It is drawn at device pixels on a
  whole texel and sits in the gutter beside the page on wide windows.
- **Opaque cards, one level only.** A card is `look.plate` (`#15171e` for amber) with a hairline
  edge (`EDGE`, white at 7 %; `EDGE_LIGHT`, 12 % of the ink, in light), paper white in light.
  Menus and dialogs float on `FLOAT` with a shadow. Corners (`glass::radius`): dialogs 14, cards
  12, menus 10, rows, fields and tiles 8, buttons and tabs 6, switches 4.
- **One accent, corona amber** (`ACCENT`, `#f5a524`). It fills the one main action of a view (the
  Play block, with a 2 px shine and a 3 px shade), the selected icon, the open tab's underline, a
  switch that is on and the ticks. `accent-soft` (amber mixed with white in OKLab) draws progress,
  links and the focus ring. In light, `Glass::light` darkens the mark and the soft shade until
  they read on every light surface (a test checks every accent the picker makes); a switch that is
  on is filled with the mark there. Text on the accent turns dark when the accent is light.
- **Selection is a raised plate**: the selected fill (white at 10 %) with a hairline edge, and the
  item's icon in the mark. Never a bar at the left edge.
- **White text at a few opacities** (`glass::ink`: full, strong 86 %, body 72 %, muted 60 %, dim
  50 %; `ink_light` maps them onto the light ink so every level reads at 4.5:1 on every surface) and
  white fills for states (`glass::fill`: hover 5 %, field 7 %, button 8 %, selected 10 %, active
  12 %, the danger fill 12 % of the red, the inset well black at 25 %). Live `#4ade80`, warn
  `#ff7059`, danger `#ff8a8a`; their light forms are checked against every light plate and hover
  fill.
- **Type**: Geist through the steps of `glass::step` (label 11, caption 12, note 13, body 14, lead
  15, title 16, figure 22). Monocraft, uppercase, for navigation, tabs and buttons (18) and for
  card and page titles (27 and 36): only on its 9 px grid (`MONOCRAFT_PIXEL`), where each font
  pixel is a whole number of screen pixels. Text is drawn in greyscale so the pixel stems stay
  clean.
- **Space** comes from `glass::space` (2, 4, 8, 12, 16, 24, 32); cards stack 16 apart
  (`layout::CARD_GAP`), a tab strip sits 16 above what it opens, a note 12 under a title.
- **Controls**: buttons are 32, 36, 44 or 56 tall (`ButtonSize`), Primary (the amber block),
  Secondary (a faint fill) or Ghost (text only); fields take the same sizes, an icon before the
  text and their button inside. A disabled control is 40 % opaque and never lights under the
  pointer. The keyboard focus ring is 2 px of `accent-soft`: outside a solid control with a gap in
  the page colour, inside anything translucent.
- **Motion** (`launcher/src/motion.rs`): hovers fade over `quick`, a page arrives with a fade and
  a 6 px rise, a switch's knob slides. Under reduced motion each is drawn where it ends, and
  progress bars stand still.
- **Icons**: Eclipse's pixel set, drawn on an 8 px grid at 2 px a pixel and filled in the text
  colour (`crates/eclipse-theme/icons/glass/`, served as `icons/glass/<name>.svg`).
- **Pictures**: a mod's or pack's own icon and gallery images from its source,
  downloaded when first shown and kept on disk (`launcher/src/browser/images.rs`); the launcher
  ships no third-party pictures. Plain Minecraft shows Eclipse's own grass block
  (`tools/launcher_grass_block.py`), Eclipse the brand icon (`docs/media/brand/icon.svg`). Sources are named in text ("from Modrinth"): Modrinth's logo is
  all rights reserved, so it is not shown.
- **Sign in with Microsoft** (`launcher/src/microsoft.rs`): Microsoft's four-square symbol as
  Microsoft publishes it, never recoloured. `microsoft::sign_in_button` is Microsoft's own button
  (41 tall, `#2F2F2F` with white text in dark, white with `#5E5E5E` text and a `#8C8C8C` border in
  light, square corners); the owner sheet and the account card use the white 48-tall block
  (`microsoft::white_button`).
- **The sign-in sheet** (`launcher/src/owner_page.rs`, the owner gate): an opaque plate over the
  dimmed launcher, never instead of it. It says why (the account that owns Java Edition), the code
  in mono with Copy code, Open link as the main action and Not now; the window goes on by itself
  once Microsoft answers (`ECLIPSE_LAUNCHER_PAGE=sign-in` shows it with a made-up code).
- **Instance icons** (`launcher/src/instance_icons.rs`): the sidebar's rows, Home's head and the
  New instance page show the same icon: its own picture first (one the player chose, or its
  modpack's), else the block or colour chosen for it, else its mark (the loader's logo, the grass
  block for plain Minecraft, Eclipse's for Eclipse), the letter tile last. Pixel-art logos scale by
  whole pixels; each is decoded once per size.
- **Home's tabs** (`launcher/src/home_tabs.rs`): under the head, one at a time: Patch notes (an
  Eclipse instance), Mods, Worlds, Multiplayer, Packs, Screenshots, and Console once the game has
  said something. A tab names how many it holds when the strip has room ("Worlds 3")
  (`ECLIPSE_LAUNCHER_PAGE=worlds`, `instance-mods`, `multiplayer`, `packs`, `screenshots`,
  `console` opens one for screenshots).
- **Getting ready** (`launcher/src/home.rs`): Play fills with the preparation ("Preparing 34%");
  under it sit the bar, what is happening with the time left, and the steps (game files, Java,
  sounds, prepare, then play when Play was pressed), the current one lit and those done ticked:
  steps, never the log (`ECLIPSE_LAUNCHER_PAGE=getting-ready` shows it; nothing runs).
- **New instance** (`launcher/src/instance_page.rs`, its state in `instance_state.rs`): a page in
  the frame whose cards scroll under its title, with the instance as it will be, Cancel and Create
  instance pinned at the foot. Cards for the kind (Eclipse, Vanilla, Modded); a Java kind lists
  Mojang's versions (search, snapshots, old versions; the newest release chosen), Modded adds the
  loader at its newest stable version, and the name follows the choice ("Fabric 1.21.4") until
  the player types one. Create instance is the main action.

## The mark

**Eclipse line.** A sun half set below a horizon, drawn as three horizontal bars of 4, 8 and
10 units over a 12 unit line, on a 16 unit grid. The bars are the sun cut by the horizon into
slices, which also reads as layers of terrain and as a nod to voxels without drawing a block. It
is built on whole units of the 16 px grid so it stays sharp at 16 px, and it scales without
detail to 1024 px.

- Monochrome: the mark in `ink` on light, in `ink` (dark) on dark; the line can take the accent.
- App icon: the mark in `#ecedf3` with the line in `#9a86ff`, on a `#16181f` rounded square
  (macOS grid: 824 px tile with 185 px corners in a 1024 canvas; Windows and Linux use the full
  square with 22 % corners).
- Exports and where they go: `docs/media/brand/README.md`.
