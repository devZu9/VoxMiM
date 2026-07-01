#![allow(dead_code)]
pub struct InputSimulation;

impl InputSimulation {
    pub fn new() -> Self {
        Self
    }

    pub fn key_click(&self, key: &str) {
        let _ = key;
        log::info!("Нажатие клавиши: {key}");
    }

    pub fn key_sequence(&self, keys: &[&str]) {
        let _ = keys;
        log::info!("Последовательность клавиш: {:?}", keys);
    }

    pub fn type_text(&self, text: &str) {
        let _ = text;
        log::info!("Ввод текста: {text}");
    }
}
