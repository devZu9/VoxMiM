use regex::Regex;
use std::path::Path;
use std::sync::Mutex;

static CUSTOM_REGEXES: Mutex<Option<Vec<Regex>>> = Mutex::new(None);
static CUSTOM_MID_TOKENS: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn compile_end_regexes(patterns: &[String]) -> Vec<Regex> {
    patterns.iter()
        .filter_map(|p| {
            let escaped = regex::escape(p);
            Regex::new(&format!(r"(?i){}\s*$", escaped)).ok()
        })
        .collect()
}

fn default_phrases() -> Vec<&'static str> {
    vec![
        "субтитры создавал DimaTorzok",
        "субтитры создавал",
        "продолжение следует...",
        "Подписывайтесь на наш канал...",
        "[BLANK_AUDIO]",
        "[ Silence ]",
        "[MUSIC]",
        "[NOISE]",
        "[_BLANK_]",
        "Thank you.",
        "Thanks for watching!",
        "Subscribe!",
        "Please subscribe",
        "www.",
        "http://",
        "https://",
    ]
}

fn load_phrases_from_file(path: &Path) -> Vec<String> {
    let content = std::fs::read_to_string(path).unwrap_or_default();
    content
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.to_string())
        .collect()
}

fn sort_and_save(path: &Path, phrases: &[String], headers: &[String]) {
    let mut non_empty: Vec<String> = phrases.iter()
        .filter(|p| !p.trim().is_empty())
        .cloned()
        .collect();
    non_empty.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));

    let mut out = String::new();
    for h in headers {
        out.push_str(h);
        out.push('\n');
    }
    for p in &non_empty {
        out.push_str(p);
        out.push('\n');
    }
    let _ = std::fs::write(path, &out);
}

pub fn load_custom_phrases(path: &Path) {
    if !path.exists() {
        log::info!("hallucinations.txt не найден, создаю по умолчанию");
        let header = "# Фразы, удаляемые из конца распознанного текста (каждая на новой строке)
# Регистр не важен";
        let mut defaults: Vec<String> = default_phrases().iter().map(|s| s.to_string()).collect();
        defaults.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
        let content = defaults.join("\n");
        let full = format!("{}\n{}\n", header, content);
        let _ = std::fs::write(path, &full);
        // не делаем return — проваливаемся в чтение и компиляцию
    }

    let phrases = load_phrases_from_file(path);
    if phrases.is_empty() {
        log::warn!("{} пуст или содержит только пустые строки", path.display());
        return;
    }

    let regexes = compile_end_regexes(&phrases);
    if let Ok(mut guard) = CUSTOM_REGEXES.lock() {
        *guard = Some(regexes);
    }

    // Токены в скобках — удаляем и в середине текста
    let mid: Vec<String> = phrases.iter()
        .filter(|p| p.starts_with('[') && p.ends_with(']'))
        .cloned()
        .collect();
    if let Ok(mut guard) = CUSTOM_MID_TOKENS.lock() {
        *guard = mid;
    }

    log::info!("Загружено фраз галлюцинаций из {}", path.display());
}

pub fn add_custom_phrase(phrase: &str) {
    let phrase = phrase.trim();
    if phrase.is_empty() { return; }

    let path = crate::config::dicts_path().join("hallucinations.txt");

    // Читаем текущие строки (с комментариями и фразами)
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let mut headers: Vec<String> = Vec::new();
    let mut phrases: Vec<String> = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            headers.push(trimmed.to_string());
        } else if !trimmed.is_empty() {
            phrases.push(trimmed.to_string());
        }
    }

    // Если файла нет — ставим заголовки по умолчанию
    if headers.is_empty() {
        headers = vec![
            "# Фразы, удаляемые из конца распознанного текста (каждая на новой строке)".to_string(),
            "# Регистр не важен".to_string(),
        ];
        // Добавляем дефолтные фразы, которых ещё нет
        for d in default_phrases() {
            let d_str = d.to_string();
            if !phrases.contains(&d_str) {
                phrases.push(d_str);
            }
        }
    }

    // Добавляем новую фразу
    if !phrases.iter().any(|p| p == &phrase) {
        phrases.push(phrase.to_string());
    }

    // Сохраняем с сортировкой
    sort_and_save(&path, &phrases, &headers);

    load_custom_phrases(&path);
    log::info!("Добавлена фраза галлюцинации: «{phrase}»");
}

fn mid_text_tokens() -> Vec<&'static str> {
    vec![
        "[BLANK_AUDIO]", "[ Silence ]", "[MUSIC]",
        "[NOISE]", "[_BLANK_]",
    ]
}

pub fn remove_hallucinations(text: &str) -> String {
    let mut result = text.to_string();

    // Убираем метки, которые могут быть в середине текста
    for h in mid_text_tokens() {
        result = result.replace(h, "");
    }
    if let Ok(guard) = CUSTOM_MID_TOKENS.lock() {
        for token in guard.iter() {
            result = result.replace(token.as_str(), "");
        }
    }

    // Убираем инициалы + фамилию
    let re = Regex::new(r"\b[А-ЯA-Z][а-яa-z]+\s+[A-Z]\.\s*[А-ЯA-Z][а-яa-z]+\b").unwrap();
    result = re.replace_all(&result, "").to_string();
    let re = Regex::new(r"\b[A-Z]\.\s*[А-ЯA-Z][а-яa-z]+\b").unwrap();
    result = re.replace_all(&result, "").to_string();

    // Удаляем суффиксы из конца текста
    if let Ok(guard) = CUSTOM_REGEXES.lock() {
        if let Some(ref regexes) = *guard {
            for re in regexes {
                result = re.replace_all(&result, "").to_string();
            }
        }
    }

    let re = Regex::new(r"\s+").unwrap();
    result = re.replace_all(&result.trim(), " ").to_string();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init_test_data() {
        let phrases: Vec<String> = vec![
            "субтитры создавал DimaTorzok".into(),
            "субтитры создавал".into(),
            "продолжение следует...".into(),
            "subscribe!".into(),
        ];
        let regexes = compile_end_regexes(&phrases);
        *CUSTOM_REGEXES.lock().unwrap() = Some(regexes);
    }

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
        init_test_data();
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
        init_test_data();
        let result = remove_hallucinations("передай привет субтитры создавал");
        assert_eq!(result, "передай привет");
    }
}
