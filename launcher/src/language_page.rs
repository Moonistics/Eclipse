//! Settings' Language row (`eclipse_launcher::i18n`): a dropdown of the system's language and the
//! ones the launcher has, each named in itself. A list rather than a segment per language: nine
//! segments took the row's whole width and left the row's own sentence a character wide.
use super::*;
use crate::ui;
use eclipse_launcher::i18n::Language;

/// What the dropdown shows for a choice: the system's, in the launcher's language with the
/// language it follows, or a language's name in itself (never through the table: a table entry
/// for "English" would rename the very entry a player looks for).
fn choice_name(choice: Option<Language>) -> String {
    match choice {
        None => trf(
            "System ({language})",
            &[("language", Language::system().name())],
        ),
        Some(language) => language.name().to_string(),
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
                )
                .on_press(
                    cx.listener(move |this, _: &ClickEvent, _, cx| this.pick_language(choice, cx)),
                ),
            );
        }
        // A solid backing: the backdrop doesn't paint under a popover whose rows scroll (seen on
        // the desktop run), and the rows must stay legible.
        let menu = ui::popover("language-menu")
            .bg(ui::surface_colour(ui::Surface::Float))
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
                .top(px(ui::Size::Md.height() + ui::MENU_GAP))
                .right_0()
                .child(gpui::anchored().anchor(gpui::Anchor::TopRight).child(menu)),
        )
        .with_priority(1)
    }

    pub(super) fn language_row(&self, cx: &mut Context<Self>) -> Div {
        let mut wrap = div().relative().child(
            ui::dropdown(
                "language-picker",
                None,
                choice_name(self.settings.language),
                self.language_menu,
                ui::Size::Md.height(),
            )
            .on_press(cx.listener(|this, event: &ClickEvent, window, cx| {
                if this.closed_by_this_press(event) {
                    return;
                }
                this.language_menu = !this.language_menu;
                if this.language_menu {
                    this.opened_menu(window, cx);
                }
                cx.notify();
            })),
        );
        if self.language_menu {
            wrap = wrap.child(self.language_list(cx));
        }
        ui::setting_row(tr("Language"), None, Some(wrap.into_any_element()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_language_is_named_in_itself_and_the_system_choice_names_the_one_it_follows() {
        assert_eq!(choice_name(Some(Language::Russian)), "Русский");
        assert_eq!(choice_name(Some(Language::English)), "English");
        let system = choice_name(None);
        assert!(system.contains(Language::system().name()), "{system}");
    }
}
