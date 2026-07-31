// Модель меню трея — чистые данные, без Win32.
// Состояние иконки трея → список пунктов меню в правильном порядке.
// Юнит-тесты: порядок, id, галочки, ключи локализации.

pub const CMD_SETTINGS: u32 = 1000;
pub const CMD_AUTOSTOP: u32 = 1001;
pub const CMD_MATH: u32 = 1002;
pub const CMD_QUIT: u32 = 1003;
pub const CMD_CONSOLE: u32 = 1004;
pub const CMD_ADD_WORD: u32 = 1005;
pub const CMD_EDIT_DICT: u32 = 1006;
pub const CMD_WAKE: u32 = 1007;
pub const CMD_ADD_HALL: u32 = 1008;
pub const CMD_EDIT_HALL: u32 = 1009;
pub const CMD_TRANSCRIBE_FILE: u32 = 1010;
pub const CMD_SUBTITLE_FILE: u32 = 1011;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MenuKind {
    /// Версия — серый пункт сверху
    Version,
    Separator,
    Item,
    /// Пункт с галочкой (вкл/выкл)
    Checked(bool),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrayMenuItem {
    pub kind: MenuKind,
    pub id: u32,
    pub label_key: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct TrayMenuState {
    pub vad_on: bool,
    pub wake_on: bool,
    pub console_visible: bool,
}

pub fn menu_items(state: &TrayMenuState) -> Vec<TrayMenuItem> {
    vec![
        TrayMenuItem { kind: MenuKind::Version, id: 0, label_key: "tray.menu.version" },
        TrayMenuItem { kind: MenuKind::Separator, id: 0, label_key: "" },
        TrayMenuItem { kind: MenuKind::Item, id: CMD_SETTINGS, label_key: "tray.menu.settings" },
        TrayMenuItem { kind: MenuKind::Separator, id: 0, label_key: "" },
        TrayMenuItem {
            kind: MenuKind::Item,
            id: CMD_CONSOLE,
            label_key: if state.console_visible { "tray.menu.toggle_console.hide" } else { "tray.menu.toggle_console.show" },
        },
        TrayMenuItem { kind: MenuKind::Separator, id: 0, label_key: "" },
        TrayMenuItem { kind: MenuKind::Item, id: CMD_ADD_WORD, label_key: "tray.menu.add_word" },
        TrayMenuItem { kind: MenuKind::Item, id: CMD_EDIT_DICT, label_key: "tray.menu.edit_dict" },
        TrayMenuItem { kind: MenuKind::Separator, id: 0, label_key: "" },
        TrayMenuItem { kind: MenuKind::Item, id: CMD_ADD_HALL, label_key: "tray.menu.add_hall" },
        TrayMenuItem { kind: MenuKind::Item, id: CMD_EDIT_HALL, label_key: "tray.menu.edit_hall" },
        TrayMenuItem { kind: MenuKind::Separator, id: 0, label_key: "" },
        TrayMenuItem { kind: MenuKind::Item, id: CMD_TRANSCRIBE_FILE, label_key: "tray.menu.transcribe_file" },
        TrayMenuItem { kind: MenuKind::Item, id: CMD_SUBTITLE_FILE, label_key: "tray.menu.subtitle_file" },
        TrayMenuItem { kind: MenuKind::Separator, id: 0, label_key: "" },
        TrayMenuItem { kind: MenuKind::Checked(state.wake_on), id: CMD_WAKE, label_key: "tray.menu.voice_activation" },
        TrayMenuItem { kind: MenuKind::Checked(state.vad_on), id: CMD_AUTOSTOP, label_key: "tray.menu.auto_stop" },
        TrayMenuItem { kind: MenuKind::Item, id: CMD_MATH, label_key: "tray.menu.math_mode" },
        TrayMenuItem { kind: MenuKind::Separator, id: 0, label_key: "" },
        TrayMenuItem { kind: MenuKind::Item, id: CMD_QUIT, label_key: "tray.menu.quit" },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> TrayMenuState {
        TrayMenuState { vad_on: false, wake_on: false, console_visible: false }
    }

    #[test]
    fn test_menu_20_items_exact_order() {
        let items = menu_items(&state());
        assert_eq!(items.len(), 20);

        // Порядок пунктов и id — эталон, сверка с треем
        let ids: Vec<u32> = items.iter().map(|i| i.id).collect();
        assert_eq!(ids, vec![
            0,            // версия
            0,            // разделитель
            CMD_SETTINGS, // настройки
            0,            // разделитель
            CMD_CONSOLE,  // показать/скрыть окно
            0,            // разделитель
            CMD_ADD_WORD,
            CMD_EDIT_DICT,
            0,            // разделитель
            CMD_ADD_HALL,
            CMD_EDIT_HALL,
            0,            // разделитель
            CMD_TRANSCRIBE_FILE,
            CMD_SUBTITLE_FILE,
            0,            // разделитель
            CMD_WAKE,
            CMD_AUTOSTOP,
            CMD_MATH,
            0,            // разделитель
            CMD_QUIT,
        ]);
    }

    #[test]
    fn test_first_is_version_grayed() {
        let items = menu_items(&state());
        assert_eq!(items[0].kind, MenuKind::Version);
        assert_eq!(items[0].label_key, "tray.menu.version");
    }

    #[test]
    fn test_console_label_depends_on_visibility() {
        let shown = menu_items(&TrayMenuState { console_visible: true, ..state() });
        let hidden = menu_items(&TrayMenuState { console_visible: false, ..state() });
        assert_eq!(shown[4].label_key, "tray.menu.toggle_console.hide");
        assert_eq!(hidden[4].label_key, "tray.menu.toggle_console.show");
    }

    #[test]
    fn test_vad_checked_when_enabled() {
        let on = menu_items(&TrayMenuState { vad_on: true, ..state() });
        let off = menu_items(&state());
        assert_eq!(on[16].kind, MenuKind::Checked(true));
        assert_eq!(off[16].kind, MenuKind::Checked(false));
        assert_eq!(on[16].id, CMD_AUTOSTOP);
    }

    #[test]
    fn test_wake_checked_when_enabled() {
        let on = menu_items(&TrayMenuState { wake_on: true, ..state() });
        let off = menu_items(&state());
        assert_eq!(on[15].kind, MenuKind::Checked(true));
        assert_eq!(off[15].kind, MenuKind::Checked(false));
        assert_eq!(on[15].id, CMD_WAKE);
    }

    #[test]
    fn test_quit_is_last() {
        let items = menu_items(&state());
        assert_eq!(items[19].kind, MenuKind::Item);
        assert_eq!(items[19].id, CMD_QUIT);
        assert_eq!(items[19].label_key, "tray.menu.quit");
    }

    #[test]
    fn test_labels_localization_keys_present_in_ru() {
        // Все ключи должны существовать в lang/ru.json (проверка локализации)
        let ru: serde_json::Value = serde_json::from_str(include_str!("../../lang/ru.json")).unwrap();
        for item in menu_items(&state()) {
            if item.label_key.is_empty() { continue; }
            assert!(ru.get(item.label_key).is_some(), "нет ключа {item:?} в ru.json");
        }
    }
}
