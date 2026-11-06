//! The Create a server dialog over the dimmed page. On the left, what the server will be (a
//! picture, its name and its rules); on the right, its name, world, game mode, difficulty, how
//! many players, who may join and whether accounts are checked; the EULA, Cancel and Create at
//! the foot.
use super::*;
use crate::ui::{self, white};
use eclipse_theme::glass::{fill, ink, pixel_text, radius, space, step};
use gpui::{img, ImageFormat};
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
const MODE_NOTES: [(&str, &str); 3] = [
    ("survival", "Gather, build and stay alive"),
    ("creative", "Fly and build with every block"),
    ("adventure", "Play a map the way it was made"),
];

fn mode_note(mode: &str) -> &'static str {
    MODE_NOTES
        .iter()
        .find(|(id, _)| *id == mode)
        .map_or("", |(_, note)| note)
}

/// A player name Minecraft accepts: 3 to 16 letters, digits or underscores.
fn valid_name(name: &str) -> bool {
    (3..=16).contains(&name.len()) && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
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
    pub(crate) fn create_dialog(&mut self, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
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

    /// The left column: the picture, the name, the rules and where the world comes from.
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
            label(&MODES, c.mode),
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
            None => {
                let i = MODES.iter().position(|(v, _)| *v == c.mode).unwrap_or(0);
                img(COVERS[i].clone())
                    .size_full()
                    .object_fit(gpui::ObjectFit::Cover)
                    .into_any_element()
            }
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
                    let thumb = div()
                        .flex_none()
                        .w(px(WORLD_THUMB.0))
                        .h(px(WORLD_THUMB.1))
                        .overflow_hidden()
                        .rounded(px(radius::ROW))
                        .bg(white(fill::BUTTON))
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

        // The game mode, three cards.
        let mode_cards = MODES
            .iter()
            .map(|&(id, label)| {
                let card =
                    ui::choice_card(SharedString::from(format!("create-mode-{id}")), mode == id)
                        .flex_1()
                        .min_w_0()
                        .p(px(space::LG))
                        .child(ui::item_text(tr(label), tr(mode_note(id)).to_string()));
                (card, SharedString::from(tr(label)), mode == id)
            })
            .collect();
        let this = cx.entity().downgrade();
        let modes = ui::options(
            div().id("create-modes").flex().gap(px(space::MD)),
            mode_cards,
            true,
            move |i, _, cx| {
                let _ = this.update(cx, |this, cx| {
                    if let Some(c) = &mut this.servers.create {
                        c.mode = MODES[i].0;
                    }
                    cx.notify();
                });
            },
        );

        // The difficulty and the players stepper.
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

        let accounts = ui::toggle_row(
            "create-accounts",
            tr("Require Microsoft sign-in"),
            Some(tr("Stops anyone joining under someone else's name.").into()),
            check,
        )
        .bg(ui::inset())
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

        let body = ui::scroll_pane("create-body", &self.servers.create_scroll.handle)
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .gap(px(space::XXL))
            .pb(px(space::LG))
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
            .children(ui::more_below_fade(
                &self.servers.create_scroll.handle,
                ui::dialog_colour(),
            ))
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
                    white(ink::BODY),
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
        let ok = valid_name(&typed) && !friends.contains(&typed);
        chips.child(self.name_adder(
            Field::NewServerFriend,
            "create-friend",
            layout::FIELD_MD,
            ok,
            cx,
        ))
    }

    /// The name typed in the friend field joins the list, if Minecraft would take it.
    pub(super) fn add_create_friend(&mut self) {
        let typed = self.fields.value(Field::NewServerFriend).trim().to_string();
        let Some(c) = &mut self.servers.create else {
            return;
        };
        if valid_name(&typed) && !c.friends.contains(&typed) {
            c.friends.push(typed);
            self.fields.text(Field::NewServerFriend).set(String::new());
        }
    }

    /// The foot: what is happening, the EULA and its link, Cancel and Create server.
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
        let cancel = ui::button(
            "create-cancel",
            None,
            tr("Cancel"),
            ButtonKind::Ghost,
            ui::ButtonSize::Md,
        );
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
            foot.push(ui::disabled(cancel).into_any_element());
            foot.push(
                ui::primary_busy("create-go", tr("Creating"), None, ui::ButtonSize::Md)
                    .into_any_element(),
            );
            return foot;
        }
        foot.extend(fix.map(|fix| {
            ui::button(
                "create-fix",
                None,
                tr("Prepare game files"),
                ButtonKind::Secondary,
                ui::ButtonSize::Md,
            )
            .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.fix(fix.clone(), cx);
                cx.notify();
            }))
            .into_any_element()
        }));
        foot.push(
            cancel
                .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.servers.create = None;
                    this.servers.problem = None;
                    cx.notify();
                }))
                .into_any_element(),
        );
        foot.push(
            ui::primary("create-go", tr("Create server"), ui::ButtonSize::Md)
                .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.create_server();
                    cx.notify();
                }))
                .into_any_element(),
        );
        foot
    }
}

#[cfg(test)]
mod tests {
    use super::valid_name;

    #[test]
    fn friend_names_follow_minecrafts_rules() {
        assert!(valid_name("Player46008"));
        assert!(valid_name("oak_leaf"));
        assert!(!valid_name("ab"));
        assert!(!valid_name("seventeen_letters"));
        assert!(!valid_name("no spaces"));
        assert!(!valid_name("dash-name"));
    }
}
