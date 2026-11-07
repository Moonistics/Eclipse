# Eclipse's design system

One design language for the launcher, the website and the few overlays the game draws that
vanilla has no screen for (the on-screen keyboard a controller types with). Every game screen
and the HUD are not part of it: they are vanilla's, because parity is the point of the game, and
Eclipse's own settings are pages built from vanilla's widgets, the way mods add theirs.

The tokens live in one place, `crates/eclipse-theme`. The launcher (`launcher/src/theme.rs`)
and the game (`crates/eclipse-client/src/ui_theme.rs`) read them as Rust constants, and the
website reads them as CSS custom properties generated from the same crate
(`cargo run -p xtask -- tokens` writes `crates/eclipse-theme/tokens.css`, which the website's build
reads from main; a test fails when the file is out of date).

The launcher wears § Corona, built from the components in § Components. The website wears
§ Blocks; the game's overlays take § Game menus.

## Corona

The launcher's look: quiet charcoal cards with hairline edges over a pixel eclipse scene, white
text at a few opacities, one amber accent, Monocraft for what is pressed and named, Geist for
what is read. Its tokens are `eclipse_theme::glass` (the module keeps its name: `tokens.css`
carries the colours as `--glass-*`); the launcher's
components are `launcher/src/ui/`, its layout numbers `launcher/src/layout.rs`.

- **The scene is the background**: a pixel eclipse (an eclipsed sun with an amber corona) over
  ridges, blocky terrain, trees and torches, night in the dark appearance and dawn in the light one
  (`launcher/src/scene.rs`, colours in `eclipse_theme::night`). It is drawn at device pixels on a
  whole texel and sits in the gutter beside the page on wide windows. Narrower, it sets behind the
  hills under Home: the hills stay below Home's head and a short tab (Home reports its head's
  height, `scene::track_home_head`), the sun's disc stays below that line and its corona and
  streamers fade out before reaching it; once a page runs past it the sun has set.
- **Opaque cards, one level only.** A card is `look.plate` (`#15171e` for amber) with a hairline
  edge (`ui::edge()`: `EDGE`, white at 7 %; `EDGE_LIGHT`, 12 % of the ink, in light), paper white in
  light. Menus and floats sit on `FLOAT` with a shadow (`ui::float_card`); dialogs are a card plate over
  the dimmed page (`ui::dialog_plate`); a footer pinned over a pane lifts with a shadow
  (`ui::foot_card`). Corners (`glass::radius`): dialogs 14,
  cards 12, menus 10, rows and tiles 8, buttons, tabs, fields and dropdowns 6, switches 4.
- **One accent, corona amber** (`ACCENT`, `#f5a524`). It fills the one main action of a view (the
  Play block, with a 2 px shine and a 3 px shade), a switch that is on and the open tab's 4 px
  underline (drawn in `look.mark` in light, so it reads at 3:1 on paper and the dawn sky). `look.mark` (the accent, darkened in light until it reads at 3:1 on every light
  surface) draws the selected icon and the ticks; `accent-soft` (amber mixed with white in OKLab,
  62 % amber) draws progress, links and the focus ring. In light a switch that is on keeps the
  amber fill with an ink rim (`SWITCH_EDGE_LIGHT`) so it reads at 3:1. Text on the accent turns
  dark when the accent is light. Tests check every accent the picker makes.
- **Selection is a raised plate**: the selected fill (white at 10 %) with one edge that reads at
  3:1 on what is around it (`SELECTED_EDGE` 30 %, `SELECTED_EDGE_LIGHT` 52 % of the ink in light,
  `ui::selected_edge`), and the item's icon in the mark (`ui::selected`). The same plate marks
  segments, accent swatches, chips and choice cards; a choice card also carries the mark's tick in
  its corner (`ui::choice_card`) and rests with a hairline, so the options read as a set; a picked
  filter chip leads with a tick and a picked swatch shows one. In the sidebar the current page, the
  current instance or server row and Settings' section list share one look (`ui::side_selected`,
  `ui::side_row`: paper with that edge and a 2 px shade under it in light). Never a bar at the left
  edge.
