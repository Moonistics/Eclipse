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
const FRIEND_FIELD: f32 = 260.;

const DIALOG_MAX: (f32, f32) = (1180., 840.);

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
        let aside = (self.window_width >= layout::DIALOG_ASIDE_WIDE).then(|| self.create_aside());
        let main = self.create_main(aside.is_some(), cx);
        let close = cx.listener(move |this, _: &MouseDownEvent, _, cx| {
            if !creating {
                this.servers.create = None;
                this.servers.problem = None;
            }
            cx.notify();
        });
        let card = ui::dialog_plate()
            .id("create-card")
            .occlude()
            .flex()
            .w_full()
            .max_w(px(DIALOG_MAX.0))
            .h_full()
            .max_h(px(DIALOG_MAX.1))
            .children(aside)
            .child(main);
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
            .rounded_l(px(radius::MODAL))
            .bg(ui::inset())
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

    /// The right column: the title, every choice, and the foot.
    fn create_main(&mut self, aside: bool, cx: &mut Context<Self>) -> Div {
        let Some(c) = self.servers.create.as_ref() else {
            return div();
        };
        let (new_world, copy, worlds) = (c.new_world, c.copy.clone(), c.worlds.clone());
        let details = c.details.clone();
        let (mode, difficulty, max) = (c.mode, c.difficulty, c.max_players);
        let (friends_only, check, friends) = (c.friends_only, c.check_accounts, c.friends.clone());
        let creating = self.servers.creating;
        let head = div()
            .flex()
            .flex_none()
            .items_start()
            .justify_between()
            .gap(px(space::LG))
            .px(px(space::XXL))
            .pt(px(space::XXL))
            .pb(px(space::XL))
            .child(div().flex().flex_col().min_w_0().child(ui::entity_title(
                tr("Create a server"),
                pixel_text::CARD_TITLE,
            )))
            .child(
                ui::icon_button(
                    "create-close",
                    "cancel-01",
                    tr("Close"),
                    ButtonKind::Ghost,
                    ui::ButtonSize::Md,
                    false,
                )
                .when(creating, ui::disabled)
                .when(!creating, |b| {
                    b.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.servers.create = None;
                        this.servers.problem = None;
                        cx.notify();
                    }))
                }),
            );

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
        let mut copy_card = card(
            "create-world-copy",
            !new_world,
            tr("Copy one of my worlds").into(),
            count,
        );
        copy_card = if worlds.is_empty() {
            ui::disabled(copy_card)
        } else {
            copy_card.on_press(set(cx, |c| c.new_world = false))
        };
        let kinds = div()
            .flex()
            .gap(px(space::MD))
            .child(
                card(
                    "create-world-new",
                    new_world,
                    tr("New world").into(),
                    String::new(),
                )
                .on_press(set(cx, |c| c.new_world = true)),
            )
            .child(copy_card);
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
            let mut list = ui::list_box("create-worlds", WORLD_LIST);
            for (name, path) in worlds {
                let on = copy.as_ref() == Some(&path);
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
                list = list.child(
                    ui::list_choice(SharedString::from(format!("create-copy-{name}")), on)
                        .p(px(space::SM))
                        .pr(px(space::LG))
                        .gap(px(space::LG))
                        .child(thumb)
                        .child(match details.get(&path) {
                            Some((name, line)) => ui::item_text(name.clone(), line.clone()),
                            None => ui::item_text(name, short(&path)),
                        })
                        .child(ui::tick_slot(on))
                        .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if let Some(c) = &mut this.servers.create {
                                c.copy = Some(path.clone());
                            }
                            cx.notify();
                        })),
                );
            }
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
            Some(
                div()
                    .child(tr("Stops anyone joining under someone else's name."))
                    .into_any_element(),
            ),
            check,
        )
        .bg(ui::inset())
        .on_press(set(cx, |c| c.check_accounts = !c.check_accounts));

        let body = ui::scroll_pane("create-body", &self.servers.create_scroll.handle)
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .gap(px(space::XXL))
            .px(px(space::XXL))
            .pb(px(space::XXL))
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
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .child(head)
            .child(
                self.servers
                    .create_scroll
                    .wrap(body)
                    .flex_1()
                    .min_h_0()
                    .children(ui::more_below_fade(
                        &self.servers.create_scroll.handle,
                        ui::dialog_colour(),
                    )),
            )
            .child(self.create_foot(aside, cx))
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
        let add = ui::button(
            "create-friend-add",
            None,
            tr("Add"),
            ButtonKind::Secondary,
            ui::ButtonSize::Sm,
        );
        let add = if ok {
            add.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.add_create_friend();
                cx.notify();
            }))
        } else {
            ui::disabled(add)
        };
        chips.child(self.field_with(
            Field::NewServerFriend,
            None,
            tr("Add a friend's name"),
            FRIEND_FIELD,
            ui::ButtonSize::Lg,
            Some(add.into_any_element()),
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

    /// The foot: the EULA and its link, what is happening, Cancel and Create server.
    fn create_foot(&mut self, aside: bool, cx: &mut Context<Self>) -> Div {
        let Some(c) = self.servers.create.as_ref() else {
            return div();
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
        // What is happening, between the EULA and the buttons.
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
            ui::ButtonSize::Lg,
        );
        let buttons = if creating {
            div()
                .flex()
                .gap(px(space::SM))
                .child(ui::disabled(cancel))
                .child(ui::primary_busy(
                    "create-go",
                    tr("Creating"),
                    None,
                    ui::ButtonSize::Lg,
                ))
        } else {
            let create = ui::primary("create-go", tr("Create server"), ui::ButtonSize::Lg);
            div()
                .flex()
                .gap(px(space::SM))
                .children(fix.map(|fix| {
                    ui::button(
                        "create-fix",
                        None,
                        tr("Prepare the game files"),
                        ButtonKind::Secondary,
                        ui::ButtonSize::Lg,
                    )
                    .on_press(cx.listener(
                        move |this, _: &ClickEvent, _, cx| {
                            this.fix(fix.clone(), cx);
                            cx.notify();
                        },
                    ))
                }))
                .child(cancel.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.servers.create = None;
                    this.servers.problem = None;
                    cx.notify();
                })))
                .child(create.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.create_server();
                    cx.notify();
                })))
        };
        div()
            .flex()
            .flex_none()
            .flex_col()
            .gap(px(space::MD))
            .px(px(space::XXL))
            .py(px(space::XL))
            .rounded_br(px(radius::MODAL))
            .when(!aside, |d| d.rounded_bl(px(radius::MODAL)))
            .bg(ui::inset())
            .children(status.map(|s| s.min_w_0()))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(space::LG))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(space::MD))
                            .child(agree)
                            .child(read),
                    )
                    .child(buttons.ml_auto()),
            )
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
