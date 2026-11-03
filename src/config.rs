//! Application configuration and user preferences.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::pacer::modes::PacingMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ReadingTheme {
    #[default]
    Light,
    Dark,
    Sepia,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TextAlignment {
    #[default]
    Left,
    Center,
    Justify,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacerAppearance {
    pub highlight_color: String,
    pub underline_color: String,
    pub pointer_color: String,
    pub highlight_opacity: f64,
    pub underline_thickness: f64,
    pub pointer_size: f64,
    pub animation_speed: f64,
    pub background_color: String,
    pub text_color: String,
}

impl Default for PacerAppearance {
    fn default() -> Self {
        Self {
            highlight_color: "#e67e22".into(),
            underline_color: "#2980b9".into(),
            pointer_color: "#c0392b".into(),
            highlight_opacity: 0.35,
            underline_thickness: 2.5,
            pointer_size: 10.0,
            animation_speed: 1.0,
            background_color: "#faf9f7".into(),
            text_color: "#1a1a1a".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub font_family: String,
    pub font_size: f64,
    pub line_spacing: f64,
    pub paragraph_spacing: f64,
    pub max_reading_width: f64,
    pub text_alignment: TextAlignment,
    pub theme: ReadingTheme,
    pub pacing_mode: PacingMode,
    pub default_wpm: u32,
    pub phrase_group_size: usize,
    pub auto_advance_paragraph: bool,
    pub auto_advance_page: bool,
    pub paragraph_pause_ms: u64,
    pub rsvp_transition: bool,
    pub rsvp_focal_letter: bool,
    pub daily_goal_minutes: u32,
    pub appearance: PacerAppearance,
    pub window_width: i32,
    pub window_height: i32,
    pub sidebar_visible: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            font_family: "Liberation Serif".into(),
            font_size: 18.0,
            line_spacing: 1.7,
            paragraph_spacing: 1.2,
            max_reading_width: 720.0,
            text_alignment: TextAlignment::Left,
            theme: ReadingTheme::Light,
            pacing_mode: PacingMode::WordHighlight,
            default_wpm: 200,
            phrase_group_size: 3,
            auto_advance_paragraph: true,
            auto_advance_page: true,
            paragraph_pause_ms: 400,
            rsvp_transition: true,
            rsvp_focal_letter: true,
            daily_goal_minutes: 30,
            appearance: PacerAppearance::default(),
            window_width: 1280,
            window_height: 800,
            sidebar_visible: true,
        }
    }
}

impl AppConfig {
    pub fn config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("mreader")
    }

    pub fn data_dir() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("mreader")
    }

    pub(crate) fn legacy_config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("rustpace")
    }

    pub(crate) fn legacy_data_dir() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("rustpace")
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    pub fn load() -> Self {
        for path in [
            Self::config_path(),
            Self::legacy_config_dir().join("config.toml"),
        ] {
            if let Ok(contents) = fs::read_to_string(path) {
                if let Ok(config) = toml::from_str(&contents) {
                    return config;
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let dir = Self::config_dir();
        fs::create_dir_all(&dir)?;
        let contents = toml::to_string_pretty(self)?;
        fs::write(Self::config_path(), contents)?;
        Ok(())
    }

    pub fn clamp_wpm(wpm: u32) -> u32 {
        wpm.clamp(50, 1000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_wpm_bounds() {
        assert_eq!(AppConfig::clamp_wpm(10), 50);
        assert_eq!(AppConfig::clamp_wpm(200), 200);
        assert_eq!(AppConfig::clamp_wpm(5000), 1000);
    }

    #[test]
    fn default_roundtrip_toml() {
        let cfg = AppConfig::default();
        let s = toml::to_string(&cfg).unwrap();
        let back: AppConfig = toml::from_str(&s).unwrap();
        assert_eq!(back.default_wpm, 200);
        assert_eq!(back.phrase_group_size, 3);
    }
}
