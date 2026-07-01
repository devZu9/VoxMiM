use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum CommandAction {
    Paste(String),
    Hotkey(String),
    MouseMove(String),
    MouseClick(String),
    MouseScroll(String),
    MouseScrollMax(String),
    MouseMonitor(String),
    MouseContinuous(String),
    MouseStop,
    FocusSwitch,
    FocusSave,
    Grid(String),
    GridZoom(String),
    SelectionMore,
    SelectionLess,
    ToggleMathMode(bool),
    None,
}

#[derive(Debug, Deserialize)]
struct RawCommand {
    triggers: Option<RawTriggers>,
    #[serde(alias = "type")]
    cmd_type: String,
    value: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawTriggers {
    common: Option<Vec<String>>,
    ru: Option<Vec<String>>,
    en: Option<Vec<String>>,
}

pub struct CommandExecutor {
    commands: Vec<(String, CommandAction)>,
    aliases: HashMap<String, String>,
}

impl CommandExecutor {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            aliases: HashMap::new(),
        }
    }

    pub fn load_commands<P: AsRef<std::path::Path>>(&mut self, path: P) {
        let path = path.as_ref();
        if !path.exists() {
            log::info!("Файл команд не найден: {}", path.display());
            return;
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("Не удалось прочитать {e}");
                return;
            }
        };

        let raw: HashMap<String, serde_json::Value> = match serde_json::from_str(&content) {
            Ok(m) => m,
            Err(e) => {
                log::warn!("Ошибка парсинга команд: {e}");
                return;
            }
        };

        self.commands.clear();
        for (name, val) in &raw {
            if name.starts_with('_') {
                continue;
            }
            let cmd: RawCommand = match serde_json::from_value(val.clone()) {
                Ok(c) => c,
                Err(e) => {
                    log::warn!("Пропуск команды '{name}': {e}");
                    continue;
                }
            };
            let triggers = extract_triggers(&cmd.triggers);
            for trigger in triggers {
                let action = parse_action(name, &cmd.cmd_type, cmd.value.as_deref());
                self.commands.push((trigger.to_lowercase(), action));
            }
        }

        log::info!("Загружено {} команд", self.commands.len());
    }

    pub fn load_aliases<P: AsRef<std::path::Path>>(&mut self, path: P) {
        let path = path.as_ref();
        if !path.exists() {
            return;
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return,
        };

        let raw: HashMap<String, HashMap<String, String>> =
            match serde_json::from_str(&content) {
                Ok(m) => m,
                Err(_) => return,
            };

        self.aliases.clear();
        for (_lang, aliases) in &raw {
            for (mistake, correct) in aliases {
                self.aliases.insert(mistake.to_lowercase(), correct.to_lowercase());
            }
        }
    }

    pub fn resolve_alias(&self, text: &str) -> String {
        let lower = text.to_lowercase();
        self.aliases
            .get(&lower)
            .cloned()
            .unwrap_or(lower)
    }

    pub fn try_execute(&self, text: &str, max_words: u32) -> Option<&CommandAction> {
        let lower = self.resolve_alias(text);
        let word_count = lower.split_whitespace().count() as u32;

        // Если слов >= порога — это диктовка, а не команда
        if word_count >= max_words.max(3) {
            return None;
        }

        for (trigger, action) in &self.commands {
            let t = trigger.to_lowercase();
            // Точное совпадение
            if lower == t {
                return Some(action);
            }
            // Начинается с триггера
            if lower.starts_with(&format!("{} ", t)) {
                return Some(action);
            }
            // Заканчивается триггером
            if lower.ends_with(&format!(" {}", t)) {
                return Some(action);
            }
        }
        None
    }
}

fn extract_triggers(triggers: &Option<RawTriggers>) -> Vec<String> {
    let mut all = Vec::new();
    match triggers {
        Some(t) => {
            if let Some(common) = &t.common {
                all.extend(common.iter().cloned());
            }
            if let Some(ru) = &t.ru {
                all.extend(ru.iter().cloned());
            }
            if let Some(en) = &t.en {
                all.extend(en.iter().cloned());
            }
        }
        None => {}
    }
    all
}

fn parse_action(name: &str, cmd_type: &str, value: Option<&str>) -> CommandAction {
    match cmd_type {
        "paste" => CommandAction::Paste(value.unwrap_or("").to_string()),
        "hotkey" => CommandAction::Hotkey(value.unwrap_or("").to_string()),
        "mouse_move" => CommandAction::MouseMove(value.unwrap_or("right").to_string()),
        "mouse_click" => CommandAction::MouseClick(value.unwrap_or("left").to_string()),
        "mouse_scroll" => CommandAction::MouseScroll(value.unwrap_or("down").to_string()),
        "mouse_scroll_max" => CommandAction::MouseScrollMax(value.unwrap_or("down").to_string()),
        "mouse_monitor" => CommandAction::MouseMonitor(value.unwrap_or("1").to_string()),
        "mouse_continuous" => CommandAction::MouseContinuous(value.unwrap_or("right").to_string()),
        "mouse_stop" => CommandAction::MouseStop,
        "focus_switch" => CommandAction::FocusSwitch,
        "focus_save" => CommandAction::FocusSave,
        "grid" => CommandAction::Grid(value.unwrap_or("").to_string()),
        "grid_zoom" => CommandAction::GridZoom(value.unwrap_or("").to_string()),
        "selection_more" => CommandAction::SelectionMore,
        "selection_less" => CommandAction::SelectionLess,
        "toggle_math_mode" => CommandAction::ToggleMathMode(value == Some("on")),
        _ => {
            log::warn!("Неизвестный тип команды {cmd_type} для {name}");
            CommandAction::None
        }
    }
}
