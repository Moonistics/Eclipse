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

The launcher wears § Corona, built from the components in § Components. The website wears
§ Blocks; the game's overlays take § Game menus.

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
  edge (`ui::edge()`: `EDGE`, white at 7 %; `EDGE_LIGHT`, 12 % of the ink, in light), paper white in
  light. Menus and dialogs float on `FLOAT` with a shadow. Corners (`glass::radius`): dialogs 14,
  cards 12, menus 10, rows and tiles 8, buttons, tabs, fields and dropdowns 6, switches 4.
- **One accent, corona amber** (`ACCENT`, `#f5a524`). It fills the one main action of a view (the
  Play block, with a 2 px shine and a 3 px shade), a switch that is on and the open tab's 3 px
  underline (drawn in `look.mark` in light, so it reads at 3:1 on paper and the dawn sky). `look.mark` (the accent, darkened in light until it reads at 3:1 on every light
  surface) draws the selected icon and the ticks; `accent-soft` (amber mixed with white in OKLab,
  62 % amber) draws progress, links and the focus ring. In light a switch that is on keeps the
  amber fill with an ink rim (`SWITCH_EDGE_LIGHT`) so it reads at 3:1. Text on the accent turns
  dark when the accent is light. Tests check every accent the picker makes.
- **Selection is a raised plate**: the selected fill (white at 10 %) with a hairline edge, and the
  item's icon in the mark (`ui::selected`). Cards and chips to pick from, which have no icon to
  light, take a 3:1 edge instead (`ui::chosen`, `SELECTED_EDGE`): segments, accent swatches,
  chips and choice cards. A choice card also carries the mark's tick in its corner
  (`ui::choice_card`), a picked filter chip leads with one and a picked swatch shows one. In the light sidebar the plate is paper with a hairline and a 1 px shade under it
  (`ui::side_selected`), so it reads raised in both appearances. Never a bar at the left edge.
- **White text at a few opacities** (`glass::ink`: full, strong 86 %, body 72 %, muted 60 %, dim
  50 %; `ink_light` maps them onto the light ink so every level reads at 4.5:1 on every surface) and
  white fills for states (`glass::fill`: hover 5 %, button 8 % (12 % in light, with a hairline),
  selected 10 %, active 12 %, the danger fill 12 % of the red, the inset well black at 25 %; light
  fields 7 % of the ink). Dark fields are opaque, `FIELD_BG` and `FIELD_BG_FOCUS`. Live `#4ade80`,
  warn `#ff7059`, danger `#ff8a8a`; their light forms are checked against every light plate and
  hover fill.
- **Type**: Geist through the steps of `glass::step` (label 12, caption 12, note 13, body 14, lead
  15, title 16, figure 22, code 28 for a sign-in code); Geist Mono through `ui::mono`. Monocraft, at its regular weight only (its bold sits off the pixel
  grid), only on its 9 px grid (`MONOCRAFT_PIXEL`, `glass::pixel_text`): 18 for buttons, tabs,
  navigation (uppercase) and section titles (`ui::section_title`, mixed case); 27 for card and
  entity titles, empty, loading and error state titles (mixed case) and the Hero buttons' labels
  (PLAY); 36 for page titles. Hierarchy comes from size: page title 36 > entity and state title
  27 > section title 18 in full ink > a Secondary or Ghost label 18 in body ink (an icon button's
  icon too) > Geist. A section title names a group of controls; `ui::card_title` (Geist 16) names
  a thing inside a card (a release, a version, a product). Text is drawn in greyscale so the pixel stems stay clean.
- **Space** comes from `glass::space` (2, 4, 8, 12, 16, 24, 32); cards stack 16 apart
  (`layout::CARD_GAP`), a tab strip sits 16 above what it opens, a note 12 under a title.
