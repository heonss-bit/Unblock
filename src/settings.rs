//! Настройки приложения: лежат в %LOCALAPPDATA%\Unblock\settings.json.

use serde::{Deserialize, Serialize};
use std::fs;

use crate::bat::Services;
use crate::engines;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Both,
    Youtube,
    Discord,
}

impl Mode {
    pub fn services(self) -> Services {
        match self {
            Mode::Both => Services::Both,
            Mode::Youtube => Services::Youtube,
            Mode::Discord => Services::Discord,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Mode::Both => "YouTube и Discord",
            Mode::Youtube => "Только YouTube",
            Mode::Discord => "Только Discord",
        }
    }

    pub fn index(self) -> usize {
        match self {
            Mode::Both => 0,
            Mode::Youtube => 1,
            Mode::Discord => 2,
        }
    }

    pub fn from_index(i: usize) -> Mode {
        match i {
            1 => Mode::Youtube,
            2 => Mode::Discord,
            _ => Mode::Both,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    pub mode: Mode,
    pub strategy: String,
    pub secret: String,
    pub zapret_tag: String,
    pub tg_tag: String,
    /// Тёмная тема (иначе светлая).
    pub theme_dark: bool,
    /// Номер цвета акцента из ui::ACCENTS.
    pub accent: usize,
    /// Включать запрет и TG-прокси сразу при запуске приложения.
    pub auto_enable: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            mode: Mode::Both,
            strategy: "general.bat".to_string(),
            secret: String::new(),
            zapret_tag: String::new(),
            tg_tag: String::new(),
            theme_dark: true,
            accent: 0,
            auto_enable: false,
        }
    }
}

impl Settings {
    fn path() -> std::path::PathBuf {
        engines::data_dir().join("settings.json")
    }

    pub fn load() -> Settings {
        fs::read_to_string(Self::path())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let _ = fs::create_dir_all(engines::data_dir());
        if let Ok(t) = serde_json::to_string_pretty(self) {
            let _ = fs::write(Self::path(), t);
        }
    }
}
