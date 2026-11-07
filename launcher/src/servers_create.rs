//! The Create a server dialog over the dimmed page.
use crate::fields::{CanAdd, Field};
use crate::scrollbar::Scrollbar;
use crate::servers_state::{set_up_label, Create, GameMode, Needs, DIFFICULTIES};
use crate::ui::{self, ink, ButtonKind, Press};
use crate::{layout, short, Launcher};
use eclipse_launcher::i18n::{tr, trf};
use eclipse_launcher::servers::Fix;
use eclipse_launcher::{account, servers};
use eclipse_theme::glass::{ink, pixel_text, radius, space, step};
use gpui::prelude::*;
use gpui::{
    div, img, px, AnyElement, App, ClickEvent, Div, FontWeight, ImageFormat, MouseDownEvent,
    SharedString, Stateful, Window,
};
use std::sync::{Arc, LazyLock};

/// A picture for each game mode: Eclipse screenshots from `docs/media`.
static COVERS: LazyLock<[Arc<gpui::Image>; 3]> = LazyLock::new(|| {
    let webp = |bytes: &[u8]| Arc::new(gpui::Image::from_bytes(ImageFormat::Webp, bytes.to_vec()));
    [
        webp(include_bytes!("../../docs/media/village.webp")),
        webp(include_bytes!("../../docs/media/underwater.webp")),
        webp(include_bytes!("../../docs/media/nether.webp")),
    ]
});

const FRIEND_FACE: f32 = ui::CHIP_FACE;

const ASIDE_WIDTH: f32 = 360.;

const COVER: (f32, f32) = (304., 190.);

const WORLD_LIST: f32 = 232.;

const WORLD_THUMB: (f32, f32) = (64., 40.);

/// Each game mode's line under its name.
fn mode_note(mode: GameMode) -> &'static str {
    match mode {
        GameMode::Survival => tr("Gather, build and stay alive"),
        GameMode::Creative => tr("Fly and build with every block"),
        GameMode::Adventure => tr("Play a map the way it was made"),
    }
}

fn mode_cover(mode: GameMode) -> Arc<gpui::Image> {
    let [village, underwater, nether] = &*COVERS;
    match mode {
        GameMode::Survival => village,
        GameMode::Creative => underwater,
        GameMode::Adventure => nether,
    }
    .clone()
}

/// A click that changes the draft with `f`.
fn set(
    cx: &mut Context<Launcher>,
    f: fn(&mut Create),
) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    let this = cx.entity().downgrade();
    move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
        let _ = this.update(cx, |this, cx| {
            if let Some(c) = &mut this.servers.create {
                f(c);
            }
            cx.notify();
        });
    }
}

impl Launcher {
    pub(super) fn create_dialog(&mut self, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        let creating = self.servers.creating;
        self.servers.create.as_ref()?;
        let aside = (self.window_width >= layout::DIALOG_ASIDE_WIDE)
            .then(|| self.create_aside().into_any_element());
        let body = self.create_body(cx);
        let foot = self.create_foot(cx);
        let close = cx.listener(move |this, _: &MouseDownEvent, _, cx| {
            if !creating {
                this.servers.create = None;
                this.servers.problem = None;
            }
            cx.notify();
        });
        let card = ui::split_dialog("create-card", tr("Create a server"), aside, body, foot);
        Some(
            ui::modal(
                "create-dialog",
                tr("Create a server"),
                Some(Box::new(close)),
            )
            .flex()
            .items_center()
            .justify_center()
            .p(px(space::XL))
            .child(card),
        )
    }

