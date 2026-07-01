#![allow(dead_code)]
pub struct SettingsWindow;

impl SettingsWindow {
    pub fn new() -> Self {
        Self
    }

    pub fn open(&self) {
        log::info!("Открытие окна настроек");
    }
}
