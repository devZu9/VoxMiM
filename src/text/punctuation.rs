use regex::Regex;

pub fn fix_punctuation(text: &str) -> String {
    let text = text.trim();

    if text.is_empty() {
        return String::new();
    }

    let mut result = text.to_string();

    let first = result.chars().next();
    if first.map_or(false, |c| c.is_lowercase()) {
        let c = first.unwrap().to_uppercase().to_string();
        result = c + &result[first.unwrap().len_utf8()..];
    }

    let last_char = result.chars().last();
    if !matches!(last_char, Some('.') | Some('!') | Some('?') | Some(',') | Some(';') | Some(':') | Some(')') | Some(']') | Some('}')) {
        result.push('.');
    }

    let re = Regex::new(r"\s+([.,!?;:])").unwrap();
    result = re.replace_all(&result, "$1").to_string();

    let re = Regex::new(r",\s*,+").unwrap();
    result = re.replace_all(&result, ",").to_string();

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_period() {
        let result = fix_punctuation("привет мир");
        assert_eq!(result, "Привет мир.");
    }

    #[test]
    fn test_keep_existing_period() {
        let result = fix_punctuation("Привет мир.");
        assert_eq!(result, "Привет мир.");
    }

    #[test]
    fn test_capitalize() {
        let result = fix_punctuation("привет.");
        assert_eq!(result, "Привет.");
    }

    #[test]
    fn test_space_before_comma() {
        let result = fix_punctuation("Привет , мир");
        assert_eq!(result, "Привет, мир.");
    }
}
