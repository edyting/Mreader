//! Left navigation sidebar.

use gtk::prelude::*;
use gtk::{Box as GtkBox, Button, Orientation, ScrolledWindow, ToggleButton};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarPage {
    Library,
    Recent,
    Bookmarks,
    History,
    Statistics,
    Settings,
}

pub struct Sidebar {
    pub root: GtkBox,
    pub library_btn: ToggleButton,
    pub recent_btn: ToggleButton,
    pub bookmarks_btn: ToggleButton,
    pub history_btn: ToggleButton,
    pub stats_btn: ToggleButton,
    pub settings_btn: ToggleButton,
    pub list_box: GtkBox,
}

impl Sidebar {
    pub fn new() -> Self {
        let root = GtkBox::new(Orientation::Vertical, 8);
        root.add_css_class("sidebar");
        root.set_size_request(220, -1);

        let library_btn = nav_toggle("Library", true);
        let recent_btn = nav_toggle("Recent", false);
        let bookmarks_btn = nav_toggle("Bookmarks", false);
        let history_btn = nav_toggle("History", false);
        let stats_btn = nav_toggle("Statistics", false);
        let settings_btn = nav_toggle("Settings", false);
        for button in [
            &recent_btn,
            &bookmarks_btn,
            &history_btn,
            &stats_btn,
            &settings_btn,
        ] {
            button.set_group(Some(&library_btn));
        }

        let nav = GtkBox::new(Orientation::Vertical, 2);
        for b in [
            &library_btn,
            &recent_btn,
            &bookmarks_btn,
            &history_btn,
            &stats_btn,
            &settings_btn,
        ] {
            b.set_hexpand(true);
            nav.append(b);
        }
        root.append(&nav);

        let scroll = ScrolledWindow::new();
        scroll.set_vexpand(true);
        scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        let list_box = GtkBox::new(Orientation::Vertical, 4);
        list_box.set_margin_top(8);
        scroll.set_child(Some(&list_box));
        root.append(&scroll);

        Self {
            root,
            library_btn,
            recent_btn,
            bookmarks_btn,
            history_btn,
            stats_btn,
            settings_btn,
            list_box,
        }
    }

    pub fn clear_list(&self) {
        while let Some(child) = self.list_box.first_child() {
            self.list_box.remove(&child);
        }
    }

    pub fn add_list_button(&self, label: &str) -> Button {
        let btn = Button::with_label(label);
        btn.add_css_class("library-row");
        btn.add_css_class("flat");
        btn.set_halign(gtk::Align::Fill);
        btn.set_hexpand(true);
        self.list_box.append(&btn);
        btn
    }

    pub fn set_active(&self, page: SidebarPage) {
        self.library_btn.set_active(page == SidebarPage::Library);
        self.recent_btn.set_active(page == SidebarPage::Recent);
        self.bookmarks_btn
            .set_active(page == SidebarPage::Bookmarks);
        self.history_btn.set_active(page == SidebarPage::History);
        self.stats_btn.set_active(page == SidebarPage::Statistics);
        self.settings_btn.set_active(page == SidebarPage::Settings);
    }
}

fn nav_toggle(label: &str, active: bool) -> ToggleButton {
    let btn = ToggleButton::with_label(label);
    btn.add_css_class("nav-button");
    btn.add_css_class("flat");
    btn.set_active(active);
    btn
}