    fn create_aside(&mut self) -> Stateful<Div> {
        let Some(c) = self.servers.create.as_ref() else {
            return div().id("create-aside");
        };
        let typed = self.fields.value(Field::NewServerName).trim().to_string();
        let title = if typed.is_empty() {
            tr("My server").to_string()
        } else {
            typed
        };
        let label = |list: &[(&str, &'static str)], id: &str| {
            list.iter()
                .find(|(v, _)| *v == id)
                .map_or(String::new(), |(_, l)| tr(l).to_string())
        };
        let rules = [
            c.mode.label().to_string(),
            label(&DIFFICULTIES, c.difficulty),
            trf(
                "up to {max} players",
                &[("max", &c.max_players.to_string())],
            ),
            tr(if c.friends_only {
                "friends only"
            } else {
                "anyone with the address"
            })
            .to_string(),
        ]
        .join(" · ");
        let copy = c
            .copy
            .as_ref()
            .filter(|_| !c.new_world)
            .and_then(|p| c.worlds.iter().find(|(_, path)| path == p));
        let seed = self.fields.value(Field::NewServerSeed).trim().to_string();
        let origin = match (copy, seed.is_empty()) {
            (Some((folder, path)), _) => {
                let name = c.details.get(path).map_or(folder, |d| &d.0);
                trf("A copy of {name}", &[("name", name)])
            }
            (None, false) => trf("New world, seed {seed}", &[("seed", &seed)]),
            (None, true) => tr("A new world with a random seed").to_string(),
        };
        // A copied world shows its own icon, else the mode's picture.
        let cover = match copy
            .map(|(_, p)| p.join("icon.png"))
            .filter(|p| p.is_file())
        {
            Some(icon) => img(icon)
                .size_full()
                .object_fit(gpui::ObjectFit::Cover)
                .into_any_element(),
            None => img(mode_cover(c.mode))
                .size_full()
                .object_fit(gpui::ObjectFit::Cover)
                .into_any_element(),
        };
        div()
            .id("create-aside")
            .flex()
            .flex_col()
            .flex_none()
            .w(px(ASIDE_WIDTH))
            .h_full()
            .p(px(space::XL))
            .overflow_y_scroll()
            .child(
                div()
                    .w(px(COVER.0))
                    .h(px(COVER.1))
                    .flex_none()
                    .overflow_hidden()
                    .rounded(px(radius::ROW))
                    .child(div().size_full().child(cover)),
            )
            .child(
                ui::entity_title(title, pixel_text::CARD_TITLE)
                    .mt(px(space::XL))
                    .truncate(),
            )
            .child(
                ui::text(step::BODY, FontWeight::NORMAL, ink::MUTED)
                    .mt(px(space::SM))
                    .child(rules),
            )
            .child(
                ui::text(step::BODY, FontWeight::NORMAL, ink::MUTED)
                    .mt(px(space::XS))
                    .truncate()
                    .child(origin),
            )
    }

    /// Every choice, scrolling between the dialog's title and its foot.
    fn create_body(&mut self, cx: &mut Context<Self>) -> Div {
        let Some(c) = self.servers.create.as_ref() else {
            return div();
        };
        let (new_world, copy, worlds) = (c.new_world, c.copy.clone(), c.worlds.clone());
        let details = c.details.clone();
        let (mode, difficulty, max) = (c.mode, c.difficulty, c.max_players);
        let (friends_only, check, friends) = (c.friends_only, c.check_accounts, c.friends.clone());
        let locked_out = check && c.offline;

        // The world: new (with a seed) or a copy of one of the player's.
        let card = |id: &'static str, on: bool, title: SharedString, note: String| {
            ui::choice_card(id, on)
                .flex_1()
                .min_w_0()
                .p(px(space::LG))
                .child(ui::item_text(title, note))
        };
        let count = if worlds.is_empty() {
            tr("No worlds yet").to_string()
        } else {
            trf(
                "{count} worlds to pick from",
                &[("count", &worlds.len().to_string())],
            )
        };
        let copy_card = card(
            "create-world-copy",
            !new_world,
            tr("Copy one of my worlds").into(),
            count,
        );
        let no_worlds = worlds.is_empty();
        let this = cx.entity().downgrade();
        let kinds = ui::options(
            div().id("create-world-kinds").flex().gap(px(space::MD)),
            tr("World type"),
            vec![
                (
                    card(
                        "create-world-new",
                        new_world,
                        tr("New world").into(),
                        String::new(),
                    ),
                    tr("New world").into(),
                    new_world,
                ),
                (
                    if no_worlds {
                        ui::disabled(copy_card)
                    } else {
                        copy_card
                    },
                    tr("Copy one of my worlds").into(),
                    !new_world,
                ),
            ],
            true,
            move |i, _, cx| {
                if i == 1 && no_worlds {
                    return;
                }
                let _ = this.update(cx, |this, cx| {
                    if let Some(c) = &mut this.servers.create {
                        c.new_world = i == 0;
                    }
                    cx.notify();
                });
            },
        );
        let detail = if new_world {
            self.field(
                Field::NewServerSeed,
                tr("Seed (optional)"),
                layout::FIELD_LG,
                ui::ButtonSize::Lg,
                cx,
            )
            .w_full()
            .into_any_element()
        } else {
            let rows = worlds
                .iter()
                .map(|(name, path)| {
                    let on = copy.as_ref() == Some(path);
                    let icon = path.join("icon.png");
                    let thumb = ui::picture_slot()
                        .flex_none()
                        .w(px(WORLD_THUMB.0))
                        .h(px(WORLD_THUMB.1))
                        .when(icon.is_file(), |d| {
                            d.child(img(icon).size_full().object_fit(gpui::ObjectFit::Cover))
                        });
                    let (shown, line) = match details.get(path) {
                        Some((name, line)) => (name.clone(), line.clone()),
                        None => (name.clone(), short(path)),
                    };
                    let row =
                        ui::list_choice(SharedString::from(format!("create-copy-{name}")), on)
                            .p(px(space::SM))
                            .pr(px(space::LG))
                            .gap(px(space::LG))
                            .child(thumb)
                            .child(ui::item_text(shown.clone(), line))
                            .child(ui::tick_slot(on));
                    (row, SharedString::from(shown), on)
                })
                .collect();
            let paths: Vec<_> = worlds.iter().map(|(_, path)| path.clone()).collect();
            let this = cx.entity().downgrade();
            let list = ui::options(
                ui::list_box("create-worlds", WORLD_LIST),
                tr("World"),
                rows,
                true,
                move |i, _, cx| {
                    let _ = this.update(cx, |this, cx| {
                        if let (Some(c), Some(path)) = (&mut this.servers.create, paths.get(i)) {
                            c.copy = Some(path.clone());
                        }
                        cx.notify();
                    });
                },
            );
            ui::with_scrollbar(list).into_any_element()
        };
        let world = ui::field_group(
            "create-world-title",
            tr("World"),
            None,
            div()
                .flex()
                .flex_col()
                .gap(px(space::MD))
                .child(kinds)
                .child(detail),
        );

        let mode_cards = GameMode::ALL
            .into_iter()
            .map(|m| {
                let card = ui::choice_card(
                    SharedString::from(format!("create-mode-{}", m.id())),
                    mode == m,
                )
                .flex_1()
                .min_w_0()
                .p(px(space::LG))
                .child(ui::item_text(m.label(), mode_note(m).to_string()));
                (card, SharedString::from(m.label()), mode == m)
            })
            .collect();
        let this = cx.entity().downgrade();
        let modes = ui::options(
            div().id("create-modes").flex().gap(px(space::MD)),
            tr("Mode"),
            mode_cards,
            true,
            move |i, _, cx| {
                let _ = this.update(cx, |this, cx| {
                    if let Some(c) = &mut this.servers.create {
                        c.mode = GameMode::ALL[i];
                    }
                    cx.notify();
                });
            },
        );

        let this = cx.entity().downgrade();
        let difficulties = ui::segments(
            "create-difficulty",
            tr("Difficulty"),
            &ui::translated(&DIFFICULTIES),
            difficulty,
            move |id, _, cx| {
                let _ = this.update(cx, |this, cx| {
                    if let Some(c) = &mut this.servers.create {
                        c.difficulty = id;
                    }
                    cx.notify();
                });
            },
        );
        let players = self.number_stepper("create-players", max as i64, 1..=100, cx, |this, to| {
            if let Some(c) = &mut this.servers.create {
                c.max_players = to as u32;
            }
        });
        let numbers = div()
            .flex()
            .justify_between()
            .gap(px(space::XXL))
            .child(ui::field_group(
                "create-difficulty-title",
                tr("Difficulty"),
                None,
                div().flex().child(difficulties),
            ))
            .child(ui::field_group(
                "create-players-title",
                tr("Max players"),
                None,
                div().flex().child(players),
            ));

        // Who may join: friends only (their names, the first an operator) or anyone.
        let this = cx.entity().downgrade();
        let joins = ui::options(
            div().id("create-who").flex().gap(px(space::MD)),
            tr("Who can join"),
            vec![
                (
                    card(
                        "create-friends",
                        friends_only,
                        tr("Friends only").into(),
                        tr("Only players you list can join").to_string(),
                    ),
                    tr("Friends only").into(),
                    friends_only,
                ),
                (
                    card(
                        "create-anyone",
                        !friends_only,
                        tr("Anyone with the address").into(),
                        tr("Share the address only with people you trust").to_string(),
                    ),
                    tr("Anyone with the address").into(),
                    !friends_only,
                ),
            ],
            true,
            move |i, _, cx| {
                let _ = this.update(cx, |this, cx| {
                    if let Some(c) = &mut this.servers.create {
                        c.friends_only = i == 0;
                    }
                    cx.notify();
                });
            },
        );
        let mut who = div().flex().flex_col().child(joins);
        if friends_only {
            who = who
                .child(self.create_friends(&friends, cx))
                .child(ui::card_note(tr("The first name becomes an operator.")).mt(px(space::MD)));
        }

        let accounts = ui::toggle_row_inset(
            "create-accounts",
            tr("Require Microsoft sign-in"),
            Some(tr("Stops anyone joining under someone else's name.").into()),
            check,
        )
        .on_press(set(cx, |c| c.check_accounts = !c.check_accounts));
        let accounts = div()
            .flex()
            .flex_col()
            .gap(px(space::SM))
            .child(accounts)
            .when(locked_out, |d| {
                d.child(
                    ui::state_note(
                        ui::Tone::Warn,
                        tr("You play offline, so you couldn't join it."),
                    )
                    .px(px(ui::SETTING_ROW_PADDING)),
                )
            });

        let needs = self.create_needs(cx);
        let body = ui::scroll_pane("create-body", &self.servers.create_scroll.handle)
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .gap(px(space::XXL))
            .pb(px(space::LG))
            .pr(px(Scrollbar::ROOM))
            .children(needs)
            .child(ui::field_group(
                "create-name-title",
                tr("Name"),
                None,
                self.field(
                    Field::NewServerName,
                    tr("My server"),
                    layout::FIELD_LG,
                    ui::ButtonSize::Lg,
                    cx,
                )
                .w_full(),
            ))
            .child(world)
            .child(ui::field_group(
                "create-mode-title",
                tr("Game mode"),
                None,
                modes,
            ))
            .child(numbers)
            .child(ui::field_group(
                "create-join-title",
                tr("Who can join"),
                None,
                who,
            ))
            .child(accounts);
        self.servers
            .create_scroll
            .wrap(body)
            .flex_1()
            .min_h_0()
            .mr(px(-Scrollbar::ROOM))
            .children(ui::more_below_fade(
                &self.servers.create_scroll.handle,
                ui::dialog_colour(),
            ))
    }

