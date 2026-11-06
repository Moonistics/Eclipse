//! Settings' Language row (`eclipse_launcher::i18n`): a dropdown of the system's language and the
//! ones the launcher has, each named in itself. A list rather than a segment per language: nine
//! segments took the row's whole width and left the row's own sentence a character wide.
use super::*;
use crate::menus::Menu;
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
        self.close_menu();
        cx.notify();
    }

    /// The list: a popover like the others, its rows scrolling past ten.
    fn language_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut menu = self.menu_popover("language-menu", cx);
        for choice in std::iter::once(None).chain(Language::ALL.map(Some)) {
            let id = choice.map_or("system", Language::code);
            menu = menu.child(
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
        ui::menu_under(ui::ButtonSize::Md.height(), ui::Align::End, menu)
    }

    pub(super) fn language_row(&self, cx: &mut Context<Self>) -> Div {
        let picker = ui::dropdown(
            "language-picker",
            tr("Launcher language"),
            None,
            choice_name(self.settings.language),
            self.menu_is(&Menu::Language),
            ui::ButtonSize::Md,
        );
        let mut wrap = div()
            .relative()
            .child(self.menu_opener(picker, Menu::Language, cx));
        if self.menu_is(&Menu::Language) {
            wrap = wrap.child(self.language_list(cx));
        }
        ui::setting_row(tr("Launcher language"), None, Some(wrap.into_any_element()))
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
