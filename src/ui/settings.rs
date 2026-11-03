//! Settings panel.

use gtk::prelude::*;
use gtk::{
    Box as GtkBox, DropDown, Label, Orientation, Scale, ScrolledWindow, SpinButton, StringList,
    Switch,
};

use crate::config::{AppConfig, ReadingTheme, TextAlignment};
use crate::pacer::modes::PacingMode;

pub struct SettingsPanel {
    pub root: ScrolledWindow,
    pub content: GtkBox,
    pub font_family: DropDown,
    pub font_size: SpinButton,
    pub line_spacing: SpinButton,
    pub paragraph_spacing: SpinButton,
    pub max_width: SpinButton,
    pub alignment: DropDown,
    pub theme: DropDown,
    pub pacing_mode: DropDown,
    pub default_wpm: SpinButton,
    pub phrase_size: SpinButton,
    pub auto_para: Switch,
    pub paragraph_pause: SpinButton,
    pub rsvp_transition: Switch,
    pub rsvp_focal: Switch,
    pub daily_goal: SpinButton,
    pub highlight_opacity: Scale,
}

impl SettingsPanel {
    pub fn new(cfg: &AppConfig) -> Self {
        let content = GtkBox::new(Orientation::Vertical, 12);
        content.set_margin_top(16);
        content.set_margin_bottom(16);
        content.set_margin_start(24);
        content.set_margin_end(24);
        content.set_hexpand(true);

        let title = Label::new(Some("Settings"));
        title.add_css_class("reading-title");
        title.set_halign(gtk::Align::Start);
        content.append(&title);

        let fonts = StringList::new(&[
            "Liberation Serif",
            "DejaVu Serif",
            "FreeSerif",
            "Noto Serif",
            "Source Serif 4",
            "Literata",
            "Cantarell",
            "DejaVu Sans",
        ]);
        let font_family = DropDown::new(Some(fonts), Option::<&gtk::Expression>::None);
        select_string(&font_family, &cfg.font_family);
        add_row(&content, "Font family", &font_family);

        let font_size = SpinButton::with_range(10.0, 48.0, 1.0);
        font_size.set_value(cfg.font_size);
        add_row(&content, "Font size", &font_size);

        let line_spacing = SpinButton::with_range(1.0, 3.0, 0.1);
        line_spacing.set_digits(1);
        line_spacing.set_value(cfg.line_spacing);
        add_row(&content, "Line spacing", &line_spacing);

        let paragraph_spacing = SpinButton::with_range(0.5, 3.0, 0.1);
        paragraph_spacing.set_digits(1);
        paragraph_spacing.set_value(cfg.paragraph_spacing);
        add_row(&content, "Paragraph spacing", &paragraph_spacing);

        let max_width = SpinButton::with_range(400.0, 1400.0, 20.0);
        max_width.set_value(cfg.max_reading_width);
        add_row(&content, "Max reading width", &max_width);

        let alignment = DropDown::new(
            Some(StringList::new(&["Left", "Center", "Justify"])),
            Option::<&gtk::Expression>::None,
        );
        alignment.set_selected(match cfg.text_alignment {
            TextAlignment::Left => 0,
            TextAlignment::Center => 1,
            TextAlignment::Justify => 2,
        });
        add_row(&content, "Text alignment", &alignment);

        let theme = DropDown::new(
            Some(StringList::new(&["Light", "Dark", "Sepia"])),
            Option::<&gtk::Expression>::None,
        );
        theme.set_selected(match cfg.theme {
            ReadingTheme::Light => 0,
            ReadingTheme::Dark => 1,
            ReadingTheme::Sepia => 2,
        });
        add_row(&content, "Reading theme", &theme);

        let mode_labels: Vec<&str> = PacingMode::all().iter().map(|m| m.label()).collect();
        let pacing_mode = DropDown::new(
            Some(StringList::new(&mode_labels)),
            Option::<&gtk::Expression>::None,
        );
        if let Some(idx) = PacingMode::all().iter().position(|m| *m == cfg.pacing_mode) {
            pacing_mode.set_selected(idx as u32);
        }
        add_row(&content, "Default pacing mode", &pacing_mode);

        let default_wpm = SpinButton::with_range(50.0, 1000.0, 10.0);
        default_wpm.set_value(cfg.default_wpm as f64);
        add_row(&content, "Default WPM", &default_wpm);

        let phrase_size = SpinButton::with_range(2.0, 12.0, 1.0);
        phrase_size.set_value(cfg.phrase_group_size as f64);
        add_row(&content, "Words per phrase group", &phrase_size);

        let auto_para = Switch::new();
        auto_para.set_active(cfg.auto_advance_paragraph);
        add_row(&content, "Auto paragraph advance", &auto_para);

        let paragraph_pause = SpinButton::with_range(0.0, 2000.0, 50.0);
        paragraph_pause.set_value(cfg.paragraph_pause_ms as f64);
        add_row(&content, "Paragraph pause (ms)", &paragraph_pause);

        let rsvp_transition = Switch::new();
        rsvp_transition.set_active(cfg.rsvp_transition);
        add_row(&content, "RSVP transition", &rsvp_transition);

        let rsvp_focal = Switch::new();
        rsvp_focal.set_active(cfg.rsvp_focal_letter);
        add_row(&content, "RSVP focal letter", &rsvp_focal);

        let daily_goal = SpinButton::with_range(5.0, 240.0, 5.0);
        daily_goal.set_value(cfg.daily_goal_minutes as f64);
        add_row(&content, "Daily reading goal (minutes)", &daily_goal);

        let highlight_opacity = Scale::with_range(Orientation::Horizontal, 0.1, 1.0, 0.05);
        highlight_opacity.set_value(cfg.appearance.highlight_opacity);
        highlight_opacity.set_draw_value(true);
        highlight_opacity.set_hexpand(true);
        add_row(&content, "Highlight opacity", &highlight_opacity);

        let shortcuts = Label::new(Some(
            "Keyboard shortcuts\n\
             Space Play/Pause · ← → Word · ↑ ↓ Speed · Esc Stop\n\
             Ctrl+O Open · Ctrl++ / Ctrl+- Font size · Ctrl+B Bookmark",
        ));
        shortcuts.set_halign(gtk::Align::Start);
        shortcuts.set_wrap(true);
        content.append(&shortcuts);

        let root = ScrolledWindow::new();
        root.set_child(Some(&content));
        root.set_vexpand(true);
        root.set_hexpand(true);

        Self {
            root,
            content,
            font_family,
            font_size,
            line_spacing,
            paragraph_spacing,
            max_width,
            alignment,
            theme,
            pacing_mode,
            default_wpm,
            phrase_size,
            auto_para,
            paragraph_pause,
            rsvp_transition,
            rsvp_focal,
            daily_goal,
            highlight_opacity,
        }
    }

