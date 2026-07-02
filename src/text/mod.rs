pub mod aliases;
pub mod dictionary;
mod hallucinations;
mod punctuation;
mod repetitions;
mod space_fixer;
pub mod user_dict;

use crate::config::TextFixConfig;
pub use dictionary::Dictionary;
pub use hallucinations::load_custom_phrases;
pub use user_dict::UserDict;

pub fn fix_text(text: &str, config: &TextFixConfig, dict: &Dictionary, user_dict: &UserDict) -> String {
    let mut text = text.to_string();

    if config.fix_hallucinations {
        text = hallucinations::remove_hallucinations(&text);
    }

    text = text.split_whitespace().collect::<Vec<_>>().join(" ");

    if config.fix_spaces {
        text = space_fixer::fix_spaces(&text, dict);
    }

    if config.fix_dictionary {
        log::debug!("fix_text: fix_dictionary=true, текст до: «{text}»");
        text = dictionary::apply_dict(&text);
        text = user_dict.apply(&text);
        log::debug!("fix_text: после словарей: «{text}»");
    } else {
        log::debug!("fix_text: fix_dictionary=false — словари отключены");
    }

    if config.fix_repetitions {
        text = repetitions::fix_repetitions(&text);
    }

    if config.fix_punctuation {
        text = punctuation::fix_punctuation(&text);
    }

    text.trim().to_string()
}
