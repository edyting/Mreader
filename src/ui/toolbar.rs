//! Bottom reading toolbar.

use gtk::prelude::*;
use gtk::{Box as GtkBox, Button, DropDown, Label, Orientation, Scale, SpinButton, StringList};

use crate::pacer::modes::PacingMode;

pub struct BottomToolbar {
    pub root: GtkBox,
    pub play_btn: Button,
    pub stop_btn: Button,
    pub restart_btn: Button,
    pub prev_btn: Button,
    pub next_btn: Button,
    pub prev_paragraph_btn: Button,
    pub next_paragraph_btn: Button,
    pub speed_scale: Scale,
    pub wpm_label: Label,
    pub progress_label: Label,
    pub mode_dropdown: DropDown,
    pub view_toggle: Button,
    pub paragraph_selector: SpinButton,
}

impl BottomToolbar {
    pub fn new(default_wpm: u32) -> Self {
        let root = GtkBox::new(Orientation::Horizontal, 10);
        root.add_css_class("bottom-toolbar");
        root.set_hexpand(true);

        let prev_btn = Button::from_icon_name("go-previous-symbolic");
        prev_btn.set_tooltip_text(Some("Previous word or PDF page (←)"));
        let prev_paragraph_btn = Button::from_icon_name("go-up-symbolic");
        prev_paragraph_btn.set_tooltip_text(Some("Previous paragraph"));
        let play_btn = Button::from_icon_name("media-playback-start-symbolic");
        play_btn.set_tooltip_text(Some("Play / Pause (Space)"));
        let stop_btn = Button::from_icon_name("media-playback-stop-symbolic");
        stop_btn.set_tooltip_text(Some("Stop (Esc)"));
        let restart_btn = Button::from_icon_name("view-refresh-symbolic");
        restart_btn.set_tooltip_text(Some("Restart reading session"));
        let next_btn = Button::from_icon_name("go-next-symbolic");
        next_btn.set_tooltip_text(Some("Next word or PDF page (→)"));
        let next_paragraph_btn = Button::from_icon_name("go-down-symbolic");
        next_paragraph_btn.set_tooltip_text(Some("Next paragraph"));

        for b in [
            &prev_btn,
            &prev_paragraph_btn,
            &play_btn,
            &stop_btn,
            &restart_btn,
            &next_btn,
            &next_paragraph_btn,
        ] {
            b.add_css_class("circular");
            root.append(b);
        }

        let speed_scale = Scale::with_range(Orientation::Horizontal, 50.0, 1000.0, 10.0);
        speed_scale.set_value(default_wpm as f64);
        speed_scale.set_hexpand(true);
        speed_scale.set_draw_value(false);
        speed_scale.set_tooltip_text(Some("Reading speed (WPM)"));
        root.append(&speed_scale);

        let wpm_label = Label::new(Some(&format!("{default_wpm} WPM")));
        wpm_label.add_css_class("wpm-label");
        root.append(&wpm_label);

        let labels: Vec<&str> = PacingMode::all().iter().map(|m| m.label()).collect();
        let model = StringList::new(&labels);
        let mode_dropdown = DropDown::new(Some(model), Option::<&gtk::Expression>::None);
        mode_dropdown.set_tooltip_text(Some("Pacing mode"));
        root.append(&mode_dropdown);

        let view_toggle = Button::with_label("Focus View");
        view_toggle.set_tooltip_text(Some("Toggle Original / Focus view"));
        root.append(&view_toggle);

        let paragraph_label = Label::new(Some("¶"));
        paragraph_label.set_tooltip_text(Some("Jump to paragraph"));
        root.append(&paragraph_label);
        let paragraph_selector = SpinButton::with_range(1.0, 1.0, 1.0);
        paragraph_selector.set_sensitive(false);
        paragraph_selector.set_tooltip_text(Some("Jump to paragraph"));
        root.append(&paragraph_selector);

        let progress_label = Label::new(Some("0%"));
        progress_label.add_css_class("progress-label");
        root.append(&progress_label);

        Self {
            root,
            play_btn,
            stop_btn,
            restart_btn,
            prev_btn,
            next_btn,
            prev_paragraph_btn,
            next_paragraph_btn,
            speed_scale,
            wpm_label,
            progress_label,
            mode_dropdown,
            view_toggle,
            paragraph_selector,
        }
    }

    pub fn set_playing(&self, playing: bool) {
        let icon = if playing {
            "media-playback-pause-symbolic"
        } else {
            "media-playback-start-symbolic"
        };
        self.play_btn.set_icon_name(icon);
    }

    pub fn set_wpm(&self, wpm: u32) {
        self.wpm_label.set_text(&format!("{wpm} WPM"));
        if (self.speed_scale.value() as u32) != wpm {
            self.speed_scale.set_value(wpm as f64);
        }
    }

    pub fn set_progress(&self, progress: f64, word: usize, total: usize) {
        self.progress_label
            .set_text(&format!("{:.0}%  ({word}/{total})", progress * 100.0));
    }

    pub fn selected_mode(&self) -> PacingMode {
        let idx = self.mode_dropdown.selected() as usize;
        PacingMode::all().get(idx).copied().unwrap_or_default()
    }
}
