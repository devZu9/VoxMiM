use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;


struct CachedEntry {
    regex: Regex,
    value: String,
}

pub struct UserDict {
    map: RwLock<HashMap<String, String>>,
    path: RwLock<PathBuf>,
    cache: RwLock<Vec<CachedEntry>>,
}

impl UserDict {
    pub fn new() -> Self {
        Self {
            map: RwLock::new(HashMap::new()),
            path: RwLock::new(PathBuf::new()),
            cache: RwLock::new(Vec::new()),
        }
    }

    pub fn load<P: AsRef<Path>>(&self, path: P) {
        let path = path.as_ref().to_path_buf();
        let mut map = HashMap::new();

        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    match serde_json::from_str::<HashMap<String, String>>(&content) {
                        Ok(parsed) => {
                            for (k, v) in parsed {
                                map.insert(k.to_lowercase(), v);
                            }
                            log::info!("Пользовательский словарь: {} записей", map.len());
                        }
                        Err(e) => log::warn!("Ошибка парсинга user_dict.json: {e}"),
                    }
                }
                Err(e) => log::warn!("Ошибка чтения user_dict.json: {e}"),
            }
        } else {
            log::info!("Пользовательский словарь не найден, создам при добавлении");
        }

        if let Ok(mut m) = self.map.write() {
            *m = map;
        }
        if let Ok(mut p) = self.path.write() {
            *p = path;
        }

        self.rebuild_cache();
    }

    fn rebuild_cache(&self) {
        let guard = self.map.read().unwrap();
        let mut keys: Vec<&String> = guard.keys().collect();
        keys.sort_by(|a, b| b.len().cmp(&a.len()));

        let mut cached = Vec::new();
        for key in keys {
            let value = match guard.get(key) {
                Some(v) => v.clone(),
                None => continue,
            };

            let words: Vec<String> = key.split_whitespace()
                .map(regex::escape)
                .collect();
            let pattern = words.join(r"\s+");
            let full = format!("(?i){}", pattern);

            match Regex::new(&full) {
                Ok(re) => cached.push(CachedEntry { regex: re, value }),
                Err(e) => log::warn!("user_dict: ошибка regex для «{key}»: {e}"),
            }
        }

        if let Ok(mut c) = self.cache.write() {
            *c = cached;
        }
    }

    pub fn apply(&self, text: &str) -> String {
        let cache = self.cache.read().unwrap();
        if cache.is_empty() {
            log::debug!("user_dict: пуст, пропускаем");
            return text.to_string();
        }

        log::debug!("user_dict: записей в кеше {}", cache.len());

        let mut result = String::with_capacity(text.len());
        let mut last_end = 0;

        for entry in cache.iter() {
            for m in entry.regex.find_iter(text) {
                if m.start() < last_end {
                    continue;
                }

                let left_ok = match text[..m.start()].chars().last() {
                    Some(c) => !c.is_alphabetic(),
                    None => true,
                };
                let right_ok = match text[m.end()..].chars().next() {
                    Some(c) => !c.is_alphabetic(),
                    None => true,
                };

                if left_ok && right_ok {
                    log::info!("user_dict: найдено «{}» → «{}» на byte[{},{}]",
                        &text[m.start()..m.end()], entry.value, m.start(), m.end());
                    result.push_str(&text[last_end..m.start()]);
                    result.push_str(&entry.value);
                    last_end = m.end();
                }
            }
        }

        result.push_str(&text[last_end..]);

        if result != text {
            log::info!("user_dict: замена: «{text}» → «{result}»");
        }

        result
    }

    pub fn add_entry(&self, wrong: &str, correct: &str) {
        let wrong = wrong.trim();
        let correct = correct.trim();
        if wrong.is_empty() || correct.is_empty() {
            return;
        }

        let key = wrong.to_lowercase();
        if let Ok(mut map) = self.map.write() {
            map.insert(key, correct.to_string());
        }

        self.rebuild_cache();
        self.save();
    }

    pub fn save(&self) {
        let path = self.path.read().unwrap().clone();
        if path.as_os_str().is_empty() {
            return;
        }

        let guard = self.map.read().unwrap();

        // Сортируем: сначала ASCII (английские), потом Cyrillic, по алфавиту внутри групп
        let mut entries: Vec<(&str, &str)> = guard.iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        entries.sort_by(|a, b| {
            let a_is_ascii = a.0.chars().all(|c| c.is_ascii());
            let b_is_ascii = b.0.chars().all(|c| c.is_ascii());
            if a_is_ascii != b_is_ascii {
                return if a_is_ascii { std::cmp::Ordering::Less } else { std::cmp::Ordering::Greater };
            }
            a.0.cmp(b.0)
        });

        // serde_json::Map сохраняет порядок вставки (IndexMap)
        let map: serde_json::Map<String, serde_json::Value> = entries.iter()
            .map(|(k, v)| (k.to_string(), serde_json::Value::String(v.to_string())))
            .collect();
        let value = serde_json::Value::Object(map);

        match serde_json::to_string_pretty(&value) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&path, json) {
                    log::error!("Ошибка сохранения user_dict.json: {e}");
                }
            }
            Err(e) => log::error!("Ошибка сериализации user_dict.json: {e}"),
        }
    }

    #[allow(dead_code)]
    pub fn reload(&self) {
        let path = self.path.read().unwrap().clone();
        if path.as_os_str().is_empty() {
            return;
        }
        self.load(&path);
    }

    pub fn path(&self) -> PathBuf {
        self.path.read().unwrap().clone()
    }
}
