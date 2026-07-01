use regex::Regex;

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

pub fn remove_hallucinations(text: &str) -> String {
    let mut result = text.to_string();

    for h in common_hallucinations() {
        result = result.replace(h, "");
    }

    let re = Regex::new(r"\b[А-ЯA-Z][а-яa-z]+\s+[A-Z]\.\s*[А-ЯA-Z][а-яa-z]+\b").unwrap();
    result = re.replace_all(&result, "").to_string();
    let re = Regex::new(r"\b[A-Z]\.\s*[А-ЯA-Z][а-яa-z]+\b").unwrap();
    result = re.replace_all(&result, "").to_string();

    result = result
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

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
}
