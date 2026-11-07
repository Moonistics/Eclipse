use super::line_edit::{self, Key, LineEdit};
use super::*;
use crate::ui;
use eclipse_theme::glass::{space, step, GEIST_ADVANCE};
use std::cell::Cell;
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(super) enum Field {
    NewServerName,
    NewServerSeed,
    NewServerFriend,
    ServerCommand,
    ServerPlayer,
    ServerMotd,
    ServerPort,
    TailscaleToken,
    FriendEmail,
    /// The New instance page's (`instance_page.rs`).
    InstanceName,
    InstanceGroup,
    /// Its search in Mojang's versions (`instance_versions.rs`).
    VersionSearch,
    /// Home's Mods tab's filter (`home_mods.rs`).
    InstanceModsFilter,
    /// Home's Multiplayer tab's (`home_lists.rs`).
    MultiplayerName,
    MultiplayerAddress,
    /// Home's Console's search (`console_page.rs`).
    ConsoleSearch,
    /// The Launch panel's (`launch_page.rs`).
    PreLaunch,
    LaunchWrapper,
    LaunchEnv,
    PostExit,
    /// Edit instance's memory row (`launch_page.rs`).
    MemoryLimit,
    /// Settings' new offline name (`settings_account.rs`).
    OfflineName,
    /// Settings' accent as hex (`settings_appearance.rs`).
    AccentHex,
    InviteCode,
    ModsSearch,
    GithubLink,
    GameArgs,
    Proxy,
    PaletteSearch,
}

impl Field {
    fn name(self) -> &'static str {
        match self {
            Field::NewServerName | Field::MultiplayerName => tr("Server name"),
            Field::NewServerSeed => tr("World seed"),
            Field::NewServerFriend | Field::ServerPlayer => tr("Player name"),
            Field::ServerCommand => tr("Server command"),
            Field::ServerMotd => tr("Server description (MOTD)"),
            Field::ServerPort => tr("Port"),
            Field::TailscaleToken => tr("Tailscale API token"),
            Field::FriendEmail => tr("Friend's email"),
            Field::InstanceName => tr("Name"),
            Field::InstanceGroup => tr("Group (optional)"),
            Field::VersionSearch => tr("Search versions"),
            Field::InstanceModsFilter => tr("Filter installed"),
            Field::MultiplayerAddress => tr("Server address"),
            Field::ConsoleSearch => tr("Search the log"),
            Field::PreLaunch => tr("Command before the game"),
            Field::LaunchWrapper => tr("Run the game through"),
            Field::LaunchEnv => tr("Environment variables"),
            Field::PostExit => tr("Command after the game"),
            Field::MemoryLimit => tr("Maximum memory"),
            Field::OfflineName => tr("Offline name"),
            Field::AccentHex => tr("Accent colour (hex)"),
            Field::InviteCode => tr("Invite code"),
            Field::ModsSearch => tr("Search mods"),
            Field::GithubLink => tr("GitHub repository link"),
            Field::GameArgs => tr("Eclipse game arguments"),
            Field::Proxy => tr("Proxy"),
            Field::PaletteSearch => tr("Search commands"),
        }
    }

    fn holds_code(self) -> bool {
        matches!(self, Field::ServerCommand)
    }

    fn empty(self) -> LineEdit {
        let max = match self {
            Field::InviteCode | Field::Proxy => 256,
            Field::GameArgs => 1024,
            Field::ModsSearch => 100,
            Field::GithubLink => 300,
            Field::PaletteSearch => 80,
            _ => return LineEdit::default(),
        };
        LineEdit::with_max(max)
    }
}

#[derive(Default)]
pub(super) struct Fields {
    texts: HashMap<Field, LineEdit>,
    widths: HashMap<Field, Rc<Cell<f32>>>,
    focus: HashMap<Field, FocusHandle>,
    focused: Option<Field>,
    /// The field to put the keyboard in on the next frame.
    pub(super) focus_soon: Option<Field>,
    /// A suggested value shown selected: the first key typed replaces it.
    pub(super) selected: Option<Field>,
    /// A field whose value was refused, and why: shown under it until it's edited.
    pub(super) invalid: Option<(Field, &'static str)>,
}

impl Fields {
    pub(super) fn text(&mut self, f: Field) -> &mut LineEdit {
        self.texts.entry(f).or_insert_with(|| f.empty())
    }

