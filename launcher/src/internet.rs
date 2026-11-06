//! The Servers page's "Friends over the internet" panel: Tailscale on this computer, the address friends
//! type, inviting a friend to this one computer, and keeping shared friends to the server's port
//! (`eclipse_launcher::tailscale`). Signing in is always the player's, in Tailscale's own app or page;
//! the launcher reads `tailscale status` and, with the player's own access token, calls the API.
use super::fields::Field;
use super::*;
use crate::ui;
use eclipse_launcher::tailscale::{self, Api, Invite, Reach, State};
use eclipse_theme::glass::{ink, radius, space, step};
use serde_json::Value;

/// How often Tailscale's state is read while the panel shows.
const POLL: Duration = Duration::from_secs(10);
/// How often the invitations and access controls are read again while the panel shows, so an
/// accepted invitation appears without a click.
const RELOAD: Duration = Duration::from_secs(30);

pub(super) enum Event {
    State(State),
    Token(Option<String>),
    Invites(Result<Vec<Invite>, String>),
    Policy(Result<(Value, Option<String>), String>),
    /// An action finished: what to tell the player, and whether to read invites and policy again.
    Done(Result<String, String>),
    Invited(Result<Invite, String>),
}

pub(super) struct Internet {
    state: Option<State>,
    reading: bool,
    read: Option<Instant>,
    /// The access token, once read from the keychain (None: not read yet; Some(None): none).
    token: Option<Option<String>>,
    asking_token: bool,
    invites: Option<Result<Vec<Invite>, String>>,
    policy: Option<Result<(Value, Option<String>), String>>,
    loaded: Option<Instant>,
    busy: bool,
    message: Option<Result<String, String>>,
    /// The invitation just made, its link shown to copy.
    fresh: Option<Invite>,
    confirm_limit: bool,
    copied: Option<(String, Instant)>,
    tx: Sender<Event>,
    rx: Receiver<Event>,
}

impl Internet {
    pub(super) fn new() -> Self {
        let (tx, rx) = channel();
        Self {
            state: None,
            reading: false,
            read: None,
            token: None,
            asking_token: false,
            invites: None,
            policy: None,
            loaded: None,
            busy: false,
            message: None,
            fresh: None,
            confirm_limit: false,
            copied: None,
            tx,
            rx,
        }
    }

    fn node(&self) -> Option<&tailscale::Node> {
        match &self.state {
            Some(State::Running(n)) => Some(n),
            _ => None,
        }
    }

    fn token(&self) -> Option<String> {
        self.token.clone().flatten()
    }
}

impl Launcher {
    /// What the background sent, and the next look at Tailscale while the panel shows.
    pub(super) fn internet_tick(&mut self, showing: bool) -> bool {
        let mut changed = false;
        while let Ok(event) = self.internet.rx.try_recv() {
            changed = true;
            self.internet_event(event);
        }
        let i = &mut self.internet;
        if showing && !i.reading && i.read.is_none_or(|at| at.elapsed() >= POLL) {
            self.internet_refresh();
        }
        changed
    }

    fn internet_refresh(&mut self) {
        let i = &mut self.internet;
        i.reading = true;
        i.read = Some(Instant::now());
        let tx = i.tx.clone();
        let want_token = i.token.is_none();
        std::thread::spawn(move || {
            let _ = tx.send(Event::State(tailscale::status()));
            if want_token {
                let token = account::keychain().get(tailscale::TOKEN_KEY).ok().flatten();
                let _ = tx.send(Event::Token(token));
            }
        });
    }

    fn internet_event(&mut self, event: Event) {
        let i = &mut self.internet;
        match event {
            Event::State(state) => {
                i.reading = false;
                let was_running = i.node().map(|n| n.id.clone());
                i.state = Some(state);
                let now = i.node().map(|n| n.id.clone());
                let stale = i.loaded.is_none_or(|at| at.elapsed() >= RELOAD);
                if now.is_some() && (now != was_running || (stale && !i.busy)) {
                    self.internet_load();
                }
            }
            Event::Token(token) => {
                i.token = Some(token);
                self.internet_load();
            }
            Event::Invites(r) => i.invites = Some(r),
            Event::Policy(r) => i.policy = Some(r),
            Event::Done(r) => {
                i.busy = false;
                i.message = Some(r);
                self.internet_load();
            }
            Event::Invited(r) => {
                i.busy = false;
                match r {
                    Ok(invite) => {
                        i.message = None;
                        i.fresh = Some(invite);
                        self.fields.text(Field::FriendEmail).set(String::new());
                    }
                    Err(e) => i.message = Some(Err(e)),
                }
                self.internet_load();
            }
        }
    }

