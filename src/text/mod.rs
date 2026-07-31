pub mod aliases;
pub mod dictionary;
pub mod hallucinations;
mod punctuation;
mod repetitions;
mod space_fixer;
pub mod user_dict;

use crate::config::TextFixConfig;
pub use hallucinations::load_custom_phrases;
pub use user_dict::UserDict;

pub fn fix_text(text: &str, config: &TextFixConfig, user_dict: &UserDict) -> String {
    let mut text = text.to_string();

    if config.fix_hallucinations {
        text = hallucinations::remove_hallucinations(&text);
    }

    if config.fix_user_dict {
        text = user_dict.apply(&text);
    }

    if config.fix_repetitions {
        text = repetitions::fix_repetitions(&text);
    }

    if config.fix_punctuation {
        text = punctuation::fix_punctuation(&text);
    }

    let mut text = text.trim().to_string();

    if config.trailing_space && !text.is_empty() {
        text.push(' ');
    }

    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TextFixConfig;

    fn cfg() -> TextFixConfig {
        TextFixConfig {
            fix_hallucinations: true,
            fix_user_dict: true,
            fix_repetitions: true,
            fix_punctuation: true,
            trailing_space: false,
        }
    }

    fn dict() -> UserDict {
        let d = UserDict::new();
        d.add_entry("фыва", "привет");
        d
    }

    #[test]
    fn test_fix_all_on() {
        // Галлюцинации вырезаются, пунктуация ставится, текст тримится
        let text = fix_text("  привет [BLANK_AUDIO] мир  ", &cfg(), &UserDict::new());
        assert_eq!(text, "Привет мир.");
    }

    #[test]
    fn test_empty_input_stays_empty() {
        let text = fix_text("", &cfg(), &UserDict::new());
        assert_eq!(text, "");
        assert_eq!(fix_text("   ", &cfg(), &UserDict::new()), "");
    }

    #[test]
    fn test_user_dict_applied() {
        let text = fix_text("фыва мир", &cfg(), &dict());
        assert_eq!(text, "Привет мир.");
    }

    #[test]
    fn test_user_dict_disabled() {
        let mut c = cfg();
        c.fix_user_dict = false;
        let text = fix_text("фыва мир", &c, &dict());
        assert_eq!(text, "Фыва мир.");
    }

    #[test]
    fn test_trailing_space() {
        let mut c = cfg();
        c.trailing_space = true;
        assert_eq!(fix_text("привет", &c, &UserDict::new()), "Привет. ");
        assert_eq!(fix_text("", &c, &UserDict::new()), "");
    }

    #[test]
    fn test_all_disabled_only_trims() {
        let mut c = cfg();
        c.fix_hallucinations = false;
        c.fix_user_dict = false;
        c.fix_repetitions = false;
        c.fix_punctuation = false;
        let text = fix_text("  да-да-да  ", &c, &dict());
        assert_eq!(text, "да-да-да");
    }

    #[test]
    fn test_repetitions_fixed_by_default() {
        let text = fix_text("да-да-да", &cfg(), &UserDict::new());
        assert_eq!(text, "Да.");
    }
}