- **Controls**: buttons are 32, 36, 44 or 56 tall (`ButtonSize`), Primary (the amber block),
  Secondary (a faint fill, its label in body ink that turns full under the pointer) or Ghost (text
  only); fields take the same sizes, an icon before the text and their button inside, and a
  placeholder never wraps. A disabled control is 40 % opaque, never lights under the pointer and
  tells assistive tech it is unavailable; one that can't be pressed for a reason keeps its Tab stop
  and says why in a tooltip, on keyboard focus too, with its focus ring at full strength
  (`ui::unavailable`). Play never greys out: it
  prepares the game first when it has to, and while it works it fills its block with its ⋯ on the
  same track (`ui::primary_busy`, `ui::more_busy`). The keyboard focus ring is 2 px
  of `accent-soft`: outside a solid control with a gap in the page colour, inside anything
  translucent.
- **Window**: at least 800 × 500 (`layout::MIN_WINDOW`); the sidebar is 236 wide, 208 under 960
  (`layout::sidebar`), the page gutter 32 (24 there); a new window is 1180 × 760, never more than
  90 % of the display. Under 720 tall the page head and the sidebar tighten (`ui::short_window`).
- **Scrolling**: every pane that scrolls shows its bar (`scrollbar::Scrollbar`, a thumb of white at
  36 % or the ink at 50 % in light, 3:1 on what it crosses), nested lists and long menus too
  (`ui::with_scrollbar`). Every page's pane runs to the window's edge, its bar there
  (`ui::page_pane` for a pane of its own under a still title: Settings, New instance). New
  instance's pane ends above its foot, so nothing focused hides under it. A list that scrolls
  fades at the end it hides (`ui::more_above_fade`, `ui::more_below_fade`) in its surface's colour,
  and keeps what it reveals clear of the fade (`ui::SCROLL_FADE`).
- **Motion** (`launcher/src/motion.rs`): hovers fade over `quick`, a page and a toast arrive with a
  fade and a 6 px rise over 200 ms, a switch's knob slides for 200 ms. Under reduced motion each is
  drawn where it ends, and progress bars stand still.
- **Icons**: Eclipse's pixel set, filled in the text colour: 16 px icons on an 8x8 grid
  (`crates/eclipse-theme/icons/glass/`); icons at 24 px have their own drawings on a 12x12 grid
  (`icons/glass/24/`, served as `icons/24/<name>.svg`, falling back to the 16 px drawing); both
  2 px a pixel (`ui::icon` asks `icons::path` for the size). 16 in controls, 24 in the Hero
  buttons (Play and its icon buttons alike) and the empty, loading and error states. A text button
  carries an icon when it names what the button acts on or does: refresh (Try again), copy
  (Copy details), globe (Proxy settings), folder, download, plus (add, create), search (find);
  confirmation verbs, Cancel, Close, Dismiss and Not now carry none.
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
  instance at the foot. Cards for the kind (Eclipse, Vanilla, Modded); a Java kind lists
  Mojang's versions (search, snapshots, old versions; the newest release chosen), Modded adds the
  loader at its newest stable version, and the name follows the choice ("Fabric 1.21.4") until
  the player types one. Create instance is the main action. An instance's Duplicate… opens the
  same page as "Duplicate {name}", with the game it plays and Duplicate as the main action.

## Components

The launcher's components live in `launcher/src/ui/` (one file per family: `buttons.rs`,
`items.rs`, `tiles.rs`, `surfaces.rs`, `hover.rs`, `press.rs`; `mod.rs` holds the look, text,
icons and progress) and are reached as `ui::…`. Each takes its text already translated and reads
the frame's look itself; a screen is built from them, never from hand-made copies, and a call site
doesn't restyle one (a variant it needs goes into the component). The look they draw is § Corona's.

