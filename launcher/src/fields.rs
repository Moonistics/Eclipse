use super::line_edit::{self, Key, LineEdit};
use super::*;
use crate::ui;
use eclipse_theme::glass::{radius, step};

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
    /// The New instance page's (`instance_state.rs`).
    InstanceName,
    InstanceGroup,
    /// Its search in Mojang's versions (`new_instance.rs`).
    VersionSearch,
    /// Home's Mods tab's filter (`instance_mods.rs`).
    InstanceModsFilter,
    /// Home's Multiplayer section's (`multiplayer_page.rs`).
    MultiplayerName,
    MultiplayerAddress,
    /// Home's Console's search (`console_page.rs`).
    ConsoleSearch,
    /// The Launch panel's (`launch_page.rs`).
    PreLaunch,
    LaunchWrapper,
    LaunchEnv,
    PostExit,
    /// Home's memory row (`launch_page.rs`).
    MemoryLimit,
    /// Settings' new offline name (`switch_page.rs`).
    OfflineName,
    /// Settings' accent as hex (`themes_page.rs`).
    AccentHex,
    InviteCode,
    ModsSearch,
    GithubLink,
    GameArgs,
    Proxy,
    PaletteSearch,
}

impl Field {
    const ALL: [Field; 29] = [
        Field::NewServerName,
        Field::NewServerSeed,
        Field::NewServerFriend,
        Field::ServerCommand,
        Field::ServerPlayer,
        Field::ServerMotd,
        Field::ServerPort,
        Field::TailscaleToken,
        Field::FriendEmail,
        Field::InstanceName,
        Field::InstanceGroup,
        Field::VersionSearch,
        Field::InstanceModsFilter,
        Field::MultiplayerName,
        Field::MultiplayerAddress,
        Field::ConsoleSearch,
        Field::PreLaunch,
        Field::LaunchWrapper,
        Field::LaunchEnv,
        Field::PostExit,
        Field::MemoryLimit,
        Field::OfflineName,
        Field::AccentHex,
        Field::InviteCode,
        Field::ModsSearch,
        Field::GithubLink,
        Field::GameArgs,
        Field::Proxy,
        Field::PaletteSearch,
    ];

    fn empty(self) -> LineEdit {
        let (max, visible) = match self {
            Field::InviteCode => (256, 32),
            Field::GameArgs => (1024, 38),
            Field::Proxy => (256, 44),
            Field::ModsSearch => (100, 40),
            Field::GithubLink => (300, 40),
            Field::PaletteSearch => (80, 40),
            _ => return LineEdit::default(),
        };
        LineEdit::new(String::new(), max, visible)
    }
}

#[derive(Default)]
pub(super) struct Fields {
    texts: HashMap<Field, LineEdit>,
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

    pub(super) fn handle(&mut self, f: Field, cx: &mut App) -> FocusHandle {
        self.focus
            .entry(f)
            .or_insert_with(|| cx.focus_handle().tab_index(0).tab_stop(true))
            .clone()
    }

    /// Which field has the keyboard, read every frame.
    pub(super) fn note_focus(&mut self, window: &mut Window, cx: &mut App) {
        if let Some(h) = self.focus_soon.take().and_then(|f| self.focus.get(&f)) {
            window.focus(h, cx);
        }
        self.focused = Field::ALL
            .into_iter()
            .find(|f| self.focus.get(f).is_some_and(|h| h.is_focused(window)));
        if self.focused.is_some() && self.focused != self.selected {
            self.selected = None;
        }
    }
}

