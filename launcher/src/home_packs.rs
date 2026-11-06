//! Home's Packs tab: one row per resource or shader pack, its picture, "Resource pack · on" and
//! a switch that turns it on or off. Add pack… and Open folder head the list; packs dropped on it
//! are added; Delete asks first, in place. Then the Screenshots tab.
use super::*;
use crate::ui::{self, white};
use eclipse_launcher::packs::{self, Kind as PackKind, Pack};
use eclipse_theme::glass::{fill, ink, radius};
use gpui::ExternalPaths;
use home_lists::{danger_line, picture};

impl Launcher {
    /// Home's Packs tab: the selected instance's resource packs, then its shader packs.
    pub(super) fn packs_tab(&mut self, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        let instance = self.selected_instance()?;
        let (home, name) = (instance.home.clone(), instance.name.clone());
        self.load_packs_list(home.clone());
        let resource = self.game_packs.resource.clone();
        let shader = self.game_packs.shader.clone();
        let count = resource.len() + shader.len();
        // The whole tab takes dropped packs, lit as a row under the pointer while one is over it.
        let mut body = div()
            .id("packs")
            .flex()
            .flex_col()
            .rounded(px(radius::ROW))
            .drag_over::<ExternalPaths>(|s, _, _, _| s.bg(white(fill::HOVER)))
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                this.add_packs(paths.paths().to_vec());
                // Not the window's import as well.
                cx.stop_propagation();
                cx.notify();
            }));
        let add = || {
            cx.listener(
                |this: &mut Self, _: &ClickEvent, _, cx: &mut Context<Self>| {
                    this.choose_packs(cx);
                    cx.notify();
                },
            )
        };
        if count == 0 {
            let add = ui::button(
                "packs-add",
                Some("plus-sign"),
                tr("Add pack…"),
                ButtonKind::Secondary,
                ui::Size::Md,
            )
            .on_press(add());
            return Some(body.child(ui::empty_state(
                "packs-empty",
                "package-01",
                tr("No packs yet"),
                "",
                vec![add],
            )));
        }
        let note = match count {
            1 => trf("1 pack in {name}", &[("name", &name)]),
            _ => trf(
                "{count} packs in {name}",
                &[("count", &count.to_string()), ("name", &name)],
            ),
        };
        let folder = home.join(PackKind::Resource.folder());
        body = body.child(
            ui::list_head(note).child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(
                        ui::button(
                            "packs-folder",
                            None,
                            tr("Open folder"),
                            ButtonKind::Secondary,
                            ui::Size::Md,
                        )
                        .on_press(move |_, _, cx| {
                            let _ = std::fs::create_dir_all(&folder);
                            crate::toast::open(cx, &folder);
                        }),
                    )
                    .child(
                        ui::button(
                            "packs-add",
                            Some("plus-sign"),
                            tr("Add pack…"),
                            ButtonKind::Secondary,
                            ui::Size::Md,
                        )
                        .on_press(add()),
                    ),
            ),
        );
        match self.game_packs.note.clone() {
            Some(Ok(m)) => body = body.child(ui::list_head(m)),
            Some(Err(e)) => body = body.child(danger_line(e)),
            None => {}
        }
        if self.client.is_some() {
            body = body.child(ui::list_head(tr("Close the game to change packs here.")));
        }
        for (pack, icon) in resource {
            let row = self.pack_row(pack, icon.map(gpui::ImageSource::from), cx);
            body = body.child(row);
        }
        for pack in shader {
            let row = self.pack_row(pack, None, cx);
            body = body.child(row);
        }
        Some(body)
    }

    fn pack_row(
        &mut self,
        pack: Pack,
        icon: Option<gpui::ImageSource>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let kind = match pack.kind {
            PackKind::Resource => "resource",
            PackKind::Shader => "shader",
        };
        let key = SharedString::from(format!("pack-{kind}-{}", pack.file));
        let hovered = self.list_hover.as_ref() == Some(&key);
        let which = (pack.kind, pack.file.clone());
        let confirming = self.game_packs.confirm_remove.as_ref() == Some(&which);
        let running = self.client.is_some();
        let hover = self.list_hover_listener(key.clone(), cx);
        let mut note = vec![match (pack.kind, pack.enabled) {
            (PackKind::Resource, true) => tr("Resource pack · on"),
            (PackKind::Resource, false) => tr("Resource pack · off"),
            (PackKind::Shader, true) => tr("Shader pack · on"),
            (PackKind::Shader, false) => tr("Shader pack · off"),
        }
        .to_string()];
        if !pack.description.is_empty() {
            note.push(pack.description.clone());
        }
        let toggle = ui::toggle(SharedString::from(format!("{key}-toggle")), pack.enabled);
        let toggle = if running {
            crate::disabled(toggle)
        } else {
            let flip = pack.clone();
            toggle.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                let flip = flip.clone();
                this.pack_action(move |home| {
                    packs::set_enabled(home, &flip, !flip.enabled).map(|()| match flip.enabled {
                        true => trf("{name} is off.", &[("name", &flip.title)]),
                        false => trf("{name} is on.", &[("name", &flip.title)]),
                    })
                });
                cx.notify();
            }))
        };
        let remove = SharedString::from(format!("{key}-remove"));
        let end = if confirming && !running {
            let gone = pack.clone();
            ui::confirm(
                remove,
                trf("Delete {name}?", &[("name", &pack.title)]),
                tr("Delete"),
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.game_packs.confirm_remove = None;
                    cx.notify();
                }),
                cx.listener(move |this, _: &ClickEvent, _, cx| {
                    let gone = gone.clone();
                    this.pack_action(move |home| {
                        packs::remove(home, &gone)
                            .map(|()| trf("{name} moved to the trash.", &[("name", &gone.title)]))
                    });
                    cx.notify();
                }),
            )
        } else {
            let ask = ui::icon_button(remove, "delete-02", tr("Delete…"), ui::Size::Md, false);
            let ask = if running {
                crate::disabled(ask)
            } else {
                ask.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.game_packs.confirm_remove = Some(which.clone());
                    cx.notify();
                }))
            };
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(ask)
                .child(toggle)
        };
        ui::list_row(key, hovered || confirming, hover)
            .child(picture(icon, "package-01"))
            .child(ui::item_text(pack.title.clone(), note.join(" · ")))
            .child(end.relative().flex_none())
    }
}

