//! Home's Packs tab: one row per resource or shader pack, its picture, "Resource pack · on" and
//! a switch that turns it on or off. Find packs, Add from file… and the packs folder head the
//! list; packs dropped on it are added; Delete asks first, in place.
use super::*;
use crate::menus::Asking;
use crate::ui::{self};
use eclipse_launcher::packs::{self, Kind as PackKind, Pack};
use eclipse_theme::glass::{radius, space};
use gpui::ExternalPaths;

impl Launcher {
    /// Home's Packs tab: the selected instance's resource packs, then its shader packs.
    pub(super) fn packs_tab(&mut self, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        let home = self.selected_instance()?.home.clone();
        self.load_packs_list(home.clone());
        let resource = self.game_packs.resource.clone();
        let shader = self.game_packs.shader.clone();
        let count = resource.len() + shader.len();
        // The whole tab takes dropped packs, lit while one is over it.
        let mut body = ui::drop_target(div().id("packs"))
            .flex()
            .flex_col()
            .rounded(px(radius::ROW))
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                this.add_packs(paths.paths().to_vec());
                // Not the window's import as well.
                cx.stop_propagation();
                cx.notify();
            }));
        let add = || {
            ui::button(
                "packs-add",
                Some("plus-sign"),
                tr("Add from file…"),
                ButtonKind::Secondary,
                ui::ButtonSize::Md,
            )
            .on_press(cx.listener(
                |this: &mut Self, _: &ClickEvent, _, cx: &mut Context<Self>| {
                    this.choose_packs(cx);
                    cx.notify();
                },
            ))
        };
        let find = || {
            ui::button(
                "packs-find",
                Some("search-01"),
                tr("Find packs"),
                ButtonKind::Secondary,
                ui::ButtonSize::Md,
            )
            .on_press(home_mods::find_on_mods_page(
                eclipse_launcher::browser::Kind::ResourcePack,
                cx,
            ))
        };
        if count == 0 {
            return Some(body.child(ui::empty_state(
                "packs-empty",
                crate::icons::PACK,
                tr("No resource packs or shader packs yet"),
                tr("Resource packs change textures and sounds; shader packs change the lighting."),
                vec![find(), add()],
            )));
        }
        let folder = home.join(PackKind::Resource.folder());
        body = body.child(
            ui::list_head(SharedString::default()).child(
                div()
                    .flex()
                    .gap(px(space::SM))
                    .child(find())
                    .child(add())
                    .child(
                        ui::icon_button(
                            "packs-folder",
                            "folder-01",
                            tr("Open packs folder"),
                            ButtonKind::Ghost,
                            ui::ButtonSize::Md,
                            false,
                        )
                        .on_press(move |_, _, cx| ui::open_folder(&folder, cx)),
                    ),
            ),
        );
        match self.game_packs.note.clone() {
            Some(Ok(m)) => body = body.child(ui::list_head(m)),
            Some(Err(e)) => {
                body = body.child(ui::error_line_for(
                    "packs-failed",
                    tr("Couldn't change the packs"),
                    &e,
                    None,
                    Vec::new(),
                ))
            }
            None => {}
        }
        if self.game.client.is_some() {
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
        let hovered = ui::hover_within(&key);
        let which = (pack.kind, pack.file.clone());
        let confirming = self.is_asking(&Asking::GamePack(which.0, which.1.clone()));
        let running = self.game.client.is_some();
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
            ui::disabled(toggle)
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
                ui::Confirm::Delete(&pack.title),
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.asking = None;
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
            let ask = ui::remove_button(remove, tr("Delete…"), ui::ButtonSize::Md);
            let ask = if running {
                ui::disabled(ask)
            } else {
                ask.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.asking = Some(Asking::GamePack(which.0, which.1.clone()));
                    cx.notify();
                }))
            };
            div()
                .flex()
                .items_center()
                .gap(px(space::SM))
                .child(ask)
                .child(toggle)
        };
        ui::list_row(key, hovered || confirming)
            .child(ui::row_picture(icon, crate::icons::PACK))
            .child(ui::item_text(pack.title.clone(), note.join(" · ")))
            .child(end.relative().flex_none())
    }
}
