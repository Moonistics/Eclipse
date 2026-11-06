//! Home's Mods tab in the glass look (the mockup's `InstanceMods`, `launcher-views.tsx`): "N mods
//! in {instance}" and + Find mods over one `ROW` per mod, its icon, name and "version · source".
//! The launcher's own controls fit in: the filter and Update all in the head, a mod's update
//! and its on/off `Toggle` at the row's end. Dark appearance only.
use super::*;
use crate::glass_ui::{self as ui, white};
use eclipse_theme::glass::{ink, radius};
use glass_home_lists::Listener;
use instance_mods::ModRow;
use servers_page::Field;

impl Launcher {
    /// Home's Mods tab, for the selected instance.
    pub(super) fn glass_instance_mods(&mut self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
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
            return body.child(ui::empty().child(tr("Reading the mods…")));
        }
        if count == 0 {
            let find = ui::button(
                "instance-mods-find",
                Some("search-01"),
                tr("Find mods"),
                Kind::Primary,
                ui::Size::Md,
                &t,
            )
            .on_click(find_mods(cx));
            return body.child(ui::empty_state(
                "instance-mods-empty",
                "puzzle",
                tr("No mods yet"),
                "",
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
            .servers
            .value(Field::InstanceModsFilter)
            .trim()
            .to_lowercase();
        let field = self.field(Field::InstanceModsFilter, tr("Filter installed"), 220., cx);
        let update_all = (updates > 0 && !busy).then(|| {
            ui::button(
                "instance-mods-update-all",
                None,
                trf("Update all ({count})", &[("count", &updates.to_string())]),
                Kind::Secondary,
                ui::Size::Md,
                &t,
            )
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.mods_update_all();
                cx.notify();
            }))
        });
        body = body.child(
            ui::list_head(note).child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(field)
                    .children(update_all)
                    .child(
                        ui::button(
                            "instance-mods-add",
                            Some("plus-sign"),
                            tr("Find mods"),
                            Kind::Secondary,
                            ui::Size::Md,
                            &t,
                        )
                        .on_click(find_mods(cx)),
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
                "",
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
            |i| {
                self.glass_mod_row(rows[i].clone(), busy, cx)
                    .into_any_element()
            },
        );
        body.child(list)
    }

    fn glass_mod_row(&mut self, row: ModRow, busy: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        let t = self.theme;
        let key = SharedString::from(format!("mod-{}", row.file));
        let hovered = self.list_hover.as_ref() == Some(&key);
        let hover = self.list_hover_listener(key.clone(), cx);
        // `PixelIcon` at 36, round at `round(36 · 0.26)`; the lists' plain tile without one.
        let known = row.icon.is_some()
            || row
                .entry
                .as_ref()
                .is_some_and(|e| self.project_icon(e).is_some());
        let icon = if known {
            div()
                .relative()
                .flex_none()
                .size(px(36.))
                .rounded(px(radius::ROW))
                .overflow_hidden()
                .child(self.mod_icon(row.entry.as_ref(), row.icon.clone(), 36.))
                .into_any_element()
        } else {
            glass_home_lists::picture(None, "puzzle")
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
                Kind::Secondary,
                ui::Size::Sm,
                &t,
            );
            Some(match busy {
                true => crate::disabled(chip),
                false => chip.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
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
                        self.look.accent,
                        self.look.on_accent,
                        &t,
                    )
                    .on_click(cx.listener(
                        move |this, _: &ClickEvent, _, cx| {
                            this.switch_mod(&switch_row);
                            cx.notify();
                        },
                    )),
                )
            }
        };
        ui::list_row(key, hovered, hover, &t)
            .child(icon)
            .child(ui::list_text(row.name.clone(), note))
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

/// Find mods: the Mods page, to add some to this instance.
fn find_mods(cx: &mut Context<Launcher>) -> Listener<ClickEvent> {
    Box::new(cx.listener(|this, _: &ClickEvent, _, cx| {
        this.go(Page::Mods);
        cx.notify();
    }))
}
