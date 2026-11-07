//! Home's Mods tab: one row per mod, its icon, name and "version · source". The filter, Update
//! all and Find mods sit in the head, a mod's update, its on/off switch and Remove… at the row's end.
use super::*;
use crate::menus::Asking;
use crate::ui::{self, white};
use eclipse_launcher::browser::{self, java_instance, Kind as PackKind};
use eclipse_theme::glass::{ink, space};
use fields::Field;
use instance_mods::ModRow;

impl Launcher {
    /// Home's Mods tab, for the selected instance.
    pub(super) fn mods_tab(&mut self, cx: &mut Context<Self>) -> Div {
        let busy = self.mods.busy.is_some();
        let updates = self.mods.updates.len();
        let game = self
            .selected_instance()
            .and_then(|i| i.java.as_ref())
            .map(|j| j.line());
        let m = &self.instance_mods;
        let (fresh, count) = (m.fresh, m.list.len());
        let mut body = div().flex().flex_col();
        body = body.children(self.mods_busy_plate());
        if !fresh {
            return body.child(ui::loading_state(
                "instance-mods-loading",
                tr("Reading the mods…"),
            ));
        }
        if count == 0 {
            let find = ui::button(
                "instance-mods-find",
                Some("search-01"),
                tr("Find mods"),
                ButtonKind::Secondary,
                ui::ButtonSize::Md,
            )
            .on_press(find_on_mods_page(PackKind::Mod, cx));
            return body.child(ui::empty_state(
                "instance-mods-empty",
                crate::icons::MODS,
                tr("No mods yet"),
                match &game {
                    Some(game) => trf("Find mods made for {game}.", &[("game", game)]),
                    None => tr("Find mods made for Eclipse.").into(),
                },
                vec![find],
            ));
        }
        let filter = self
            .fields
            .value(Field::InstanceModsFilter)
            .trim()
            .to_lowercase();
        let field = self.field(
            Field::InstanceModsFilter,
            tr("Filter installed"),
            layout::FIELD_MD,
            ui::ButtonSize::Md,
            cx,
        );
        let update_all = (updates > 0 && !busy).then(|| {
            ui::button(
                "instance-mods-update-all",
                None,
                trf("Update all ({count})", &[("count", &updates.to_string())]),
                ButtonKind::Secondary,
                ui::ButtonSize::Md,
            )
            .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.mods_update_all();
                cx.notify();
            }))
        });
        body = body.child(
            ui::list_head(SharedString::default()).child(
                div()
                    .flex()
                    .flex_1()
                    .flex_wrap()
                    .justify_end()
                    .items_center()
                    .gap(px(space::SM))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(layout::FIELD_SM))
                            .max_w(px(layout::FIELD_MD))
                            .child(field.w_full()),
                    )
                    .children(update_all)
                    .child(
                        ui::button(
                            "instance-mods-add",
                            Some("search-01"),
                            tr("Find mods"),
                            ButtonKind::Secondary,
                            ui::ButtonSize::Md,
                        )
                        .on_press(find_on_mods_page(PackKind::Mod, cx)),
                    ),
            ),
        );
        let native = self.mods.java.is_none()
            && self.instance_mods.list.iter().any(|r| {
                r.native.is_some()
                    || r.entry
                        .as_ref()
                        .is_some_and(|e| native_status::is_native_file(&e.file))
            });
        body = body.children(native.then(|| self.native_build(cx)).flatten());
        let rows: Vec<ModRow> = self
            .instance_mods
            .list
            .iter()
            .filter(|r| filter.is_empty() || r.name.to_lowercase().contains(&filter))
            .cloned()
            .collect();
        if rows.is_empty() {
            return body.child(ui::empty_state(
                "instance-mods-no-match",
                "search-01",
                tr("No mod matches"),
                tr("Try another name."),
                vec![],
            ));
        }
        // A modpack has hundreds: only the rows near the view are built.
        let shown = (rows.len(), filter);
        if self.instance_mods.shown != shown {
            self.instance_mods.shown = shown;
            self.instance_mods.cull.reset();
        }
        let (cull, scroll) = (
            self.instance_mods.cull.clone(),
            self.content_scroll.handle.clone(),
        );
        let list = cull.column(
            &scroll,
            rows.len(),
            0.,
            |_| false,
            |i| self.mod_row(rows[i].clone(), busy, cx).into_any_element(),
        );
        body.child(list)
    }

    fn mod_row(&mut self, row: ModRow, busy: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        let key = SharedString::from(format!("mod-{}", row.file));
        let hovered = ui::hover_within(&key);
        // The lists' plain tile without one.
        let known = row.icon.is_some()
            || row
                .entry
                .as_ref()
                .is_some_and(|e| self.project_icon(e).is_some());
        let icon = if known {
            ui::face(
                self.mod_icon(row.entry.as_ref(), row.icon.clone(), ui::TILE_MD),
                ui::TILE_MD,
            )
            .into_any_element()
        } else {
            ui::row_picture(None, crate::icons::MODS)
        };
        let source = if let Some(origin) = row.native.as_ref().and_then(|n| n.origin.as_ref()) {
            trf("from GitHub: {source}", &[("source", &origin.label())])
        } else {
            match &row.entry {
                Some(e) if e.dependency => trf(
                    "{source} · needed by another mod",
                    &[("source", e.source.label())],
                ),
                Some(e) => e.source.label().to_string(),
                None => trf(
                    "Local file · {file}",
                    &[("file", row.file.trim_start_matches("mods/"))],
                ),
            }
        };
        let update = row
            .entry
            .as_ref()
            .and_then(|e| self.mods.updates.get(&(e.source, e.project.clone())))
            .map(|number| trf("{version} available", &[("version", number)]));
        let note = [row.version.clone(), source]
            .into_iter()
            .chain(update)
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" · ");
        let asked = self.mod_asked(&row);
        let confirming = self.is_asking(&asked);
        let controls = match &row.native {
            Some(native) => self.native_mod_actions(native, busy, cx),
            None => self.mod_actions(row.clone(), asked, cx),
        };
        ui::list_row(key, hovered || confirming)
            .child(icon)
            .child(ui::item_text(row.name.clone(), note))
            .child(controls.relative().flex().flex_none().items_center())
            .text_color(white(ink::FULL))
    }

    fn mod_asked(&self, row: &ModRow) -> Asking {
        match &row.entry {
            Some(e) => Asking::Mod(e.project.clone()),
            None => Asking::ModFile(self.mods.home().join(&row.file).display().to_string()),
        }
    }

    fn mod_actions(&self, row: ModRow, asked: Asking, cx: &mut Context<Self>) -> Div {
        let remove = SharedString::from(format!("mod-remove-{}", row.file));
        if self.is_asking(&asked) {
            let gone = row.clone();
            return div().child(ui::confirm(
                remove,
                ui::Confirm::Remove(&row.name),
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.asking = None;
                    cx.notify();
                }),
                cx.listener(move |this, _: &ClickEvent, _, cx| {
                    match &gone.entry {
                        Some(e) => {
                            let _ = this.mods_remove(&e.project);
                        }
                        None => this.remove_mod_file(&gone),
                    }
                    cx.notify();
                }),
            ));
        }
        let wait = self.mods_wait();
        let update = row.entry.as_ref().and_then(|e| {
            let key = (e.source, e.project.clone());
            let number = self.mods.updates.get(&key)?.clone();
            let (source, id) = key;
            let chip = ui::button(
                SharedString::from(format!("mod-update-{id}")),
                None,
                tr("Update"),
                ButtonKind::Secondary,
                ui::ButtonSize::Sm,
            )
            .aria_label(trf("Update to {version}", &[("version", &number)]));
            Some(match wait {
                Some(why) => ui::unavailable(chip, why),
                None => chip.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.mods_install(source, id.clone(), None);
                    cx.notify();
                })),
            })
        });
        let switch_row = row.clone();
        let switch = ui::toggle_label(
            SharedString::from(format!("mod-switch-{}", row.file)),
            if row.on { tr("On") } else { tr("Off") },
            row.on,
        )
        .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.switch_mod(&switch_row);
            cx.notify();
        }));
        let ask = ui::remove_button(remove, tr("Remove…"), ui::ButtonSize::Md).on_press(
            cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.asking = Some(asked.clone());
                cx.notify();
            }),
        );
        div()
            .gap(px(space::SM))
            .children(update)
            .child(switch)
            .child(ask)
    }

    fn remove_mod_file(&mut self, row: &ModRow) {
        let home = self.mods.home();
        let jar = java_instance::jars(&home)
            .into_iter()
            .find(|j| j.file == row.file);
        let said = match (browser::try_lock_home(&home), jar) {
            (None, _) => Err(tr("Wait for the current operation to finish.").into()),
            (Some(_lock), Some(jar)) => eclipse_launcher::desktop_seams::to_trash(&jar.path)
                .map(|()| trf("{name} moved to the trash.", &[("name", &row.name)])),
            (Some(_), None) => Ok(trf("Removed {name}.", &[("name", &row.name)])),
        };
        self.mods_said(said);
        self.stop_asking(|a| matches!(a, Asking::ModFile(_)));
        self.instance_mods_changed();
    }
}

pub(super) fn find_on_mods_page(
    kind: PackKind,
    cx: &Context<Launcher>,
) -> ui::Listener<ClickEvent> {
    Box::new(cx.listener(move |this, _: &ClickEvent, _, cx| {
        this.show_mods(kind);
        cx.notify();
    }))
}
