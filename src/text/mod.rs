pub mod aliases;
pub mod dictionary;
mod hallucinations;
mod punctuation;
mod repetitions;
mod space_fixer;

use crate::config::TextFixConfig;
pub use dictionary::Dictionary;

pub fn fix_text(text: &str, config: &TextFixConfig, dict: &Dictionary) -> String {
    let mut text = text.to_string();

    if config.fix_hallucinations {
        text = hallucinations::remove_hallucinations(&text);
    }

    text = text.split_whitespace().collect::<Vec<_>>().join(" ");

    if config.fix_spaces {
        text = space_fixer::fix_spaces(&text, dict);
    }

    if config.fix_dictionary {
        text = dictionary::apply_dict(&text);
        // apply_user_dict — пока заглушка
    }

    if config.fix_repetitions {
        text = repetitions::fix_repetitions(&text);
    }

    if config.fix_punctuation {
        text = punctuation::fix_punctuation(&text);
    }

    text.trim().to_string()
}
