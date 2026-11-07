use crate::menus::Asking;
use crate::mods_page::DETAIL;
use crate::mods_state::{goes_to_trash, kind_words, licence_name, removal};
use crate::ui::{self, ButtonKind, Press};
use crate::Launcher;
use eclipse_launcher::browser;
use eclipse_launcher::browser::{Kind as PackKind, SourceId};
use eclipse_launcher::i18n::{tr, trf};
use eclipse_theme::glass::{ink, space};
use gpui::prelude::*;
use gpui::{div, px, AnyElement, ClickEvent, Div, FontWeight, SharedString, Stateful};

impl Launcher {
    pub(super) fn mod_detail(
        &mut self,
        instance: &str,
        close: Option<Stateful<Div>>,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let p = self.mods_picked()?;
        let icon = self.mods_icon(&p, ui::TILE_LG);
        let m = &self.mods;
        let support = m.support(&p);
        let entry = m.entry(&p).cloned();
        let busy = m.busy.clone();
        let checking = m.checking(&p);
        let mut by = Vec::new();
        if !p.author.is_empty() {
            by.push(trf("by {author}", &[("author", &p.author)]));
        }
        if let Some(source) = p.source.shown_for(&p.id) {
            by.push(trf("from {source}", &[("source", source)]));
        }
        if !p.license.is_empty() {
            by.push(licence_name(&p.license));
        }
        let said_above = self.mods_datapack_notice_shown()
            && support.reason() == Some(browser::DATAPACK_SUPPORT);
        let status = self.status_shown(&p, instance).filter(|_| !said_above);
        let status = status.map(|(tone, said, reason)| {
            div()
                .mt(px(space::LG))
                .flex()
                .flex_col()
                .gap(px(space::XS))
                .child(ui::state_title(tone, said))
                .children(reason.map(|r| {
                    ui::text(
                        eclipse_theme::glass::step::NOTE,
                        FontWeight::NORMAL,
                        ink::DIM,
                    )
                    .child(r)
                }))
        });
        let about = m
            .about
            .get(&p.id)
            .filter(|_| p.source == SourceId::Eclipse)
            .map(|lines| lines.join(" · "));

        let button: Option<AnyElement> = if let Some(e) = &entry {
            let confirming = self.is_asking(&Asking::Mod(e.project.clone()));
            let remove_id = SharedString::from(format!("mods-detail-remove-{}", p.id));
            let id = e.project.clone();
            let trash = goes_to_trash(e.kind);
            if confirming {
                let asked = removal(e.kind, &e.title);
                ui::confirm(
                    remove_id,
                    asked,
                    cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.asking = None;
                        cx.notify();
                    }),
                    cx.listener(move |this, _: &ClickEvent, _, cx| {
                        let _ = this.mods_remove(&id);
                        cx.notify();
                    }),
                )
                .into_any_element()
                .into()
            } else {
                let ask = match trash {
                    true => trf("Delete from {instance}…", &[("instance", instance)]),
                    false => trf("Remove from {instance}…", &[("instance", instance)]),
                };
                let short = if trash {
                    tr("Delete…")
                } else {
                    tr("Remove…")
                };
                let ask = fitted(ask, short, false, ButtonKind::Secondary);
                ui::button(
                    remove_id,
                    None,
                    ask,
                    ButtonKind::Secondary,
                    ui::ButtonSize::Lg,
                )
                .w_full()
                .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.asking = Some(Asking::Mod(id.clone()));
                    cx.notify();
                }))
                .into_any_element()
                .into()
            }
        } else if busy.as_ref().is_some_and(|b| b.installs(&p.id)) || checking {
            ui::disabled(
                ui::primary(
                    SharedString::from(format!("mods-detail-install-{}", p.id)),
                    Some(crate::icons::DOWNLOAD),
                    if checking {
                        tr("Checking…")
                    } else {
                        tr("Installing…")
                    },
                    ui::ButtonSize::Lg,
                )
                .w_full(),
            )
            .into_any_element()
            .into()
        } else if support.installable() {
            let short = match p.kind {
                PackKind::Modpack => tr("Install as instance"),
                _ => tr("Install"),
            };
            let label = Self::install_label(&p, instance);
            let label = fitted(label, short, true, ButtonKind::Primary);
            let known = p.clone();
            let why = self.install_refusal(&p);
            let install = ui::primary(
                SharedString::from(format!("mods-detail-install-{}", p.id)),
                Some(crate::icons::DOWNLOAD),
                label,
                ui::ButtonSize::Lg,
            )
            .w_full();
            match why {
                Some(why) => ui::unavailable(install, why),
                None => install.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.mods_install(known.source, known.id.clone(), Some(known.clone()));
                    cx.notify();
                })),
            }
            .into_any_element()
            .into()
        } else {
            None
        };
        let open = p.clone();
        Some(
            ui::card()
                .flex()
                .flex_col()
                .p(px(ui::CARD_PADDING))
                .children(close)
                .child(icon)
                .child(
                    ui::entity_title(
                        p.title.clone(),
                        eclipse_theme::glass::pixel_text::CARD_TITLE,
                    )
                    .mt(px(space::XL)),
                )
                .child(
                    ui::text(
                        eclipse_theme::glass::step::NOTE,
                        FontWeight::NORMAL,
                        ink::DIM,
                    )
                    .mt(px(space::XS))
                    .child(by.join(" · ")),
                )
                .child(
                    ui::text(
                        eclipse_theme::glass::step::BODY,
                        FontWeight::NORMAL,
                        ink::BODY,
                    )
                    .mt(px(space::LG))
                    .child(p.summary.clone()),
                )
                .children(about.map(|a| {
                    ui::text(
                        eclipse_theme::glass::step::NOTE,
                        FontWeight::NORMAL,
                        ink::DIM,
                    )
                    .mt(px(space::LG))
                    .child(a)
                }))
                .children(status)
                .child(
                    ui::text(
                        eclipse_theme::glass::step::NOTE,
                        FontWeight::NORMAL,
                        ink::DIM,
                    )
                    .mt(px(space::LG))
                    .child(trf(
                        "{count} downloads",
                        &[("count", &super::thousands(p.downloads))],
                    )),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(space::SM))
                        .mt(px(space::XL))
                        .children(button)
                        .child(
                            ui::button(
                                "mods-page-link",
                                None,
                                tr(kind_words(p.kind).open_page),
                                ButtonKind::Secondary,
                                ui::ButtonSize::Md,
                            )
                            .w_full()
                            .on_press(cx.listener(
                                move |this, _: &ClickEvent, _, cx| {
                                    this.mods_open(open.clone());
                                    cx.notify();
                                },
                            )),
                        ),
                ),
        )
    }
}

fn fitted(label: String, short: &str, icon: bool, kind: ButtonKind) -> String {
    let room = DETAIL - 2. * ui::CARD_PADDING;
    match ui::button_width(&label, icon, kind, ui::ButtonSize::Lg) <= room {
        true => label,
        false => short.to_string(),
    }
}
