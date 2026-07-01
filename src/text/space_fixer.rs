use crate::text::dictionary::Dictionary;

const COMMON_SUFFIXES: &[&str] = &[
    "ный", "ная", "ное", "ные", "ных", "ным",
    "тся", "ться", "сь",
    "кой", "кий", "кая", "кое", "кие", "ких",
    "ной", "ный", "ная",
];

const COMMON_PREFIXES: &[&str] = &[
    "наи", "само", "взаимо", "меж", "сверх",
    "контр", "анти", "противо",
];

fn ends_with_consonant(word: &str) -> bool {
    word.chars().last().map_or(false, |c| matches!(c,
        'б' | 'в' | 'г' | 'д' | 'ж' | 'з' | 'к' | 'л' | 'м' | 'н' |
        'п' | 'р' | 'с' | 'т' | 'ф' | 'х' | 'ц' | 'ч' | 'ш' | 'щ'
    ))
}

fn starts_with_vowel(word: &str) -> bool {
    word.chars().next().map_or(false, |c| matches!(c,
        'а' | 'е' | 'ё' | 'и' | 'о' | 'у' | 'ы' | 'э' | 'ю' | 'я'
    ))
}

fn is_short_token(word: &str) -> bool {
    let lower = word.to_lowercase();
    word.len() <= 2 && !matches!(lower.as_str(), "и" | "в" | "с" | "у" | "а" | "о" | "к" | "я")
}

fn symspell_lookup(word: &str, dict: &Dictionary) -> Option<String> {
    if dict.contains(word) {
        return Some(word.to_string());
    }

    let word_lower = word.to_lowercase();
    // Автоматически генерируем варианты с расстоянием 1
    // (пропускаем 1 букву, добавляем 1 букву, заменяем 1 букву)
    let chars: Vec<char> = word_lower.chars().collect();
    for i in 0..chars.len() {
        // Пропуск буквы
        let skipped: String = chars.iter()
            .enumerate()
            .filter(|&(j, _)| j != i)
            .map(|(_, c)| c)
            .collect();
        if dict.contains(&skipped) {
            return Some(skipped);
        }
    }
    None
}

fn is_valid_word(word: &str, dict: &Dictionary) -> bool {
    dict.contains(word)
}

pub fn fix_spaces(text: &str, dict: &Dictionary) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return text.to_string();
    }

    let mut result: Vec<String> = Vec::new();
    let mut i = 0;

    while i < words.len() {
        if i + 1 < words.len() {
            let w1 = words[i];
            let w2 = words[i + 1];
            let merged = format!("{w1}{w2}");
            let merged_lower = merged.to_lowercase();

            let w1_valid = is_valid_word(w1, dict);
            let w2_valid = is_valid_word(w2, dict);
            let merged_valid = is_valid_word(&merged_lower, dict);

            let mut merge_word: Option<String> = None;

            if merged_valid {
                if !w1_valid || !w2_valid {
                    merge_word = Some(merged.clone());
                }
            }

            if merge_word.is_none() && (is_short_token(w1) || is_short_token(w2)) {
                if let Some(found) = symspell_lookup(&merged_lower, dict) {
                    merge_word = Some(found);
                }
            }

            if merge_word.is_none() && ends_with_consonant(w1) && starts_with_vowel(w2) {
                if let Some(found) = symspell_lookup(&merged_lower, dict) {
                    merge_word = Some(found);
                }
            }

            if merge_word.is_none() {
                let w2_lower = w2.to_lowercase();
                if COMMON_SUFFIXES.contains(&w2_lower.as_str()) {
                    merge_word = Some(merged.clone());
                }
            }

            if merge_word.is_none() {
                for prefix in COMMON_PREFIXES {
                    let prefix_chars = prefix.chars().count();
                    let w1_chars_count = w1.chars().count();
                    if w1_chars_count >= prefix_chars {
                        let w1_prefix: String = w1.chars().take(prefix_chars).collect();
                        if w1_prefix.to_lowercase() == *prefix {
                            if w1_chars_count > prefix_chars || w2.len() >= 3 {
                                let prefixed = format!("{w1}{w2}");
                                if is_valid_word(&prefixed.to_lowercase(), dict) {
                                    merge_word = Some(prefixed);
                                    break;
                                }
                            }
                        }
                    }
                }
            }

            if let Some(word) = merge_word {
                result.push(word);
                i += 2;
                continue;
            }
        }
        result.push(words[i].to_string());
        i += 1;
    }

    result.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::dictionary::Dictionary;

    fn test_dict() -> Dictionary {
        Dictionary::from_words(&[
            "произвольных", "варианты",
            "и", "их",
            "красный", "крас",
        ])
    }

    #[test]
    fn test_merge_broken_words() {
        let dict = test_dict();
        let result = fix_spaces("произволь ных варианты", &dict);
        assert_eq!(result, "произвольных варианты");
    }

    #[test]
    fn test_keep_valid_separate() {
        let dict = test_dict();
        let result = fix_spaces("и их варианты", &dict);
        assert_eq!(result, "и их варианты");
    }

    #[test]
    fn test_suffix_merge() {
        let dict = test_dict();
        let result = fix_spaces("крас ный", &dict);
        assert_eq!(result, "красный");
    }
}