- **Buttons** (`ui::button`, `ui::icon_button`, `ui::more_button`, `ui::split`): Monocraft,
  uppercase, at 18 (27 with a 24 px icon at Hero size); 32, 36, 44 or 56 tall (`ButtonSize`).
  *Primary* is the amber block with its bevel, one per screen: Play on Home (Install Eclipse or Try
  again before the first install), Create on New instance, Start on a stopped server. Empty and
  error states inside a screen that already has one use Secondary (`ui::error_state_with` takes
  the kind of its Try again). Labels are uppercase; Secondary and Ghost labels and icon buttons'
  icons rest at body ink and turn full under the pointer. *Secondary*, a faint fill, is every other action, in rows, cards and
  headers. *Ghost*, text only, is for Cancel, Close, Dismiss and Not now; icon-only buttons (⋯,
  the bin, a folder, Close) are Ghost too, their label the tooltip and the accessible name (the
  one exception is Home's folder beside Play: a filled Secondary, Hero size like Play). An icon
  button that is open or current is selected (the plate and the icon in the mark). Work under way
  keeps its button and disables it with the label in progress ("Checking…", "Importing…",
  "Loading…"); Play
  fills its block instead (`ui::primary_busy`), its ⋯ on the same track (`ui::more_busy`). A
  disabled control is 40 % opaque (`ui::disabled`); `ui::unavailable(control, why)` keeps it a
  Tab stop and says why.
- **Delete and remove**: one opener per context. A row's is the bin (`ui::remove_button`, labelled
  "Delete…" or "Remove…"), a menu's the danger row (`ui::menu_row(.., true)`), and a card without
  rows a plain Secondary. The opener turns into a line that names the thing in danger text, a
  Ghost Cancel and a Secondary verb (`ui::confirm`, `ui::menu_confirm`, which take a
  `ui::Confirm`: `Delete(name)`, `Remove(name)`, `Restore(name)`, so the question and its verb
  always pair): "Delete" when files go to the trash, "Remove" when an entry leaves a list. One
  question at a time; Escape cancels it. Escape on a New instance page with changes asks
  "Discard your changes?" the same way.
- **Tabs** (`Launcher::tab_strip` in `launcher/src/tab_strip.rs`, drawn with `ui::count_tab`,
  `ui::tab_strip_padded` and `ui::tab_width`): Monocraft labels, the open one underlined in the
  accent, 16 above what they open. Every strip (Home, Mods, a project's page, a server, Settings in
  a narrow window) shows its counts when they fit, then tightens its tabs (`TAB_PAD_TIGHT`), then
  moves the tabs that still don't fit under More tabs (a ⋯ menu), keeping the open one in the
  strip: it never wraps. **Segments** (`ui::segments`) pick one of a few values on an inset track:
  one Tab stop, the arrows pick.
- **Switches** (`ui::toggle`, `ui::toggle_row`, `ui::toggle_label`): a blocky switch; in a row the
  whole row is pressed and is the switch.
- **Navigation** (`ui::nav_row` with `NavDensity::Sidebar` or `List`): the sidebar's pages and
  Settings' section list; a label that doesn't fit gets its tooltip (`ui::tip_when_clipped`, the
  room from `ui::nav_text_room` and `ui::side_row_text_room`). A page's name, title and icon have
  one owner (`Page::label`, `Page::title`, `Page::icon` in `main.rs`).
- **Rows and selection** (`ui::row(id, name, current)`, `ui::side_row(id, name, current)`,
  `ui::list_row`, `ui::choice`, `ui::choice_card`, `ui::list_choice`, `ui::pick_row`,
  `ui::pick_item`, `ui::chip`, `ui::filter_chip`): a row names itself and says whether it is
  current; `ui::selected`, `ui::chosen` and `ui::side_selected` draw what is picked. A list row keeps its fill while the
  pointer is on one of its buttons; `ui::hover_within(&id)` tells its code (one owner of hover:
  `ui/hover.rs`).
- **Lists to pick from** (`ui::options`): one Tab stop for the whole list; the arrows, Page Up and
  Down, Home, End and typed letters move through it, a single choice follows the cursor and a
  multiple one toggles with Space or Enter. Used by New instance's versions, games, loaders, icons
  and colours, the accent swatches, segments, the Mods results (a row's Install is a Tab stop only
  on the row the cursor is on) and the Mods filters. Shift+F10 or the Menu key opens a sidebar
  row's ⋯ menu.
- **Labelled groups** (`ui::field_group`, `ui::section_card`, `ui::labelled_field`): a section
  title (a level-2 heading) over its content, in a card or not; a field's own label is `ui::label`
  (Geist 12 caps). A dropdown that names itself shows its name inside its face
  (`ui::labelled_dropdown`: over the value at Hero size, before it otherwise).
