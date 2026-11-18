//! Настройки приложения: лежат в %LOCALAPPDATA%\Unblock\settings.json.

use serde::{Deserialize, Serialize};
use std::fs;

use crate::engines;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
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
    /// Крестик окна убирает Unblock в трей, а не закрывает.
    pub close_to_tray: bool,
    /// Вопрос «Установить TG-прокси?» уже задавали (он нужен только при первом запуске).
    pub tg_prompt_done: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            strategy: "general.bat".to_string(),
            secret: String::new(),
            zapret_tag: String::new(),
            tg_tag: String::new(),
            theme_dark: true,
            accent: 0,
            auto_enable: false,
            close_to_tray: true,
            tg_prompt_done: false,
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