    /// Reads the invitations and the access controls (needs the token and a running Tailscale).
    fn internet_load(&mut self) {
        let i = &mut self.internet;
        let (Some(token), Some(node)) = (i.token(), i.node().cloned()) else {
            return;
        };
        i.loaded = Some(Instant::now());
        let tx = i.tx.clone();
        std::thread::spawn(move || {
            let api = Api::new(&token);
            let _ = tx.send(Event::Invites(api.invites(&node.id)));
            let _ = tx.send(Event::Policy(api.policy()));
        });
    }

    pub(super) fn save_token(&mut self) {
        let token = self.fields.value(Field::TailscaleToken).trim().to_string();
        if !token.starts_with("tskey-") {
            self.internet.message = Some(Err("Paste your access token (tskey-…)".into()));
            return;
        }
        self.fields.text(Field::TailscaleToken).set(String::new());
        self.internet.asking_token = false;
        self.internet.message = match account::keychain().set(tailscale::TOKEN_KEY, &token) {
            Ok(()) if account::keychain().persistent() => None,
            Ok(()) => Some(Ok(
                "The token can't be saved on this computer. You'll need it again next time.".into(),
            )),
            Err(e) => Some(Err(e)),
        };
        self.internet.token = Some(Some(token));
        self.internet_load();
    }

    fn forget_token(&mut self) {
        let _ = account::keychain().delete(tailscale::TOKEN_KEY);
        let i = &mut self.internet;
        i.token = Some(None);
        i.invites = None;
        i.policy = None;
        i.fresh = None;
        i.message = Some(Ok(
            "The access token is forgotten. Delete it in Tailscale's admin \
                             console too (Settings, Keys) if you won't use it again."
                .into(),
        ));
    }

    pub(super) fn invite_friend(&mut self) {
        let i = &mut self.internet;
        let (Some(token), Some(node)) = (i.token(), i.node().cloned()) else {
            return;
        };
        if i.busy {
            return;
        }
        let email = self.fields.value(Field::FriendEmail).trim().to_string();
        if !email.is_empty() && !email.contains('@') {
            i.message = Some(Err("That isn't an email address.".into()));
            return;
        }
        i.busy = true;
        i.fresh = None;
        let tx = i.tx.clone();
        std::thread::spawn(move || {
            let email = Some(email.as_str()).filter(|e| !e.is_empty());
            let _ = tx.send(Event::Invited(Api::new(&token).invite(&node.id, email)));
        });
    }