- **White text at a few opacities** (`ui::ink(level)` with a level of `glass::ink`: full, body
  72 %, muted 60 %, dim 50 %; in light `glass::light_ink` maps them onto the light ink, full, 78, 70
  and 64 %, so every level reads at 4.5:1 on every surface) and white fills for states
  (`glass::fill`: hover 5 %, button 8 %, selected 10 %, active 12 %, the danger fill 12 % of the
  red, the inset well black at 25 % or the light ink at 7 % (`INSET_LIGHT`), a text selection
  `accent-soft` at 22 % (`TEXT_SELECTION`)). Dark
  fields are opaque, `FIELD_BG` and `FIELD_BG_FOCUS`. In light, controls that sit on the page or a
  card (Secondary buttons, dropdowns, fields) are paper at 70 % (`fill::CONTROL_LIGHT`), full paper
  while a field has the keyboard, with their hairline or 3:1 field edge: raised like the dark
  ones, never sunk below the page. Live `#4ade80` (green means running and nothing else; a ready
  instance shows the dim dot),
  warn `#ff7059`, danger `#ff8a8a`; their light forms are checked against every light plate and
  hover fill.
- **Type**: Geist through the steps of `glass::step` (label 12, caption 12, note 13, body 14, lead
  15, title 16, figure 22, code 28 for a sign-in code); Geist Mono through `ui::mono`. Monocraft, at its regular weight only (its bold sits off the pixel
  grid), only on its 9 px grid (`MONOCRAFT_PIXEL`, `glass::pixel_text`): 18 for buttons, tabs,
  navigation, segments and empty, loading and error state titles; 27 for section titles (a section
  card's or labelled group's heading, `ui::section_title`), card and dialog titles and a state that
  is a page's only content (`ui::page_only`); 36 for
  page and entity titles (Home's instance name) and the Hero buttons' labels (PLAY, the loudest
  text on a screen). Only the main actions shout: PLAY and the Hero buttons, Primary buttons,
  navigation and tabs are uppercase (`ui::shouts`); Secondary and Ghost labels, segments and filter
  chips are mixed case. At 125 % and 175 % its pixel grows to whole device pixels (`ui::pixel_text`
  does it: PRESS 21.6 at 125 %), as do the 16 and 32 px icons, and the narrow sidebar keeps its
  wide width so no page name is cut (`layout::sidebar`). Hierarchy comes from size:
  page title and PLAY 36 > section, card and dialog title 27 > state title 18 in full ink > a
  Secondary or Ghost label 18 in body ink (an icon button's icon too) > an unpicked segment 18 in
  dim ink > Geist. A section title names a group of controls; `ui::card_title` (Geist 16) names a
  thing inside a card (a release, a version, a product, the player); `ui::entity_title` is only 36
  or 27. Text is drawn in greyscale so the pixel stems stay clean.
- **Space** comes from `glass::space` (2, 4, 8, 12, 16, 24, 32); cards stack 16 apart
  (`layout::CARD_GAP`), a tab strip sits 16 above what it opens, a note 12 under a title.
- **Controls**: buttons are 32, 36, 44 or 56 tall (`ButtonSize`), Primary (the amber block),
  Secondary (a faint fill, its label in body ink that turns full under the pointer) or Ghost (text
  only); fields take the same sizes, an icon before the text and their button inside, and a
  placeholder never wraps. A control busy with work under way (`ui::disabled`) is 40 % opaque,
  never lights under the pointer, tells assistive tech it is unavailable and keeps the keyboard
  focus it had (a pressable keeps one focus handle by its id in every state), though Tab passes
  it; one that can't be pressed now for a reason keeps its Tab stop and says why in a tooltip, on
  keyboard focus too, with its focus ring at full strength (`ui::unavailable`), and an unavailable
  Primary draws the Secondary face at full opacity with its label in dim ink and no bevel
  (`ui::primary_unavailable`). Play never greys out: it
  prepares the game first when it has to, and while it works it fills its block with its ⋯ on the
  same track (`ui::primary_busy`, `ui::more_busy`: a solid track, `Glass::busy_track`, the accent
  over the plate from 70 % until the label reads at 4.5:1). The keyboard focus ring is
  `accent-soft`: outside a solid control, 2.5 px beyond a 1.5 px gap in the page colour; inside
  anything translucent, 2 px.
- **Window**: at least 800 × 500 (`layout::MIN_WINDOW`); the sidebar is 236 wide and narrows to
  208 over the 28 px under 960 (`layout::sidebar`), then the page gutter narrows from 32 to 24
  (`layout::gutter`), so the page never widens as the window narrows; a new window is 1180 × 760, never more than
  90 % of the display. Under 720 tall the page head and the sidebar tighten (`ui::short_window`).
- **Scrolling**: every pane that scrolls shows its bar (`scrollbar::Scrollbar`, a 6 px thumb with
  round ends, white at 36 % or the ink at 50 % in light, 3:1 on what it crosses; the sidebar's rows
  end before it), nested lists and long menus too
  (`ui::with_scrollbar`). Every page's pane runs to the window's edge, its bar there
  (`ui::page_pane` for a pane of its own under a still title: Settings, New instance). New
  instance's pane ends above its foot, so nothing focused hides under it. A list that scrolls
  fades at the end it hides (`ui::more_above_fade`, `ui::more_below_fade`) in its surface's colour,
  and keeps what it reveals clear of the fade (`ui::fade_height()`); the sidebar's list also never
  rests with a row cut at its top.
- **Motion** (`launcher/src/motion.rs`): hovers fade over `quick`, a page and a toast arrive with a
  fade and a 6 px rise over 200 ms, a switch's knob slides for 200 ms. Under reduced motion each is
  drawn where it ends, and progress bars stand still.
- **Icons**: Eclipse's pixel set, filled in the text colour: 16 px icons on an 8x8 grid
  (`crates/eclipse-theme/icons/glass/`, `glass_icons::ALL`); icons at 24 px have their own drawings
  on a 12x12 grid (`icons/glass/24/`, `glass_icons::LARGE`, served as `icons/24/<name>.svg`,
  falling back to the 16 px drawing); both
  2 px a pixel (`ui::icon` asks `icons::path` for the size and snaps the drawing to whole device
  pixels on the grid `icons::cells` says it is drawn on). 16 in controls, 24 in the Hero
  buttons (Play and its icon buttons alike) and in a state that is a page's only content; a state
  inside a card draws 16 in its 36 tile. One icon per meaning, named in `launcher/src/icons.rs`
  (`SERVER`, `WORLD`, `MULTIPLAYER`, `PACK`, `VERSION`, `MODS`, `NEW_INSTANCE`, `ACCOUNT`, `GAME`,
  `AI`, `FOLDERS`, `DATA_COLLECTION`, `COPY`, ...): a page or a section that means one of these
  takes its icon from there, and no two meanings share a drawing (a test keeps any two from
  sharing more than three quarters of their cells; the pack is an isometric block, not two
  squares). A text button carries an icon when it names what the button acts on or does: `RETRY`
  (Try again), `COPY` (Copy code, Copy link, Copy log), `NETWORK` (Proxy settings), `OPEN_FOLDER`
  (every folder opener and file picker: `ui::open_folder_button`, imports, Choose image…),
  `DOWNLOAD`, `ADD` (add, create), `search-01` (find). The action icons `OPEN_FOLDER`, `DOWNLOAD`
  and `ADD` are declared as the drawings of `FOLDERS`, `UPDATES` and `NEW_INSTANCE`, the only
  meanings that share one. Copy details is a labelled Secondary with the copy icon
  (`ui::copy_details`); confirmation verbs, Cancel, Keep editing, Close, Dismiss and Not now carry
  none.
- **Pictures**: a mod's or pack's own icon and gallery images from its source,
  downloaded when first shown and kept on disk (`launcher/src/browser/images.rs`); the launcher
  ships no third-party pictures. Plain Minecraft shows Eclipse's own grass block
  (`tools/launcher_grass_block.py`), Eclipse the brand icon (`docs/media/brand/icon.svg`). Sources are named in text ("from Modrinth"): Modrinth's logo is
  all rights reserved, so it is not shown.
- **Sign in with Microsoft** (`launcher/src/microsoft.rs`): Microsoft's four-square symbol as
  Microsoft publishes it, never recoloured. `microsoft::sign_in_button` is Microsoft's own button
  (41 tall, `#2F2F2F` with white text in dark, white with `#5E5E5E` text and a `#8C8C8C` border in
  light, square corners), used to add an account; the owner sheet, the account card and Home use
  the white 48-tall block (`microsoft::white_button`).
- **The sign-in sheet** (`launcher/src/owner_gate.rs`, the owner gate): an opaque plate over the
  dimmed launcher, never instead of it. It says why (the account that owns Java Edition), the code
  in mono with Copy code, Open link as the main action and Not now; the window goes on by itself
  once Microsoft answers (`ECLIPSE_LAUNCHER_PAGE=sign-in` shows it with a made-up code).
- **Instance icons** (`launcher/src/instance_icons.rs`): the sidebar's rows, Home's head and the
  New instance page show the same icon through one resolver (`Launcher::icon_for(IconSpec, size)`):
  its own picture first (one the player chose, or its
  modpack's), else the block or colour chosen for it, else its mark (the loader's logo, the grass
  block for plain Minecraft, Eclipse's for Eclipse) two thirds of its tile on the 8 px grid (a
  loader logo with its own plate fills the tile), the letter tile on its colour last (the accent
  when the colour can't be read). Pixel-art logos scale by whole pixels; each is decoded once per size.
- **Home's tabs** (`launcher/src/home_tabs.rs`): under the head, one at a time: Patch notes (an
  Eclipse instance), Mods, Worlds, Multiplayer, Packs, Screenshots, and Console once the game has
  said something. A tab names how many it holds when the strip has room ("Worlds 3")
  (`ECLIPSE_LAUNCHER_PAGE=worlds`, `instance-mods`, `multiplayer`, `packs`, `screenshots`,
  `console` opens one for screenshots).
- **Getting ready** (`launcher/src/home_page.rs`): Play fills with the preparation ("Preparing 34%");
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
`switches.rs`, `tabs.rs`, `nav.rs`, `sliders.rs`, `items.rs`, `tiles.rs`, `surfaces.rs`,
`hover.rs`, `press.rs`, `markdown.rs`; `mod.rs` holds the look, text, icons and progress) and are
reached as `ui::…`. Each takes its caller's text already translated (the words a component owns,
such as a confirmation's question and verbs, Copy details or Try again, it translates itself) and
reads the frame's look itself; a screen is built from them, never from hand-made copies, and a call site
doesn't restyle one (a variant it needs goes into the component). The look they draw is § Corona's.

- **Buttons** (`ui::button`, `ui::primary(id, icon, label, size)`, `ui::icon_button`,
  `ui::more_button`, `ui::split`): Monocraft at 18 (36 with a 24 px icon at Hero size), uppercase
  only for Primary and Hero; 32, 36, 44 or 56 tall (`ButtonSize`).
  *Primary* is the amber block with its bevel, one per screen: Play on Home (Install Eclipse or Try
  again before the first install), Create on New instance, Start on a stopped server. Empty and
  error states inside a screen that already has one use Secondary (`ui::error_state_with` takes
  the kind of its Try again). Secondary and Ghost labels are mixed case; they and icon buttons'
  icons rest at body ink and turn full under the pointer. *Secondary*, a faint fill, is every other action, in rows, cards and
  headers. *Ghost*, text only, is for Cancel, Close, Dismiss and Not now, and Keep editing (what
  "Discard your changes?" offers instead of Cancel); icon-only buttons (⋯,
  the bin, a folder, Close) are Ghost too, their label the tooltip and the accessible name (the
  one exception is Home's folder beside Play: a filled Secondary, Hero size like Play). An icon
  button that is open or current is selected (the plate and the icon in the mark); a ⋯ shows no
  tooltip while its menu is open. Work under way
  keeps its button and disables it with the label in progress ("Checking…", "Importing…",
  "Loading…"); Play
  fills its block instead (`ui::primary_busy`), its ⋯ on the same track (`ui::more_busy`). A
  busy control is 40 % opaque (`ui::disabled`) and keeps the focus it had;
  `ui::unavailable(control, why)` keeps it a Tab stop and says why (`ui::primary_unavailable` for
  a Primary). Every Primary goes through `ui::primary` / `ui::primary_busy` /
  `ui::primary_unavailable`; `ui::button` refuses the Primary kind in a debug build. A footer's
  buttons are `ui::dialog_foot(dismiss, main)`: the Ghost dismiss (`ui::dialog_dismiss`, at
  `ui::DIALOG_BUTTON`, Lg) first, the Primary last.
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
  strip: it never wraps. Tabs keep their order (the open one may be under More tabs, which then
  shows as current and says so: "More tabs, Folders open"; its menu ticks the open tab and starts
  on it). Only the open tab is a Tab stop (the ⋯ when the open tab is under it); Left, Right, Home
  and End move through every tab and open it. A strip is named by `tab_strip::Strip`; tab ids are
  `{prefix}-{key}` (`ui::value_key`: the value's name in kebab case, as segments' are), and what
  the open tab shows sits in the `{prefix}-panel` tab panel the strip hands back; a count is
  described with its noun ("3 worlds").
  **Segments** (`ui::segments(id, name, ..)`) pick one of a few values on an inset track:
  one Tab stop, the arrows pick, the group named for assistive tech.
- **Switches** (`ui::toggle`, `ui::toggle_row`, `ui::toggle_label`): a blocky switch; in a row the
  whole row is pressed and is the switch.
- **Navigation** (`ui::nav_row` with `NavDensity::Sidebar` or `List`): the sidebar's pages (links;
  the current one says so) and Settings' section list (from a 1440 px window, no card: an in-page
  outline that is a pick list; a tab strip below 1440); a label that doesn't fit gets its tooltip (`ui::tip_when_clipped`, the
  room from `ui::nav_text_room` and `ui::side_row_text_room`). A page's name, title and icon have
  one owner (`Page::label`, `Page::title`, `Page::icon` in `page.rs`).
