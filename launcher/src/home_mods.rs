//! Home's Mods tab: "N mods in {instance}" and Find mods over one row per mod, its icon, name and
//! "version · source". The filter and Update all sit in the head, a mod's update and its on/off
//! switch at the row's end.
use super::*;
use crate::ui::{self, white};
use eclipse_launcher::browser::Kind as PackKind;
use eclipse_theme::glass::{ink, radius, space};
use fields::Field;
use instance_mods::ModRow;

impl Launcher {
    /// Home's Mods tab, for the selected instance.
    pub(super) fn mods_tab(&mut self, cx: &mut Context<Self>) -> Div {
        let busy = self.mods.busy.is_some();
        let updates = self.mods.updates.len();
        let name = self
            .selected_instance()
            .map(|i| i.name.clone())
            .unwrap_or_default();
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
                "puzzle",
                tr("No mods yet"),
                tr("Find mods that run on this instance."),
                vec![find],
            ));
        }
        let note = match count {
            1 => trf("1 mod in {name}", &[("name", &name)]),
            _ => trf(
                "{count} mods in {name}",
                &[("count", &count.to_string()), ("name", &name)],
            ),
        };
        let filter = self
            .fields
            .value(Field::InstanceModsFilter)
            .trim()
            .to_lowercase();
        let field = self.field(
            Field::InstanceModsFilter,
            tr("Filter installed"),
            220.,
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
            ui::list_head(note).child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(space::SM))
                    .child(field)
                    .children(update_all)
                    .child(
                        ui::button(
                            "instance-mods-add",
                            Some("plus-sign"),
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
        let hovered = self.list_hover.as_ref() == Some(&key);
        let hover = self.list_hover_listener(key.clone(), cx);
        // The lists' plain tile without one.
        let known = row.icon.is_some()
            || row
                .entry
                .as_ref()
                .is_some_and(|e| self.project_icon(e).is_some());
        let icon = if known {
            div()
                .relative()
                .flex_none()
                .size(px(ui::TILE_MD))
                .rounded(px(radius::ROW))
                .overflow_hidden()
                .child(self.mod_icon(row.entry.as_ref(), row.icon.clone(), ui::TILE_MD))
                .into_any_element()
        } else {
            ui::row_picture(None, "puzzle")
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
        let note = match row.version.is_empty() {
            true => source,
            false => format!("{} · {source}", row.version),
        };
        let update = row.entry.as_ref().and_then(|e| {
            let key = (e.source, e.project.clone());
            let number = self.mods.updates.get(&key)?.clone();
            let (source, id) = key;
            let chip = ui::button(
                SharedString::from(format!("mod-update-{id}")),
                None,
                trf("Update {version}", &[("version", &number)]),
                ButtonKind::Secondary,
                ui::ButtonSize::Sm,
            );
            Some(match busy {
                true => ui::disabled(chip),
                false => chip.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.mods_install(source, id.clone(), None);
                    cx.notify();
                })),
            })
        });
        let controls = match &row.native {
            Some(native) => self.native_mod_actions(native, busy, cx),
            None => {
                let switch_row = row.clone();
                div().child(
                    ui::toggle(
                        SharedString::from(format!("mod-switch-{}", row.file)),
                        row.on,
                    )
                    .on_press(cx.listener(
                        move |this, _: &ClickEvent, _, cx| {
                            this.switch_mod(&switch_row);
                            cx.notify();
                        },
                    )),
                )
            }
        };
        ui::list_row(key, hovered, hover)
            .child(icon)
            .child(ui::item_text(row.name.clone(), note))
            .children(update.map(|u| u.relative()))
            .child(
                controls
                    .relative()
                    .flex()
                    .items_center()
                    .rounded(px(radius::CONTROL)),
            )
            .text_color(white(ink::FULL))
    }
}

pub(super) fn find_on_mods_page(
    kind: PackKind,
    cx: &Context<Launcher>,
) -> ui::Listener<ClickEvent> {
    Box::new(cx.listener(move |this, _: &ClickEvent, _, cx| {
        this.go(Page::Mods);
        this.mods_load();
        this.mods_set_kind(kind);
        cx.notify();
    }))
}
