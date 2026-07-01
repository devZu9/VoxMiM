use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::RwLock;

pub struct Dictionary {
    words: RwLock<HashSet<String>>,
    path: RwLock<String>,
}

impl Dictionary {
    pub fn new() -> Self {
        Self {
            words: RwLock::new(HashSet::new()),
            path: RwLock::new(String::new()),
        }
    }

    pub fn load_lang(&self, lang: &str) {
        let fname = format!("{lang}_words_utf8.txt");

        let mut set = HashSet::new();
        let paths = [
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.join("dicts").join(&fname))),
            Some(std::path::PathBuf::from("dicts").join(&fname)),
        ];

        let mut loaded = false;
        for p in paths.into_iter().flatten() {
            if !p.exists() { continue; }
            match std::fs::read_to_string(&p) {
                Ok(content) => {
                    for line in content.lines() {
                        let word = line.trim().to_lowercase();
                        if !word.is_empty() {
                            set.insert(word);
                        }
                    }
                    if let Ok(mut pw) = self.path.write() {
                        *pw = p.to_string_lossy().to_string();
                    }
                    loaded = true;
                    break;
                }
                Err(e) => log::warn!("Словарь {}: {e}", p.display()),
            }
        }

        if let Ok(mut w) = self.words.write() {
            *w = set;
        }
        if loaded {
            log::info!("Словарь ({lang}): {} слов", {
                let w = self.words.read().unwrap();
                w.len()
            });
        } else {
            log::warn!("Словарь {lang} не найден (искал dicts/{fname})");
        }
    }

    #[allow(dead_code)]
    pub fn reload(&self) {
        let lang = self.path.read().map(|p| {
            std::path::Path::new(&*p)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("ru")
                .trim_end_matches("_words_utf8")
                .to_string()
        }).unwrap_or_else(|_| "ru".to_string());
        self.load_lang(&lang);
    }

    pub fn contains(&self, word: &str) -> bool {
        if let Ok(words) = self.words.read() {
            words.contains(&word.to_lowercase())
        } else {
            false
        }
    }

    #[cfg(test)]
    pub fn from_words(words: &[&str]) -> Self {
        let set: HashSet<String> = words.iter().map(|w| w.to_lowercase()).collect();
        Self {
            words: RwLock::new(set),
            path: RwLock::new("test".to_string()),
        }
    }
}

/// Статические словарные замены (питон → Python и т.д.)
pub fn apply_dict(text: &str) -> String {
    let dict: HashMap<&str, &str> = [
        ("питон", "Python"),
        ("виспер", "Whisper"),
        ("виндос", "Windows"),
        ("линукс", "Linux"),
        ("макос", "macOS"),
        ("плюс плюс", "C++"),
        ("шарп", "C#"),
        ("джава", "Java"),
        ("джаваскрипт", "JavaScript"),
        ("тайпскрипт", "TypeScript"),
        ("пи эйч пи", "PHP"),
        ("раст", "Rust"),
        ("винда", "Windows"),
        ("вскод", "VS Code"),
        ("вэб", "Web"),
        ("дизайн", "Design"),
        ("апи", "API"),
        ("айпи", "IP"),
        ("юзер", "User"),
        ("дев", "Dev"),
    ].into_iter().collect();

    let mut result = text.to_string();
    for (rus, eng) in &dict {
        let p = format!(" {} ", rus);
        let r = format!(" {} ", eng);
        result = result.replace(&p, &r);
    }
    result
}
