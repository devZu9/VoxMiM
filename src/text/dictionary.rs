use std::collections::HashSet;
use std::sync::RwLock;

pub struct Dictionary {
    words: RwLock<HashSet<String>>,
}

impl Dictionary {
    #[allow(dead_code)]
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
        }
    }
}