    pub fn apply_to_config(&self, cfg: &mut AppConfig) {
        if let Some(s) = self
            .font_family
            .selected_item()
            .and_then(|o| o.downcast::<gtk::StringObject>().ok())
        {
            cfg.font_family = s.string().to_string();
        }
        cfg.font_size = self.font_size.value();
        cfg.line_spacing = self.line_spacing.value();
        cfg.paragraph_spacing = self.paragraph_spacing.value();
        cfg.max_reading_width = self.max_width.value();
        cfg.text_alignment = match self.alignment.selected() {
            1 => TextAlignment::Center,
            2 => TextAlignment::Justify,
            _ => TextAlignment::Left,
        };
        cfg.theme = match self.theme.selected() {
            1 => ReadingTheme::Dark,
            2 => ReadingTheme::Sepia,
            _ => ReadingTheme::Light,
        };
        cfg.pacing_mode = PacingMode::all()
            .get(self.pacing_mode.selected() as usize)
            .copied()
            .unwrap_or_default();
        cfg.default_wpm = AppConfig::clamp_wpm(self.default_wpm.value() as u32);
        cfg.phrase_group_size = self.phrase_size.value() as usize;
        cfg.auto_advance_paragraph = self.auto_para.is_active();
        cfg.paragraph_pause_ms = self.paragraph_pause.value() as u64;
        cfg.rsvp_transition = self.rsvp_transition.is_active();
        cfg.rsvp_focal_letter = self.rsvp_focal.is_active();
        cfg.daily_goal_minutes = self.daily_goal.value() as u32;
        cfg.appearance.highlight_opacity = self.highlight_opacity.value();
    }
}

fn add_row(parent: &GtkBox, label: &str, widget: &impl IsA<gtk::Widget>) {
    let row = GtkBox::new(Orientation::Horizontal, 12);
    row.add_css_class("settings-row");
    let lab = Label::new(Some(label));
    lab.set_halign(gtk::Align::Start);
    lab.set_hexpand(true);
    row.append(&lab);
    row.append(widget);
    parent.append(&row);
}

fn select_string(dropdown: &DropDown, value: &str) {
    if let Some(model) = dropdown.model() {
        let n = model.n_items();
        for i in 0..n {
            if let Some(obj) = model.item(i) {
                if let Ok(s) = obj.downcast::<gtk::StringObject>() {
                    if s.string() == value {
                        dropdown.set_selected(i);
                        return;
                    }
                }
            }
        }
    }
}
