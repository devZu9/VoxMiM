use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

static LOCALE: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

// Встроенные локали (запасной вариант, если файлов нет рядом с .exe)
const EMBEDDED_RU: &str = include_str!("../lang/ru.json");
const EMBEDDED_EN: &str = include_str!("../lang/en.json");

pub fn load_locale(lang: &str) {
    // Приоритет 1: рядом с exe
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("lang")))
        .unwrap_or_else(|| PathBuf::from("lang"));

    let path = exe_dir.join(format!("{lang}.json"));
    if let Ok(content) = std::fs::read_to_string(&path) {
        if let Ok(map) = serde_json::from_str::<HashMap<String, String>>(&content) {
            *LOCALE.lock().unwrap() = Some(map);
            log::info!("Локаль загружена: {}", path.display());
            return;
        }
    }

    // Приоритет 2: встроенная
    let embedded = match lang {
        "ru" => EMBEDDED_RU,
        _ => EMBEDDED_EN,
    };
    if let Ok(map) = serde_json::from_str::<HashMap<String, String>>(embedded) {
        *LOCALE.lock().unwrap() = Some(map);
        log::info!("Локаль (встроенная): {lang}");
        return;
    }

    log::warn!("Локаль '{lang}' не найдена");
}

pub fn t(key: &str) -> String {
    LOCALE
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|m| m.get(key))
        .cloned()
        .unwrap_or_else(|| {
            log::warn!("Ключ локали не найден: {key}");
            key.to_string()
        })
}

pub fn t_utf16(key: &str) -> Vec<u16> {
    format!("{}\0", t(key)).encode_utf16().collect()
}

#[allow(dead_code)]
pub fn has_locale() -> bool {
    LOCALE.lock().unwrap().is_some()
}