    pub(super) fn value(&self, f: Field) -> &str {
        self.texts.get(&f).map_or("", LineEdit::text)
    }

    pub(super) fn holds(&self, f: Field) -> bool {
        self.texts.contains_key(&f)
    }

    pub(super) fn clear(&mut self, f: Field) {
        self.texts.remove(&f);
    }

    /// Mark a field invalid without an explanatory message, or clear its previous error.
    pub(super) fn set_field_invalid(&mut self, field: Field, invalid: bool) {
        if invalid {
            self.invalid = Some((field, ""));
        } else if self.invalid.is_some_and(|(current, _)| current == field) {
            self.invalid = None;
        }
    }

    /// Whether this field has the keyboard, as of this frame.
    pub(super) fn has_focus(&self, f: Field) -> bool {
        self.focused == Some(f)
    }

    pub(super) fn any_focus(&self) -> bool {
        self.focused.is_some()
    }

    pub(super) fn handle(&mut self, f: Field, cx: &mut App) -> FocusHandle {
        self.focus
            .entry(f)
            .or_insert_with(|| cx.focus_handle().tab_index(0).tab_stop(true))
            .clone()
    }

    /// Which field has the keyboard, read every frame.
    pub(super) fn note_focus(&mut self, window: &mut Window, cx: &mut App) {
        if let Some(f) = self.focus_soon {
            match self.focus.get(&f) {
                Some(h) => {
                    window.focus(h, cx);
                    self.focus_soon = None;
                }
                None => window.refresh(),
            }
        }
        self.focused = self
            .focus
            .iter()
            .find(|(_, h)| h.is_focused(window))
            .map(|(f, _)| *f);
        if self.focused.is_some() && self.focused != self.selected {
            self.selected = None;
        }
    }
}

const TRAILING_INSET: f32 = space::SM + space::XXS;

fn visible_chars(room: f32, size: ui::ButtonSize) -> usize {
    let text = size.field_text();
    (room / (GEIST_ADVANCE * text.size / step::BODY.size)).max(1.) as usize
}

impl Launcher {
    /// A one-line text field: typing, paste, Backspace, Enter to confirm, Escape, Tab to the next.
    pub(super) fn field(
        &mut self,
        f: Field,
        placeholder: impl Into<SharedString>,
        width: f32,
        size: ui::ButtonSize,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        self.field_with(f, None, placeholder, width, size, None, cx)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn field_with(
        &mut self,
        f: Field,
        lead: Option<AnyElement>,
        placeholder: impl Into<SharedString>,
        width: f32,
        size: ui::ButtonSize,
        trailing: Option<AnyElement>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let focused = self.fields.has_focus(f);
        let selected = focused && self.fields.selected == Some(f);
        let refused = self.fields.invalid.filter(|(g, _)| *g == f);
        let invalid = refused.is_some();
        let secret = f == Field::TailscaleToken;
        let placeholder: SharedString = placeholder.into();
        let measured = self.fields.widths.entry(f).or_default().clone();
        let room = match measured.get() {
            seen if seen > 0. => seen,
            _ => width - 2. * size.field_pad(),
        };
        self.fields.text(f).show(visible_chars(room, size));
        let content = line_edit::content_with(
            self.fields.text(f),
            focused,
            selected,
            secret,
            placeholder.clone(),
        );
        let field = ui::field(focused, invalid, size)
            .id(SharedString::from(format!("field-{f:?}")))
            .role(gpui::Role::TextInput)
            .aria_label(f.name())
            .aria_placeholder(placeholder)
            .when_some(refused.filter(|(_, why)| !why.is_empty()), |d, (_, why)| {
                d.aria_description(tr(why))
            })
            .when(!secret, |d| d.aria_value(self.fields.value(f).to_string()))
            .w(px(width))
            .when(f.holds_code(), |d| ui::in_mono(d, step::NOTE))
            .cursor_text()
            .children(lead.map(|l| div().relative().flex_none().child(l)))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .child(ui::measure(move |bounds| {
                        measured.set(f32::from(bounds.size.width))
                    }))
                    .child(content),
            )
            .when(trailing.is_some(), |d| d.pr(px(TRAILING_INSET)))
            .children(trailing.map(|t| div().relative().flex_none().child(t)));
        self.text_input(f, field, cx)
    }