- **States** (`ui::state_line`, `ui::state_note`, `ui::state_title`): the dot and its state, one
  line in rows, a wrapping note in cards, a tinted line beside a title. Status everywhere (instances,
  servers, updates, a mod's support) is this one component.
- **Fields** (`launcher/src/fields.rs`): one store holds every text field; a field takes the
  button sizes, an icon before the text and its button inside, and turns its ring red when
  invalid. Its accessible name is what it holds (`Field::name`), its placeholder only an example,
  and it fits its text to the width it was last drawn at.
- **Menus** (`Launcher::menu_opener`, `Launcher::menu_popover`, `ui::menu_under`,
  `ui::menu_beside`): one open at a time, 280 wide and scrolling past 360, opened `MENU_GAP` under
  its button (above it when there is no room below), with one solid backing. The keyboard moves
  into it (onto the picked row, `ui::starts_menu`); Tab, F6, the palette or a press outside close
  it, and Escape gives the focus back to its button. A menu opened from elsewhere goes through
  `Launcher::open_menu_from` / `open_menu_for`.
- **Dialogs** (`ui::modal`, `ui::sheet`, `ui::dialog`): the launcher dimmed under a plate, or a
  sheet over the whole window. `ui::dialog(id, eyebrow, title, DialogWidth::Narrow | Wide, body,
  foot)` owns the plate, its width, padding and title (a heading, 27). Tab stays inside, focus
  moves in when it opens and back to what opened it when it closes.
- **States in a pane** (`ui::empty_state`, `ui::loading_state`, `ui::error_state`), centred in a
  card: an icon tile, a title at 27, one sentence that ends with the next step, then the actions.
- **One presentation for a failure**: a plain title, one plain cause line, the actions that fix it
  and Copy details with the whole error. In a pane `ui::error_state` / `ui::error_state_with`
  (the cause is the error's first line) or `ui::error_state_told` (a cause written for the
  player); in a card `ui::error_line_for(id, title, error, retry, extra)`. Never a raw multi-line
  error on screen.
- **Notices** (`ui::notice(Notice::Info | Warn | InCard, ..)`, `ui::notice_line(tone, ..)`,
  `ui::error_note`, `ui::error_line`, `ui::error_card`): an icon or the state dot and a line on a
  faint (or warn) fill, one padding (`NOTICE_PAD`), its actions with Cancel first; a notice that is
  a card's whole content is drawn flat in it (`InCard`); an error is never red text alone. A toast says what just happened for
  6 s (`toast::say`), an error until it is dismissed (`toast::say_error`).
- **Tiles and faces** (`ui::tile`, `ui::mark_tile`, `ui::face`, `ui::picture_button`): 24, 36 or
  48 (`TILE_SM`, `TILE_MD`, `TILE_LG`), a mark 32 in a 48 tile, the account's face 80 (`FACE_XL`),
  all at radius 8. Every pressable picture (gallery, screenshots, icons) is a picture button, its
  tooltip beside it. Faces are drawn by one renderer (`profile_heads`).
- **Wells** (`ui::well`, `ui::code_well`, `ui::log_well`): the inset well, a code block, a log.
- **Progress** (`ui::bar`, `ui::progress_bar`): a 4 px track; unknown progress shuttles, and both
  stand still under reduced motion.
- **Keyboard**: anything pressable is a Tab stop with Enter and Space (`.on_press`); the focus ring
  shows for the keyboard only; F6 and Shift+F6 move between the page and the sidebar; Home opens
  with Play focused and another page with its first control focused, and the first Tab after that
  shows the ring where the focus is instead of moving it. Escape closes the palette from anywhere
  in it.
- **Accessibility**: the components give AccessKit each control's role, name and state (a button
  and its label, a switch on or off, a tab, page, row or option selected, a menu button expanded, a
  field's name and value, a dropdown's value, progress, dialogs and headings: page titles at level
  1, section, state and dialog titles at 2), so every page gets them by using them. Text that
  explains (a state's note, a page's note, an error's cause, Home's play line) is `ui::said`.