    fn create_needs(&mut self, cx: &mut Context<Self>) -> Option<Div> {
        let need = self.server_needs()?;
        let game_files = matches!(need, Needs::GameFiles { .. });
        let busy = match need {
            Needs::Eclipse => self.install_bar(),
            Needs::GameFiles { .. } => self.servers_preparing(),
            Needs::ServerProgram { .. } => None,
        };
        let action = if let Some(bar) = busy {
            bar.into_any_element()
        } else {
            self.server_need_button("create", need.clone(), ButtonKind::Secondary, cx)
                .into_any_element()
        };
        let failed = (self.versions.update_error(&self.updates))
            .filter(|_| need == Needs::Eclipse)
            .map(|e| {
                ui::error_line_for(
                    "create-install-failed",
                    tr("Couldn't install Eclipse"),
                    e,
                    None,
                    Vec::new(),
                )
            })
            .or_else(|| {
                let e = self.shown_prepare_error().filter(|_| game_files)?;
                Some(ui::error_line_for(
                    "create-prepare-failed",
                    tr("Couldn't set up the game"),
                    e,
                    None,
                    Vec::new(),
                ))
            });
        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(space::MD))
                .child(ui::notice(
                    ui::Notice::Info,
                    crate::icons::INFO,
                    need.line(),
                    vec![action],
                ))
                .children(failed),
        )
    }

    /// The friends let in, as chips with their face (the first is an operator), then a field to add one.
    fn create_friends(&mut self, friends: &[String], cx: &mut Context<Self>) -> Div {
        let own = self
            .accounts
            .selected()
            .map(|a| (a.name.clone(), a.uuid.clone()));
        let mut chips = div()
            .mt(px(space::MD))
            .flex()
            .flex_wrap()
            .gap(px(space::SM));
        for (i, name) in friends.iter().enumerate() {
            let face = match &own {
                Some((n, uuid)) if n == name => self.player_face(uuid, FRIEND_FACE),
                _ => ui::tile(FRIEND_FACE).child(ui::icon(
                    "user",
                    ui::IconSize::Px16,
                    ink(ink::BODY),
                )),
            };
            let mut chip = ui::person_chip(face, name.clone()).when(i == 0, |d| {
                d.child(
                    ui::text(step::CAPTION, FontWeight::NORMAL, ink::FULL)
                        .text_color(ui::color(ui::look().accent_soft))
                        .child(tr("Operator")),
                )
            });
            if i > 0 {
                let gone = name.clone();
                chip = chip.pr(px(space::XS)).child(
                    ui::remove_button(
                        SharedString::from(format!("create-friend-remove-{name}")),
                        trf("Remove {name}", &[("name", name)]),
                        ui::ButtonSize::Sm,
                    )
                    .on_press(cx.listener(
                        move |this, _: &ClickEvent, _, cx| {
                            if let Some(c) = &mut this.servers.create {
                                c.friends.retain(|f| f != &gone);
                            }
                            cx.notify();
                        },
                    )),
                );
            }
            chips = chips.child(chip);
        }
        let typed = self.fields.value(Field::NewServerFriend).trim().to_string();
        let can_add = if typed.is_empty() {
            CanAdd::No(tr("Type a player's name").into())
        } else if friends.contains(&typed) {
            CanAdd::No(trf("{name} is already on the list", &[("name", &typed)]).into())
        } else if !account::valid_offline_name(&typed) {
            CanAdd::No(tr("A Minecraft name has 3 to 16 letters, digits or underscores.").into())
        } else {
            CanAdd::Yes
        };
        chips.child(self.name_adder(
            Field::NewServerFriend,
            "create-friend",
            layout::FIELD_MD,
            can_add,
            cx,
        ))
    }

    /// The name typed in the friend field joins the list, if Minecraft would take it.
    pub(super) fn add_create_friend(&mut self) {
        let typed = self.fields.value(Field::NewServerFriend).trim().to_string();
        let Some(c) = &mut self.servers.create else {
            return;
        };
        if account::valid_offline_name(&typed) && !c.friends.contains(&typed) {
            c.friends.push(typed);
            self.fields.text(Field::NewServerFriend).set(String::new());
        }
    }

    fn create_foot(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(c) = self.servers.create.as_ref() else {
            return Vec::new();
        };
        let (eula, missing, creating) = (c.eula, c.eula_missing, self.servers.creating);
        let agree = ui::toggle_label("create-eula", tr("I agree to the Minecraft EULA"), eula)
            .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                if let Some(c) = &mut this.servers.create {
                    c.eula = !c.eula;
                    c.eula_missing = false;
                }
                cx.notify();
            }));
        let read = ui::link("create-eula-read", tr("Read the EULA"))
            .on_press(|_, _, cx| cx.open_url(servers::EULA_URL));
        let status = if creating {
            let last = self.servers.job_log.back().cloned().unwrap_or_default();
            Some(ui::card_note(last).truncate())
        } else if let Some(p) = &self.servers.problem {
            Some(ui::error_note(p.message.clone()))
        } else if missing {
            Some(ui::error_note(tr(
                "Agree to the Minecraft EULA to create the server.",
            )))
        } else {
            None
        };
        let fix = self.servers.problem.as_ref().and_then(|p| match &p.fix {
            Some(fix @ Fix::Prepare) => Some(fix.clone()),
            _ => None,
        });
        let cancel = ui::dialog_dismiss("create-cancel", tr("Cancel"));
        let mut foot: Vec<AnyElement> = status
            .map(|s| s.w_full().min_w_0().into_any_element())
            .into_iter()
            .collect();
        foot.push(
            div()
                .flex()
                .items_center()
                .gap(px(space::MD))
                .mr_auto()
                .child(agree)
                .child(read)
                .into_any_element(),
        );
        if creating {
            let busy = ui::primary_busy("create-go", tr("Creating"), None, ui::DIALOG_BUTTON);
            foot.push(
                ui::dialog_foot(ui::disabled(cancel), Some(busy.into_any_element()))
                    .into_any_element(),
            );
            return foot;
        }
        let label = set_up_label(&self.servers_minecraft());
        foot.extend(fix.map(|fix| {
            let button = ui::button(
                "create-fix",
                None,
                label,
                ButtonKind::Secondary,
                ui::DIALOG_BUTTON,
            );
            self.fix_button(button, fix, cx).into_any_element()
        }));
        let cancel = cancel.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.servers.create = None;
            this.servers.problem = None;
            cx.notify();
        }));
        let (icon, label) = (Some(crate::icons::ADD), tr("Create server"));
        let create = match self.server_needs() {
            Some(need) => {
                ui::primary_unavailable("create-go", icon, label, ui::DIALOG_BUTTON, need.line())
            }
            None => ui::primary("create-go", icon, label, ui::DIALOG_BUTTON).on_press(cx.listener(
                |this, _: &ClickEvent, _, cx| {
                    this.create_server();
                    cx.notify();
                },
            )),
        };
        foot.push(ui::dialog_foot(cancel, Some(create.into_any_element())).into_any_element());
        foot
    }
}