    pub(super) fn name_adder(
        &mut self,
        f: Field,
        id: &str,
        width: f32,
        can_add: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let add = ui::button(
            format!("{id}-add"),
            Some("plus-sign"),
            tr("Add"),
            ui::ButtonKind::Secondary,
            ui::ButtonSize::Sm,
        );
        let add = if can_add {
            add.on_press(cx.listener(move |this, _: &ClickEvent, window, cx| {
                this.field_enter(f, window, cx);
                cx.notify();
            }))
        } else {
            ui::disabled(add)
        };
        self.field_with(
            f,
            None,
            tr("A player's name"),
            width,
            ui::ButtonSize::Md,
            Some(add.into_any_element()),
            cx,
        )
    }

    pub(super) fn text_input(
        &mut self,
        f: Field,
        el: Stateful<Div>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let handle = self.fields.handle(f, cx);
        let el = el
            .track_focus(&handle)
            .on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| {
                // A click puts the caret in: the suggestion is no longer selected.
                if matches!(ev, ClickEvent::Mouse(_)) {
                    this.fields.selected = None;
                }
                window.focus(&this.fields.handle(f, cx), cx);
                cx.notify();
            }))
            .on_key_down(cx.listener(move |this, ev: &KeyDownEvent, window, cx| {
                if ev.keystroke.key != "enter" && this.fields.invalid.is_some_and(|(g, _)| g == f) {
                    this.fields.invalid = None;
                }
                // A suggestion is selected: what's typed or pasted replaces it.
                let selected = this.fields.selected.take() == Some(f);
                if selected && ev.keystroke.key != "escape" {
                    this.fields.text(f).select_all();
                }
                match line_edit::handle(this.fields.text(f), ev, cx) {
                    Key::Enter => this.field_enter(f, window, cx),
                    Key::Edited => this.field_edited(f),
                    Key::Escape if !selected && ui::modal_open() => return,
                    // Tab and the rest go on to the window: Tab moves the focus.
                    Key::Pass => return,
                    _ => {}
                }
                cx.stop_propagation();
                cx.notify();
            }));
        let el = ui::reveal_on_tab(el);
        line_edit::mouse(el, cx, move |this| this.fields.text(f))
    }

    pub(super) fn field_error(&self, f: Field) -> Option<Div> {
        let (_, why) = self
            .fields
            .invalid
            .filter(|(g, why)| *g == f && !why.is_empty())?;
        Some(ui::error_note(tr(why)))
    }

    pub(super) fn field_enter(&mut self, f: Field, window: &mut Window, cx: &mut Context<Self>) {
        match f {
            Field::NewServerName
            | Field::NewServerSeed
            | Field::NewServerFriend
            | Field::ServerCommand
            | Field::ServerPlayer
            | Field::ServerMotd
            | Field::ServerPort => self.server_field_enter(f),
            Field::TailscaleToken => self.save_token(),
            Field::FriendEmail => self.invite_friend(),
            Field::InstanceName | Field::InstanceGroup => self.save_instance(cx),
            Field::VersionSearch
            | Field::InstanceModsFilter
            | Field::ConsoleSearch
            | Field::AccentHex
            | Field::GameArgs
            | Field::Proxy => {}
            Field::PaletteSearch => self.palette_enter(window, cx),
            Field::PreLaunch | Field::LaunchWrapper | Field::LaunchEnv | Field::PostExit => {
                self.save_launch()
            }
            Field::MemoryLimit => self.memory_enter(),
            Field::OfflineName => self.offline_name_enter(),
            Field::MultiplayerName | Field::MultiplayerAddress => self.add_server(),
            Field::InviteCode => self.join_friend(),
            Field::ModsSearch => self.mods_search(0),
            Field::GithubLink => self.github_add(),
        }
    }

    fn field_edited(&mut self, f: Field) {
        match f {
            Field::GameArgs => {
                self.settings.game_args = self.fields.value(Field::GameArgs).to_string();
                self.save_settings();
            }
            Field::Proxy => self.save_proxy(),
            Field::PaletteSearch => self.palette_edited(),
            Field::InviteCode => {
                let code = self.fields.value(Field::InviteCode).trim();
                let wrong = !code.is_empty() && eclipse_friends::invite::parse(code).is_none();
                self.fields.invalid = if wrong {
                    Some((f, tr("That isn't an invite code.")))
                } else {
                    self.fields.invalid.filter(|(g, _)| *g != f)
                };
            }
            _ => {}
        }
    }
}
