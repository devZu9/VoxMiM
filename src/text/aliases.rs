#![allow(dead_code)]
use std::collections::HashMap;

#[derive(serde::Deserialize)]
struct AliasesFile {
    ru: Option<HashMap<String, String>>,
    en: Option<HashMap<String, String>>,
}

pub struct Aliases {
    map: HashMap<String, String>,
}

impl Aliases {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn load<P: AsRef<std::path::Path>>(&mut self, path: P) {
        let path = path.as_ref();
        if !path.exists() {
            log::info!("Файл алиасов не найден: {}", path.display());
            return;
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("Не удалось прочитать алиасы: {e}");
                return;
            }
        };

        self.map.clear();

        if let Ok(flat) = serde_json::from_str::<HashMap<String, String>>(&content) {
            for (k, v) in flat {
                self.map.insert(k.to_lowercase(), v.to_lowercase());
            }
            return;
        }

        if let Ok(lang) = serde_json::from_str::<AliasesFile>(&content) {
            if let Some(ru) = lang.ru {
                for (k, v) in ru {
                    self.map.insert(k.to_lowercase(), v.to_lowercase());
                }
            }
            if let Some(en) = lang.en {
                for (k, v) in en {
                    self.map.insert(k.to_lowercase(), v.to_lowercase());
                }
            }
            return;
        }

        log::warn!("Неизвестный формат файла алиасов");
    }
}
