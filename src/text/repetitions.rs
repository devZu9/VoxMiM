pub fn fix_repetitions(text: &str) -> String {
    let mut result = text.to_string();

    loop {
        let words: Vec<&str> = result.split_whitespace().collect();
        if words.is_empty() {
            break;
        }

        let mut new_words: Vec<String> = Vec::new();
        let mut changed = false;

        for w in &words {
            let parts: Vec<&str> = w.split('-').collect();
            if parts.len() >= 2 && parts.iter().all(|p| *p == parts[0]) {
                new_words.push(parts[0].to_string());
                changed = true;
            } else {
                new_words.push(w.to_string());
            }
        }

        result = new_words.join(" ");
        if !changed {
            break;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collapse_repetition() {
        let result = fix_repetitions("да-да-да");
        assert_eq!(result, "да");
    }

    #[test]
    fn test_no_false_positive() {
        let result = fix_repetitions("нормальный текст");
        assert_eq!(result, "нормальный текст");
    }

    #[test]
    fn test_double() {
        let result = fix_repetitions("ну-ну");
        assert_eq!(result, "ну");
    }
}
