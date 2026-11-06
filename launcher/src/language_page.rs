//! Settings' Language row (`eclipse_launcher::i18n`): a dropdown of the system's language and the
//! ones the launcher has, each named in itself. A list rather than a segment per language: nine
//! segments took the row's whole width and left the row's own sentence a character wide.
use super::*;
use crate::glass_ui as ui;
use eclipse_launcher::i18n::Language;

/// What the dropdown shows for a choice: the system's, in the launcher's language, or a
/// language's name in itself (never through the table: a table entry for "English" would rename
/// the very entry a player looks for).
fn choice_name(choice: Option<Language>) -> &'static str {
    match choice {
        None => tr("System"),
        Some(language) => language.name(),
    }
}

impl Launcher {
    fn pick_language(&mut self, choice: Option<Language>, cx: &mut Context<Self>) {
        self.settings.language = choice;
        self.save_settings();
        i18n::set(self.settings.language());
        self.language_menu = false;
        cx.notify();
    }

    /// The list: a popover like the others, its rows scrolling past ten.
    fn language_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let mut rows = div()
            .id("language-list")
            .flex()
            .flex_col()
            .max_h(px(360.))
            .overflow_y_scroll();
        for choice in std::iter::once(None).chain(Language::ALL.map(Some)) {
            let id = choice.map_or("system", Language::code);
            rows = rows.child(
                ui::pick_row(
                    format!("language-pick-{id}"),
                    choice_name(choice),
                    choice == self.settings.language,
                    &t,
                )
                .on_click(
                    cx.listener(move |this, _: &ClickEvent, _, cx| this.pick_language(choice, cx)),
                ),
            );
        }
        // A solid backing: the backdrop doesn't paint under a popover whose rows scroll (seen on
        // the desktop run), and the rows must stay legible.
        let menu = ui::popover(220.)
            .bg(crate::glass::colour(crate::glass::Kind::Float))
            .id("language-menu")
            .occlude()
            .on_mouse_down_out(cx.listener(|this, event: &MouseDownEvent, _, cx| {
                this.language_menu = false;
                this.menu_closed_at = Some(event.position);
                cx.notify();
            }))
            .child(rows);
        // Anchored by its top right under the picker; it opens upwards instead when the window
        // has no room below for it.
        gpui::deferred(
            div()
                .absolute()
                .top(px(ui::Size::Md.height() + 6.))
                .right_0()
                .child(gpui::anchored().anchor(gpui::Anchor::TopRight).child(menu)),
        )
        .with_priority(1)
    }

    pub(super) fn language_row(&self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let mut wrap = div().relative().child(
            ui::dropdown(
                "language-picker",
                None,
                choice_name(self.settings.language),
                self.language_menu,
                ui::Size::Md.height(),
                &t,
            )
            .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                if this.closed_by_this_press(event) {
                    return;
                }
                this.language_menu = !this.language_menu;
                cx.notify();
            })),
        );
        if self.language_menu {
            wrap = wrap.child(self.language_list(cx));
        }
        // The sentence is about following the computer's language, so it says nothing true once
        // the player has picked one: the row keeps it only while the choice is the system's.
        let said = self.settings.language.is_none().then(|| {
            div()
                .child(trf(
                    "System follows your computer's language: {language} here. A string not \
                     translated yet shows in English.",
                    &[("language", Language::system().name())],
                ))
                .into_any_element()
        });
        index_row(tr("Language"), said, Some(wrap.into_any_element()), &t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eclipse_launcher::i18n::{current, tr_in};

    #[test]
    fn a_language_is_named_in_itself_and_the_system_choice_in_the_launchers_language() {
        assert_eq!(choice_name(Some(Language::Russian)), "Русский");
        assert_eq!(choice_name(Some(Language::English)), "English");
        assert_eq!(choice_name(None), tr_in(current(), "System"));
    }
}