    fn internet_action(
        &mut self,
        job: impl FnOnce(&Api) -> Result<String, String> + Send + 'static,
    ) {
        let i = &mut self.internet;
        let Some(token) = i.token() else {
            return;
        };
        if i.busy {
            return;
        }
        i.busy = true;
        i.message = None;
        let tx = i.tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Event::Done(job(&Api::new(&token))));
        });
    }

    /// Applies `change` to the access controls read at `etag`, checked by Tailscale first.
    fn change_policy(&mut self, new: Value, etag: Option<String>, said: &'static str) {
        self.internet.confirm_limit = false;
        self.internet_action(move |api| {
            api.validate(&new)?;
            api.set_policy(&new, etag.as_deref())?;
            Ok(said.to_string())
        });
    }

    fn copy_button(
        &self,
        id: &'static str,
        text: String,
        kind: ButtonKind,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let copied = self
            .internet
            .copied
            .as_ref()
            .is_some_and(|(what, at)| what == &text && at.elapsed() < Duration::from_secs(2));
        ui::button(
            id,
            None,
            tr(if copied { "Copied" } else { "Copy" }),
            kind,
            ui::ButtonSize::Md,
        )
        .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
            clipboard::copy_to_clipboard(cx, gpui::ClipboardItem::new_string(text.clone()));
            this.internet.copied = Some((text.clone(), Instant::now()));
            cx.notify();
        }))
    }

    /// The Servers page's "Over the internet": the Tailscale address once it runs, else what
    /// to do about it.
    pub(super) fn internet_card(&mut self, port: u16, cx: &mut Context<Self>) -> Div {
        let d = div()
            .flex()
            .flex_col()
            .child(ui::label(tr("Over the internet")));
        let check = ui::button(
            "ts-check",
            None,
            tr("Check again"),
            ButtonKind::Secondary,
            ui::ButtonSize::Md,
        )
        .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.internet_refresh();
            cx.notify();
        }));
        let act = |b: Stateful<Div>, check: Stateful<Div>| {
            div()
                .mt(px(space::MD))
                .flex()
                .flex_wrap()
                .gap(px(space::SM))
                .child(b)
                .child(check)
        };
        match self.internet.state.clone() {
            None => d.child(ui::card_note(tr("Looking for Tailscale…")).mt(px(space::XS))),
            Some(State::Missing) => d
                .child(ui::card_note(trf(
                    "Install Tailscale so friends anywhere can join, or forward TCP port {port} to this computer.",
                    &[("port", &port.to_string())],
                )).mt(px(space::XS)))
                .child(act(
                    ui::button("ts-install", None, tr("Install Tailscale"), ButtonKind::Secondary, ui::ButtonSize::Md)
                        .on_press(|_, _, cx| cx.open_url(tailscale::install_url())),
                    check,
                )),
            Some(State::Off(_)) => d
                .child(ui::card_note(tr("Tailscale is installed but not running. Open it and switch it on.")).mt(px(space::XS)))
                .child(act(
                    ui::button("ts-open", None, tr("Open Tailscale"), ButtonKind::Secondary, ui::ButtonSize::Md).on_press(cx.listener(
                        |this, _: &ClickEvent, _, cx| {
                            if let Err(e) = tailscale::open_app() {
                                this.internet.message = Some(Err(e));
                            }
                            this.internet.read = None;
                            cx.notify();
                        },
                    )),
                    check,
                )),
            Some(State::SignedOut { auth_url }) => d
                .child(ui::card_note(tr("Tailscale is installed. Sign in to Tailscale to continue.")).mt(px(space::XS)))
                .child(act(
                    ui::button("ts-login", None, tr("Sign in to Tailscale"), ButtonKind::Secondary, ui::ButtonSize::Md).on_press(
                        cx.listener(move |this, _: &ClickEvent, _, cx| {
                            match &auth_url {
                                Some(url) => cx.open_url(url),
                                None => {
                                    if let Err(e) = tailscale::open_app() {
                                        this.internet.message = Some(Err(e));
                                    }
                                }
                            }
                            this.internet.read = None;
                            cx.notify();
                        }),
                    ),
                    check,
                )),
            Some(State::Running(_)) => {
                let node = self.internet.node().cloned().unwrap_or_default();
                let address = node.address(port);
                let copied = self.internet.copied.as_ref().is_some_and(|(what, at)| {
                    *what == address && at.elapsed() < Duration::from_secs(2)
                });
                let text = address.clone();
                let copy = ui::icon_button("ts-copy", if copied { "tick-02" } else { "copy-01" }, tr("Copy the address"), ButtonKind::Ghost, ui::ButtonSize::Md, false)
                .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    clipboard::copy_to_clipboard(cx, gpui::ClipboardItem::new_string(text.clone()));
                    this.internet.copied = Some((text.clone(), Instant::now()));
                    cx.notify();
                }));
                d.child(
                    div()
                        .mt(px(space::XS))
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(space::MD))
                        .child(
                            ui::text(step::LEAD, FontWeight::MEDIUM, ink::FULL)
                                .min_w_0()
                                .truncate()
                                .child(address),
                        )
                        .child(copy),
                )
                .child(ui::card_note(tr("Only friends you invite can join.")).mt(px(space::XS)))
            }
        }
    }

    /// The Servers page's invitations, under the server's Players: once Tailscale runs,
    /// inviting a friend to this computer.
    pub(super) fn internet_invites(&mut self, port: u16, cx: &mut Context<Self>) -> Div {
        if !matches!(self.internet.state, Some(State::Running(_))) {
            return div();
        }
        let ip = self
            .internet
            .node()
            .and_then(|n| n.ipv4())
            .map(|ip| ip.to_string())
            .unwrap_or_default();
        let panel = div()
            .flex()
            .flex_col()
            .gap(px(space::MD))
            .child(ui::label(tr("Invite a friend")))
            .child(self.invite_section(&ip, port, cx));
        self.internet_message(panel)
    }

    fn internet_message(&self, panel: Div) -> Div {
        match &self.internet.message {
            Some(Ok(m)) => {
                panel.child(ui::text(step::NOTE, FontWeight::NORMAL, ink::BODY).child(m.clone()))
            }
            Some(Err(e)) => panel.child(ui::error_note(e.clone())),
            None => panel,
        }
    }

    fn invite_section(&mut self, ip: &str, port: u16, cx: &mut Context<Self>) -> Div {
        let line = |s: String| ui::text(step::NOTE, FontWeight::NORMAL, ink::MUTED).child(s);
        let link = |id: &'static str, label: &'static str, url: &'static str| {
            ui::button(
                id,
                None,
                tr(label),
                ButtonKind::Secondary,
                ui::ButtonSize::Md,
            )
            .on_press(move |_, _, cx| cx.open_url(url))
        };
        let token = self.internet.token();
        if token.is_none() {
            let mut d = div().flex().flex_col().gap(px(space::MD)).child(line(
                tr("Share this computer with friends in Tailscale's admin console, or here with an access token.")
                    .into(),
            ));
            let machines = link(
                "ts-console",
                "Open the admin console",
                "https://login.tailscale.com/admin/machines",
            );
            if self.internet.asking_token {
                let save = ui::button(
                    "ts-token-save",
                    None,
                    tr("Save"),
                    ButtonKind::Secondary,
                    ui::ButtonSize::Md,
                )
                .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.save_token();
                    cx.notify();
                }));
                d = d
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap(px(space::SM))
                            .child(self.field(
                                Field::TailscaleToken,
                                "tskey-api-…",
                                300.,
                                ui::ButtonSize::Md,
                                cx,
                            ))
                            .child(save)
                            .child(link("ts-keys", "Make a token", tailscale::KEYS)),
                    )
                    .child(line(
                        tr("In Tailscale: Settings, Keys, Generate access token.").into(),
                    ));
            } else {
                d = d.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(space::SM))
                        .child(machines)
                        .child(
                            ui::button(
                                "ts-token",
                                None,
                                tr("Use an access token…"),
                                ButtonKind::Secondary,
                                ui::ButtonSize::Md,
                            )
                            .on_press(cx.listener(
                                |this, _: &ClickEvent, _, cx| {
                                    this.internet.asking_token = true;
                                    this.internet.message = None;
                                    cx.notify();
                                },
                            )),
                        ),
                );
            }
            return d;
        }

        // Invite.
        let busy = self.internet.busy;
        let invite = ui::button(
            "ts-invite",
            None,
            tr("Make an invitation"),
            ButtonKind::Secondary,
            ui::ButtonSize::Md,
        );
        let invite = if busy {
            ui::disabled(invite)
        } else {
            invite.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.invite_friend();
                cx.notify();
            }))
        };
        let mut d = div()
            .flex()
            .flex_col()
            .gap(px(space::MD))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(space::SM))
                    .child(self.field(
                        Field::FriendEmail,
                        tr("Their email (optional)"),
                        260.,
                        ui::ButtonSize::Md,
                        cx,
                    ))
                    .child(invite),
            )
            .child(line(
                tr("One invitation per friend. Without an email, send them the link.").into(),
            ));
        if let Some(fresh) = self.internet.fresh.clone() {
            d = d.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(space::MD))
                    .child(
                        ui::text(step::CAPTION, FontWeight::NORMAL, ink::FULL)
                            .font_family(theme::MONO)
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(fresh.url.clone()),
                    )
                    .child(self.copy_button("ts-fresh-copy", fresh.url, ButtonKind::Secondary, cx)),
            );
        }
        match self.internet.invites.clone() {
            Some(Ok(list)) if !list.is_empty() => {
                let mut rows = div().flex().flex_col();
                for inv in list {
                    let who = match (&inv.accepted_by, &inv.email) {
                        (Some(by), _) => format!("Shared with {by}"),
                        (None, Some(email)) => format!("Sent to {email}, not accepted yet"),
                        (None, None) => "Link not used yet".to_string(),
                    };
                    let id = inv.id.clone();
                    let withdraw = ui::button(
                        SharedString::from(format!("ts-revoke-{}", inv.id)),
                        None,
                        tr(if inv.accepted {
                            "Stop sharing"
                        } else {
                            "Withdraw"
                        }),
                        ButtonKind::Secondary,
                        ui::ButtonSize::Md,
                    );
                    let withdraw = if busy {
                        ui::disabled(withdraw)
                    } else {
                        withdraw.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            let id = id.clone();
                            this.internet.fresh = None;
                            this.internet_action(move |api| {
                                api.revoke(&id)?;
                                Ok("Withdrawn: that friend can't reach this computer any more."
                                    .into())
                            });
                            cx.notify();
                        }))
                    };
                    let date = inv.created.get(..10).unwrap_or_default();
                    rows = rows.child(
                        ui::list_row(
                            SharedString::from(format!("ts-invite-{}", inv.id)),
                            false,
                            |_, _, _| {},
                        )
                        .child(ui::item_text(
                            who,
                            trf("Made {date}", &[("date", &dates::published(date, ""))]),
                        ))
                        .child(withdraw.flex_none()),
                    );
                }
                d = d.child(rows);
            }
            Some(Err(e)) => d = d.child(ui::error_note(e)),
            _ => {}
        }

        // What shared friends reach.
        let reach = match &self.internet.policy {
            Some(Ok((policy, etag))) => Some((
                tailscale::reach(policy, ip, port),
                policy.clone(),
                etag.clone(),
            )),
            Some(Err(e)) => {
                d = d.child(line(format!("Couldn't read your access controls: {e}")));
                None
            }
            None => None,
        };
        if let Some((reach, policy, etag)) = reach {
            let mut safety = div().flex().flex_col().gap(px(space::SM)).pt(px(space::XS));
            match reach {
                Reach::Everything => {
                    safety =
                        safety.child(ui::text(step::BODY, FontWeight::NORMAL, ink::FULL).child(
                            "Friends you share this computer with can reach all of it, not only \
                         Minecraft (Tailscale's default).",
                        ));
                    if self.internet.confirm_limit {
                        let new = tailscale::limit_shared(&policy, ip, port);
                        safety = safety
                            .child(line(format!(
                                "This changes your tailnet's access controls: the rule that lets \
                                 everyone reach everything becomes one for your own devices and \
                                 members, and friends you share with reach only {ip}:{port}."
                            )))
                            .child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .gap(px(space::SM))
                                    .child(
                                        ui::button("ts-limit-go", None, tr("Change the access controls"), ButtonKind::Secondary, ui::ButtonSize::Md)
                                            .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                                if let Some(new) = new.clone() {
                                                    this.change_policy(
                                                        new,
                                                        etag.clone(),
                                                        "Done: friends you share this computer with reach only the Minecraft server.",
                                                    );
                                                }
                                                cx.notify();
                                            })),
                                    )
                                    .child(ui::button("ts-limit-cancel", None, tr("Cancel"), ButtonKind::Ghost, ui::ButtonSize::Md).on_press(
                                        cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.internet.confirm_limit = false;
                                            cx.notify();
                                        }),
                                    )),
                            );
                    } else {
                        let b = ui::button(
                            "ts-limit",
                            None,
                            tr("Limit them to Minecraft"),
                            ButtonKind::Secondary,
                            ui::ButtonSize::Md,
                        );
                        safety = safety.child(div().flex().child(if busy {
                            ui::disabled(b)
                        } else {
                            b.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.internet.confirm_limit = true;
                                cx.notify();
                            }))
                        }));
                    }
                }
                Reach::ServerOnly => {
                    let undo = tailscale::unlimit_shared(&policy, ip, port);
                    let b = ui::button(
                        "ts-unlimit",
                        None,
                        tr("Remove the limit"),
                        ButtonKind::Secondary,
                        ui::ButtonSize::Md,
                    );
                    safety = safety.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(space::MD))
                            .child(
                                ui::text(step::BODY, FontWeight::NORMAL, ink::FULL)
                                    .flex_1()
                                    .child(ui::state_line(
                                        ui::Tone::Live,
                                        format!(
                                            "Friends you share with reach only the Minecraft server (port {port})."
                                        ),
                                    )),
                            )
                            .child(if busy {
                                ui::disabled(b)
                            } else {
                                b.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    if let Some(undo) = undo.clone() {
                                        this.change_policy(
                                            undo,
                                            etag.clone(),
                                            "Tailscale's default access controls are back.",
                                        );
                                    }
                                    cx.notify();
                                }))
                            }),
                    );
                }
                Reach::Custom => {
                    let snippet = tailscale::snippet(ip, port);
                    safety = safety
                        .child(ui::text(step::BODY, FontWeight::NORMAL, ink::FULL).child(
                            "Your access controls are your own, so Eclipse leaves them alone. To \
                             keep shared friends to Minecraft, add this rule in the admin console:",
                        ))
                        .child(
                            ui::text(step::CAPTION, FontWeight::NORMAL, ink::BODY)
                                .p(px(ui::PANEL_INSET))
                                .rounded(px(radius::ROW))
                                .bg(ui::inset())
                                .font_family(theme::MONO)
                                .child(snippet.clone()),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap(px(space::SM))
                                .child(self.copy_button(
                                    "ts-snippet-copy",
                                    snippet,
                                    ButtonKind::Secondary,
                                    cx,
                                ))
                                .child(link("ts-acls", "Open access controls", tailscale::ACCESS)),
                        );
                }
            }
            d = d.child(safety);
        }
        d.child(
            div().flex().child(
                ui::button(
                    "ts-forget",
                    None,
                    tr("Forget the access token"),
                    ButtonKind::Secondary,
                    ui::ButtonSize::Md,
                )
                .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.forget_token();
                    cx.notify();
                })),
            ),
        )
    }
}
