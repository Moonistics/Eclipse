//! Home's Packs tab: one row per resource or shader pack, its picture, "Resource pack · on" and
//! a switch that turns it on or off. Find packs, Add from file… and the packs folder head the
//! list; packs dropped on it are added; Delete asks first, in place.
use crate::menus::Asking;
use crate::mods_state;
use crate::ui::{self, ButtonKind, Press};
use crate::Launcher;
use eclipse_launcher::i18n::{tr, trf};
use eclipse_launcher::packs::{self, Kind as PackKind, Pack};
use eclipse_theme::glass::{radius, space};
use gpui::prelude::*;
use gpui::{div, px, ClickEvent, Div, ExternalPaths, SharedString, Stateful};

impl Launcher {
    /// Home's Packs tab: the selected instance's resource packs, then its shader packs.
    pub(super) fn packs_tab(&mut self, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        let home = self.selected_instance()?.home.clone();
        self.load_packs_list(&home, cx);
        let (resource, shader): (Vec<_>, Vec<_>) =
            (self.game_packs.list.iter().cloned()).partition(|(p, _)| p.kind == PackKind::Resource);
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
                Some(crate::icons::ADD),
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
            .on_press(mods_state::find_on_mods_page(
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
                    .child(ui::open_folder_icon(
                        "packs-folder",
                        tr("Open packs folder"),
                        folder,
                        ButtonKind::Ghost,
                        ui::ButtonSize::Md,
                    )),
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
        if self.selected_play_state().playing() {
            body = body.child(ui::list_head(tr("Close the game to change packs here.")));
        }
        for (pack, icon) in resource.into_iter().chain(shader) {
            let row = self.pack_row(pack, icon.map(gpui::ImageSource::from), cx);
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
        let held = self
            .selected_play_state()
            .playing()
            .then(|| self.busy_reason())
            .flatten();
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
        let toggle = if let Some(why) = held.clone() {
            ui::unavailable(toggle, why)
        } else {
            let flip = pack.clone();
            toggle.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                let flip = flip.clone();
                this.pack_action(move |home| {
                    packs::set_enabled(home, &flip, !flip.enabled).map(|()| match flip.enabled {
                        true => trf("{name} is off", &[("name", &flip.title)]),
                        false => trf("{name} is on", &[("name", &flip.title)]),
                    })
                });
                cx.notify();
            }))
        };
        let remove = SharedString::from(format!("{key}-remove"));
        let end = if confirming && held.is_none() {
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
                            .map(|()| trf("{name} moved to the trash", &[("name", &gone.title)]))
                    });
                    cx.notify();
                }),
            )
        } else {
            let ask = ui::remove_button(remove, tr("Delete…"), ui::ButtonSize::Md);
            let ask = if let Some(why) = held {
                ui::unavailable(ask, why)
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