- **Rows and selection** (`ui::row(id, name, current)`, `ui::side_row(id, name, current)`,
  `ui::list_row`, `ui::choice`, `ui::choice_card`, `ui::list_choice`, `ui::pick_row`,
  `ui::pick_item`, `ui::chip`, `ui::filter_chip`): a row names itself and says whether it is
  current; `ui::selected` and `ui::side_selected` draw what is picked. A list row keeps its fill while the
  pointer is on one of its buttons; `ui::hover_within(&id)` tells its code (one owner of hover:
  `ui/hover.rs`).
- **Lists to pick from** (`ui::options`): one Tab stop for the whole list; the arrows, Page Up and
  Down, Home, End and typed letters move through it, a single choice follows the cursor and a
  multiple one toggles with Space or Enter. Used by New instance's versions, games, loaders, icons
  and colours, the accent swatches, segments, Settings' section list, the sidebar's instances and
  servers (one list with its group headings, `ui::options_with_heads`), the Mods results (a row's
  Install shows on the row under the pointer, the cursor's and the selected one, and is a Tab stop
  only on the cursor's; the others show their state) and the Mods filters. The cursor is kept by
  its row's id, so it stays on the same row when the list changes; the keyboard reveals it with a
  whole row beside it where the list has room; `ui::focus_option(list, at)` takes the keyboard to
  a row once it is drawn (Load more). Shift+F10 or the Menu key opens a sidebar row's ⋯ menu.
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
  `Launcher::open_menu_from` / `open_menu_for`. Every menu is named for assistive tech after its
  opener (`menu_popover(id, label, cx)`: "Account", "{name} options", "Play options"); Down on a
  focused dropdown opens its menu. Focus only returns to an opener that is still drawn.
- **Dialogs** (`ui::modal`, `ui::sheet`, `ui::dialog`): the launcher dimmed under a plate, or a
  sheet over the whole window. `ui::dialog(id, eyebrow, title, DialogWidth::Narrow | Wide, body,
  foot)` owns the plate, its width, padding and title (a heading, 27); its foot is what the caller
  passes: `ui::dialog_foot(dismiss, main)`, or the answers themselves (the data collection
  question's). Tab stays inside, focus moves in when it opens and back to what opened it when
  it closes.
- **States in a pane** (`ui::empty_state`, `ui::loading_state`, `ui::error_state`), centred in a
  card: a 36 icon tile, a title at 18, one sentence that ends with the next step, then the actions;
  a state that is a page's only content passes `ui::page_only(title)` (a 48 tile, title 27). An
  error state is an alert for assistive tech; its row is Try again first, the buttons that fix it,
  then Copy details last, and wraps as a whole under the cause. A line inside a
  card that is already there (nothing yet, a server that is stopped) is `ui::empty(id, text)` /
  `ui::empty_line(id, text)`.
- **One presentation for a failure**: a plain title, one plain cause line, the actions that fix it
  and Copy details with the whole error. In a pane `ui::error_state` / `ui::error_state_with`
  (the cause is the error's first line when that is a sentence, `ui::told_cause`, else no cause
  line) or `ui::error_state_told` (a cause written for the player: what a crash or a failed setup
  says); in a card `ui::error_line_for(id, title, error, retry, extra)`, Copy details last in its
  row. Never a raw
  multi-line error on screen. **Progress** is one line (`ui::progress_line`): what runs and its
  percent, the eased bar, the step it is on.
- **Notices** (`ui::notice(Notice::Info | Warn | InCard, ..)`, `ui::notice_line(tone, ..)`,
  `ui::error_note`, `ui::error_line`): an icon or the state dot and a line on a
  faint (or warn) fill, one padding (`NOTICE_PAD`), its actions with Cancel first; a notice that is
  a card's whole content is drawn flat in it (`InCard`); an error is never red text alone. A toast says what just happened for
  6 s (`toast::say`), an error until it is dismissed (`toast::say_error`).
- **Tiles and faces** (`ui::tile`, `ui::mark_tile`, `ui::face`, `ui::picture_button`): 24, 36 or
  48 (`TILE_SM`, `TILE_MD`, `TILE_LG`), a mark 32 in a 48 tile, the account's face 80 (`FACE_XL`),
  all at radius 8. Every pressable picture (gallery, screenshots, icons) is a picture button, its
  tooltip under it; a picture still loading sits in `ui::picture_slot()`. Faces are drawn by one renderer (`profile_heads`).
- **Wells** (`ui::well`, `ui::code_well`, `ui::log_well`): the inset well, a code block, a log.
- **Progress** (`ui::progress_line`, `ui::progress_bar`): a 4 px track under a label and percent;
  unknown progress shuttles, and it stands still under reduced motion.
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
  (`PlayState`, `play_state.rs`), which Home's head and the sidebar read; whether a game runs or
  starts is `GameRun::active()`; what went wrong with the last start is `GameProblem` (needs an
  owner, failed, or a notice); an Eclipse instance's state line is `InstallState::line()`, in the
  sidebar and Settings › Updates alike. Which version plays is one model: an instance follows the
  launcher's default version (`Launcher::default_version`, kept on the newest as versions install)
  or is pinned to one; `Launcher::selected_version` is what the selected instance plays and
  `Launcher::use_version(tag)` (Versions' Use, Home's version list) pins it, or lets it follow when
  `tag` is the default. Versions and installs live in `Launcher::versions` (`versions_state.rs`),
  the update check in `UpdateCheck`; the last
  preparation, its error and the Console's lines belong to the instance that started them
  (`GameRun::begin`, `console_log`, `shown_prepare_error`); a version's name is
  `nightly::display_name`; who plays is `Launcher::who_plays`.
- **Element ids** are stable and descriptive: keyed by the enum's value in kebab case
  (`ui::value_key`: `home-tab-worlds`) or by a list item's name, path or id, never by an English
  label or a position.

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
and `side`. Text sits on the top `UNCARDED_STOPS` bands without a card; Corona's ink levels are
held to AA there (`every_ink_level_reads_on_every_surface`). `tokens.css` carries the bands, the
page and the sidebar as `--night-sky-0` to `--night-sky-5`, `--night-page` and `--night-side` for
website pages that wear the scene. The values are in `crates/eclipse-theme/src/night.rs`.

## Blocks

A blocky, Minecraft-like look in Eclipse's purple, for the website and the game's graphics
settings screen:
Minecraft's inventory rather than a document. Its colours are `eclipse_theme::Blocks`
(`BLOCKS_LIGHT`, `BLOCKS_DARK`), which `tokens.css` carries under the same names as before plus
`--panel`, `--raised`, `--button`, `--well`, `--bevel`, `--lip`, `--outline` and the `--face`
of the main action. The game's overlays keep `DARK`.

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
- **Pixel art drawn in code.** The stone behind the pages is close shades of the page colour,
  never Minecraft's stone; the landscape is a banded sky, a sun cut in slices like
  the mark, hills and square trees (a sunset in dark, a dawn in light). The website draws them in
  its `scripts/art.mjs`.
- **Icons**: Eclipse's 16 px pixel SVGs in `crates/eclipse-theme/icons/pixel/`, tinted with the
  text colour.
- **Contrast**: every text colour meets WCAG AA (4.5:1) on the panels and the page in both
  palettes (`blocks_text_meets_wcag_aa_in_both_appearances`).

## Game menus

The game's few overlays (the on-screen keyboard a controller types with) and the website's type,
spacing and motion read the crate's first tokens: `DARK`, cool neutrals with one violet accent,
and the steps below. `tokens.css` carries the steps, spacing and motion for the website.

### Colour

Every text colour passes WCAG AA (4.5:1) on every background it is used on; a test in
`eclipse-theme` checks each pair.

| Token        | Dark      | Use |
|--------------|-----------|-----|
| `page`       | `#0d0e13` | the background |
| `plate_soft` | `#181a22` | hover plate, fields, segmented track, console |
| `plate`      | `#20222c` | pressed plate, switch off, the faint wordmark |
| `surface`    | `#16181f` | dialogs and menus that sit above the page |
| `rule`       | `#24262f` | 1 px rules between rows, tracks |
| `ink`        | `#ecedf3` | text, selection, slider fill, the inverted segment |
| `ink_soft`   | `#c5c7d1` | secondary text, values |
| `muted`      | `#8b8f9c` | labels, descriptions, mono indices and details |
| `accent`     | `#9a86ff` | the main action, links, focus, progress |
| `on_accent`  | `#0d0e13` | text on the accent |
| `danger`     | `#f07a68` | errors, the crash notice's dot |
| `success`    | `#45c27a` | "Matches vanilla", "Ready" |

The overlays draw over the game; their plates are the dark `ink` at 8 % and 14 % rather than
`plate_soft` and `plate`, so they stay visible over a bright world.

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
- Heights: control 32, large control 40, segment 28, list row 86 (web).

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
