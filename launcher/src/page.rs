use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Page {
    Home,
    Versions,
    /// Mods, resource packs and modpacks (imported ones and the browser's).
    Mods,
    Servers,
    Notes,
    Settings,
    InstanceEditor,
}

impl Page {
    pub(super) const NAV: [Page; 6] = [
        Page::Home,
        Page::Mods,
        Page::Servers,
        Page::Versions,
        Page::Notes,
        Page::Settings,
    ];

    pub(super) fn label(self) -> &'static str {
        match self {
            Page::Home => tr("Home"),
            Page::Mods => tr("Mods"),
            Page::Servers => tr("Servers"),
            Page::Versions => tr("Versions"),
            Page::Notes => tr("Patch notes"),
            Page::Settings => tr("Settings"),
            Page::InstanceEditor => tr("New instance"),
        }
    }

    pub(super) fn title(self) -> &'static str {
        match self {
            Page::Versions => tr("Eclipse versions"),
            page => page.label(),
        }
    }

    /// Settings and the New instance page scroll their own pane under a still title.
    pub(super) fn scrolls_itself(self) -> bool {
        matches!(self, Page::Settings | Page::InstanceEditor)
    }

    pub(super) fn icon(self) -> &'static str {
        match self {
            Page::Home => crate::icons::HOME,
            Page::Mods => crate::icons::MODS,
            Page::Servers => crate::icons::SERVER,
            Page::Versions => crate::icons::VERSION,
            Page::Notes => crate::icons::NOTES,
            Page::Settings => crate::icons::SETTINGS,
            Page::InstanceEditor => crate::icons::NEW_INSTANCE,
        }
    }
}

impl Launcher {
    pub(super) fn page_pane(&self) -> &gpui::ScrollHandle {
        match self.page {
            Page::Settings => &self.settings_ui.pane.handle,
            Page::InstanceEditor => match &self.instances.draft {
                Some(d) => &d.scroll.handle,
                None => &self.content_scroll.handle,
            },
            _ => &self.content_scroll.handle,
        }
    }

    pub(super) fn page_key(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match (self.page, key) {
            (Page::Home, "escape") => self.screenshot_escape(),
            (Page::Home, "left") => self.screenshot_step(false),
            (Page::Home, "right") => self.screenshot_step(true),
            (Page::Mods, "escape") => self.mods_escape(window, cx),
            (Page::Servers, "escape") => self.servers_escape(),
            (Page::InstanceEditor, "escape") => self.instance_escape(),
            _ => false,
        }
    }

    pub(super) fn page_opened(&mut self) {
        self.mods_sticky.reset();
        if self.page == Page::Mods {
            self.mods.reset_lists();
        }
    }

    /// A scrolled page is drawn from its layer, not rendered: the Mods page's card asks, once
    /// the offset has moved, whether it must follow.
    pub(super) fn page_scrolled(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.page == Page::Mods {
            self.mods_scrolled(window, cx);
        }
    }

    pub(super) fn page_float_open(&self) -> bool {
        self.page == Page::Mods && self.mod_detail_floats()
    }

    pub(super) fn close_page_float(&mut self, window: &mut Window, cx: &mut App) {
        if self.page_float_open() {
            self.close_mod_detail(window, cx);
        }
    }

    pub(super) fn page_dialog(&mut self, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        match self.page {
            Page::Servers => self.create_dialog(cx),
            _ => None,
        }
    }

    pub(super) fn page_viewer(&mut self, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        match self.page {
            Page::Home => self.screenshot_viewer(cx),
            _ => None,
        }
    }
}