impl Launcher {
    /// Home's Screenshots tab: how many, then the pictures in a grid, 4 across on wide windows,
    /// else 2, each 16:9, its file name under it. A click opens the viewer.
    pub(super) fn screenshots_tab(&mut self, cx: &mut Context<Self>) -> Option<Div> {
        let home = self.selected_instance()?.home.clone();
        self.load_screenshots(home.clone());
        let list = self.screenshots.list.clone();
        if list.is_empty() {
            return Some(div().child(ui::empty_state(
                "screenshots-empty",
                "image-01",
                tr("No screenshots yet"),
                tr("Press F2 in the game."),
                vec![],
            )));
        }
        let count = match list.len() {
            1 => tr("1 screenshot").to_string(),
            n => trf("{count} screenshots", &[("count", &n.to_string())]),
        };
        let folder = eclipse_launcher::screenshots::folder(&home);
        let privacy = self.shots_privacy(cx);
        let head = div()
            .flex()
            .items_center()
            .justify_between()
            .h(px(32.))
            .child(
                ui::text(
                    eclipse_theme::glass::step::NOTE,
                    FontWeight::NORMAL,
                    ink::MUTED,
                )
                .child(count),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .child(privacy)
                    .child(
                        ui::icon_button(
                            "screenshots-folder",
                            "folder-01",
                            tr("Open folder"),
                            ui::Size::Sm,
                            false,
                        )
                        .on_press(move |_, _, cx| {
                            let _ = std::fs::create_dir_all(&folder);
                            crate::toast::open(cx, &folder);
                        }),
                    ),
            );
        let shown = if self.screenshots.all {
            list.len()
        } else {
            screenshots_page::FIRST
        };
        let more = list.len() > screenshots_page::FIRST;
        let pictures: Vec<AnyElement> = list
            .iter()
            .take(shown)
            .map(|s| self.shot_picture(&s.path, cx))
            .collect();
        let mut pictures = pictures.into_iter();
        let grid = div()
            .grid()
            .grid_cols(if self.window_width >= crate::layout::PACK_GRID_WIDE {
                4
            } else {
                2
            })
            .gap_x(px(12.))
            .gap_y(px(16.))
            .mt(px(12.))
            .children(list.into_iter().take(shown).enumerate().map(|(i, shot)| {
                let picture = pictures.next();
                div()
                    .id(SharedString::from(format!("screenshot-{i}")))
                    .min_w_0()
                    .cursor_pointer()
                    .child(
                        div()
                            .w_full()
                            .aspect_ratio(16. / 9.)
                            .rounded(px(radius::ROW))
                            .overflow_hidden()
                            .bg(white(fill::HOVER))
                            .children(picture),
                    )
                    .child(
                        ui::text(
                            eclipse_theme::glass::step::CAPTION,
                            FontWeight::NORMAL,
                            ink::MUTED,
                        )
                        .pt(px(8.))
                        .truncate()
                        .child(shot.name.clone()),
                    )
                    .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.screenshots.viewing = Some(i);
                        this.screenshots.confirm_delete = false;
                        this.screenshots.note = None;
                        cx.notify();
                    }))
            }));
        let toggle_all = more.then(|| {
            let label = match self.screenshots.all {
                true => tr("Show fewer").to_string(),
                false => trf(
                    "Show all {count}",
                    &[("count", &self.screenshots.list.len().to_string())],
                ),
            };
            div().mt(px(16.)).child(
                ui::button(
                    "screenshots-all",
                    None,
                    label,
                    ButtonKind::Secondary,
                    ui::Size::Md,
                )
                .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.screenshots.all = !this.screenshots.all;
                    cx.notify();
                })),
            )
        });
        let error = match (&self.screenshots.note, self.screenshots.viewing) {
            (Some(Err(e)), None) => Some(
                div().mt(px(8.)).child(
                    ui::text(
                        eclipse_theme::glass::step::NOTE,
                        FontWeight::NORMAL,
                        ink::FULL,
                    )
                    .text_color(ui::danger())
                    .child(e.clone()),
                ),
            ),
            _ => None,
        };
        Some(
            div()
                .px(px(12.))
                .child(head)
                .children(error)
                .child(grid)
                .children(toggle_all),
        )
    }
}