- **State owners**: what Play does and shows for an instance is `Launcher::play_state`
  (`PlayState`, `game_run.rs`), which Home's head and the sidebar read; the last
  preparation, its error and the Console's lines belong to the instance that started them
  (`GameRun::begin`, `console_log`, `shown_prepare_error`); a version's name is
  `nightly::display_name`; who plays is `Launcher::who_plays`.
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

## Night

The colours of the scene behind the launcher's pages (§ Corona): `eclipse_theme::night`, `NIGHT`
for the dark appearance and `DAWN` for the light one. `launcher/src/scene.rs` paints the sky from
its six bands (`sky`, top to horizon), and in light the page and the sidebar take `DAWN`'s `page`
and `side`. `tokens.css` carries every value as `--night-*` for website pages that wear the scene.
A test holds the module's text colours to AA over every band
(`night_text_meets_wcag_aa_over_the_sky`). The values are in `crates/eclipse-theme/src/night.rs`.

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

## Game menus

The game's few overlays (the on-screen keyboard a controller types with) and the website's type,
spacing and motion read the crate's first tokens: `LIGHT` and `DARK`, cool neutrals with one
violet accent, and the steps below. `tokens.css` carries them for the website.

### Colour

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

| Token      | Game px | Web (min–max px) | Line | Tracking | Weight | Use |
|------------|--------|------------------|------|----------|--------|-----|
| `micro`    | 11     | 11               | 1.5  | +0.04em  | mono   | indices (`01`), counts, timestamps |
| `caption`  | 12     | 12               | 1.5  | 0        | 400    | footnotes, help under a control |
| `label`    | 13     | 13               | 1.5  | 0        | 400    | row labels, nav, buttons |
| `ui`       | 14     | 14               | 1.5  | 0        | 400    | body text in the game's overlays |
| `body`     | 15     | 15–17            | 1.6  | 0        | 400    | paragraphs |
| `heading`  | 20     | 20–24            | 1.3  | -0.01em  | 500    | section heads |
| `title`    | 28     | 32–48            | 1.05 | -0.02em  | 500    | list item names |
| `display`  | 44     | 34–64            | 1.05 | -0.025em | 500    | page titles |
| `wordmark` | 220    | 18vw             | 0.8  | -0.04em  | 500    | the faint signature |

### Space, shape and lines

- Spacing scale: 4, 8, 12, 16, 24, 32, 48, 64, 96. Page gutter 64 on wide pages, 16 on phones.
  Text measure at most 56 characters for paragraphs.
- Radii: 8 for fields and small controls, 12 for plates (hover rows, console, dialogs), full for
  buttons, segmented controls and switches.
- Lines: 1 px, `rule`. No other borders.
- Heights: control 32, large control 40, segment 28, list row 64 (game), 86 (web).

### Motion

| Token   | Duration | Easing                          | Use |
|---------|----------|---------------------------------|-----|
| `quick` | 150 ms   | `cubic-bezier(.4, 0, .2, 1)`    | hovers, presses, colour |
| `move`  | 240 ms   | `cubic-bezier(.23, 1, .32, 1)`  | selection sliding, switches, page changes |
| `enter` | 600 ms   | `cubic-bezier(.23, 1, .32, 1)`  | content arriving: opacity 0→1, rise 8 px |
| `stagger` | 40 ms  |                                 | delay between list rows arriving |

`prefers-reduced-motion` (web) turns `enter` into a plain fade. The launcher's hovers use `quick`;
its own page and toast arrival is § Corona's.

## The mark

**The eclipse.** A pixel eclipse: the moon (`#0b0c12`) inside a ring of corona amber (`#f5a524`)
with a pale inner rim (`#ffd58f`) and a dark outer edge (`#6b4416`), drawn in whole pixels so it
stays sharp at 16 px and scales without detail to 1024 px. It is the sun the launcher's scene
sets behind its ridge.

- App icon: the mark on a `#10131c` rounded square (macOS grid: 824 px tile with 185 px corners
  in a 1024 canvas; Windows and Linux use the full square with 22 % corners).
- Exports and where they go: `docs/media/brand/README.md`.