impl Launcher {
    /// A one-line text field: typing, paste, Backspace, Enter to confirm, Escape, Tab to the next.
    pub(super) fn field(
        &mut self,
        f: Field,
        placeholder: &'static str,
        width: f32,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let focused = self.fields.has_focus(f);
        let selected = focused && self.fields.selected == Some(f);
        let invalid = self.fields.invalid.is_some_and(|(g, _)| g == f);
        self.fields.text(f).show(((width - 24.) / 7.5) as usize);
        // The mods filter and the log search sit by buttons: small fields, as tall as they are.
        let base = ui::field(focused, invalid)
            .when(
                matches!(
                    f,
                    Field::InstanceModsFilter
                        | Field::ConsoleSearch
                        | Field::ServerMotd
                        | Field::ServerPort
                        | Field::TailscaleToken
                        | Field::ServerPlayer
                ),
                |d| {
                    d.h(px(ui::Size::Md.height()))
                        .px(px(12.))
                        .rounded(px(radius::CONTROL))
                },
            )
            // The instance and server dialogs' are large, with larger text.
            .when(
                !invalid
                    && matches!(
                        f,
                        Field::InstanceName
                            | Field::InstanceGroup
                            | Field::VersionSearch
                            | Field::NewServerName
                            | Field::NewServerSeed
                    ),
                |d| d.h(px(48.)).text_size(px(step::LEAD.size)),
            )
            .when(f == Field::AccentHex, |d| {
                d.h(px(ui::Size::Md.height()))
                    .px(px(12.))
                    .rounded(px(radius::CONTROL))
                    .text_size(px(step::NOTE.size))
                    .font_weight(FontWeight::MEDIUM)
            })
            // The friend field sits in its chip, on the chip's fill.
            .when(f == Field::NewServerFriend, |d| {
                d.h_full()
                    .px(px(14.))
                    .border_0()
                    .bg(gpui::transparent_black())
            })
            // The server's command line: the field is the line's middle, on its fill, in mono.
            .when(f == Field::ServerCommand, |d| {
                d.h_full()
                    .px_0()
                    .rounded_none()
                    .border_0()
                    .bg(gpui::transparent_black())
                    .font_family(theme::MONO)
                    .text_size(px(step::NOTE.size))
            })
            // The focus ring on a field of its own, drawn inside its edge: GPUI has no outline.
            .when(
                focused && !invalid && !matches!(f, Field::NewServerFriend | Field::ServerCommand),
                |d| {
                    let small = matches!(
                        f,
                        Field::InstanceModsFilter
                            | Field::ConsoleSearch
                            | Field::AccentHex
                            | Field::ServerMotd
                            | Field::ServerPort
                            | Field::TailscaleToken
                            | Field::ServerPlayer
                    );
                    d.child(ui::focus_ring(if small {
                        radius::CONTROL
                    } else {
                        radius::ROW
                    }))
                },
            );
        let content = line_edit::content(
            self.fields.text(f),
            focused,
            selected,
            f == Field::TailscaleToken,
            placeholder,
        );
        let field = base
            .id(SharedString::from(format!("field-{f:?}")))
            .w(px(width))
            .cursor_text()
            .child(content);
        self.text_input(f, field, cx)
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
                if selected {
                    this.fields.text(f).select_all();
                }
                match line_edit::handle(this.fields.text(f), ev, cx) {
                    Key::Enter if matches!(f, Field::InstanceName | Field::InstanceGroup) => {
                        this.save_instance(cx)
                    }
                    Key::Enter => this.field_enter(f),
                    Key::Edited => this.field_edited(f),
                    Key::Escape => {
                        if this.servers.create.is_some() && !this.servers.creating {
                            this.servers.create = None;
                        }
                        this.instance_escape();
                        window.blur(cx);
                    }
                    // Tab and the rest go on to the window: Tab moves the focus.
                    Key::Pass => return,
                    _ => {}
                }
                cx.stop_propagation();
                cx.notify();
            }));
        line_edit::mouse(el, cx, move |this| this.fields.text(f))
    }

    pub(super) fn field_enter(&mut self, f: Field) {
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
            Field::InstanceName
            | Field::InstanceGroup
            | Field::VersionSearch
            | Field::InstanceModsFilter
            | Field::ConsoleSearch
            | Field::AccentHex
            | Field::GameArgs
            | Field::Proxy
            | Field::PaletteSearch => {}
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
            _ => {}
        }
    }
}
