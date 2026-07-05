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
