use regex::Regex;
use std::path::Path;
use std::sync::OnceLock;

static CUSTOM_PHRASES: OnceLock<Vec<Vec<String>>> = OnceLock::new();

fn common_hallucinations() -> Vec<&'static str> {
    vec![
        "[BLANK_AUDIO]",
        "[ Silence ]",
        "[MUSIC]",
        "[NOISE]",
        "[_BLANK_]",
        "www.",
        "http://",
        "https://",
        "Thank you.",
        "Thanks for watching!",
        "Subscribe!",
        "Please subscribe",
    ]
}

fn builtin_suffixes() -> Vec<Vec<&'static str>> {
    vec![
        vec!["субтитры", "создавал", "DimaTorzok"],
        vec!["Субтитры", "создавал", "DimaTorzok"],
        vec!["субтитры", "создавал", "Dimatorzok"],
        vec!["Субтитры", "создавал", "Dimatorzok"],
        vec!["субтитры", "создавал"],
        vec!["Субтитры", "создавал"],
        vec!["продолжение", "следует"],
        vec!["Продолжение", "следует"],
    ]
}

fn all_suffixes() -> Vec<Vec<String>> {
    let mut all: Vec<Vec<String>> = builtin_suffixes()
        .iter()
        .map(|s| s.iter().map(|w| w.to_string()).collect())
        .collect();
    if let Some(custom) = CUSTOM_PHRASES.get() {
        all.extend(custom.iter().cloned());
    }
    all
}

fn remove_suffix_hallucinations(words: &[&str]) -> Vec<String> {
    let mut result: Vec<String> = words.iter().map(|w| (*w).to_string()).collect();
    let suffixes = all_suffixes();

    loop {
        let mut removed = false;
        for suffix in &suffixes {
            if result.len() >= suffix.len() {
                let end: Vec<&str> = result[result.len() - suffix.len()..]
                    .iter()
                    .map(|s| s.as_str())
                    .collect();
                if end == suffix.as_slice() {
                    result.truncate(result.len() - suffix.len());
                    removed = true;
                    break;
                }
            }
        }
        if !removed {
            break;
        }
    }

    result
}

pub fn load_custom_phrases(path: &Path) {
    if !path.exists() {
        log::info!("hallucinations.txt не найден, создаю по умолчанию");
        let default = [
            "субтитры создавал",
            "продолжение следует",
        ];
        let content = default.join("\n");
        let _ = std::fs::write(path, &content);
        return;
    }

    match std::fs::read_to_string(path) {
        Ok(content) => {
            let phrases: Vec<Vec<String>> = content
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.split_whitespace().map(|w| w.to_string()).collect())
                .filter(|words: &Vec<String>| !words.is_empty())
                .collect();
            if !phrases.is_empty() {
                let _ = CUSTOM_PHRASES.set(phrases);
                log::info!("Загружено кастомных фраз галлюцинаций из {}", path.display());
            }
        }
        Err(e) => log::warn!("Не удалось прочитать {}: {e}", path.display()),
    }
}

pub fn remove_hallucinations(text: &str) -> String {
    let mut result = text.to_string();

    for h in common_hallucinations() {
        result = result.replace(h, "");
    }

    let re = Regex::new(r"\b[А-ЯA-Z][а-яa-z]+\s+[A-Z]\.\s*[А-ЯA-Z][а-яa-z]+\b").unwrap();
    result = re.replace_all(&result, "").to_string();
    let re = Regex::new(r"\b[A-Z]\.\s*[А-ЯA-Z][а-яa-z]+\b").unwrap();
    result = re.replace_all(&result, "").to_string();

    let words: Vec<&str> = result.split_whitespace().collect();
    if !words.is_empty() {
        let cleaned = remove_suffix_hallucinations(&words);
        result = cleaned.join(" ");
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_blank_audio() {
        let result = remove_hallucinations("привет [BLANK_AUDIO] мир");
        assert_eq!(result, "привет мир");
    }

    #[test]
    fn test_remove_corrector() {
        let result = remove_hallucinations("текст Корректор A. Егорова");
        assert_eq!(result, "текст");
    }

    #[test]
    fn test_remove_initial() {
        let result = remove_hallucinations("A. Семкин текст");
        assert_eq!(result, "текст");
    }

    #[test]
    fn test_keep_normal() {
        let result = remove_hallucinations("привет мир");
        assert_eq!(result, "привет мир");
    }

    #[test]
    fn test_remove_subtitles_end() {
        let result = remove_hallucinations("как устроен агент субтитры создавал DimaTorzok");
        assert_eq!(result, "как устроен агент");
    }

    #[test]
    fn test_keep_middle_not_removed() {
        let result = remove_hallucinations("я люблю смотреть фильмы с субтитрами");
        assert_eq!(result, "я люблю смотреть фильмы с субтитрами");
    }

    #[test]
    fn test_remove_multi_pass() {
        let result = remove_hallucinations("передай привет субтитры создавал");
        assert_eq!(result, "передай привет");
    }
}
