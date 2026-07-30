# VoxMiM v0.10.0 — Итоговая сводка

## Что сделано

**VoxMiM** — голосовой ввод текста на Rust. Windows + macOS.

> **v0.10.0:** Полный порт на macOS (M1 Pro). NSStatusBar-трей, CGEventTap (Cmd+Esc), pbcopy+Cmd+V, Unix socket IPC, platform splits. Исправлены dylib-зависимости whisper-server. Обе платформы собираются (cargo build —release 0 ошибок).

### Рабочий процесс

**Windows:** `Ctrl+Insert` → запись → отпустить → whisper → текст → вставка
**macOS:** `Cmd+Esc` → запись → отпустить → whisper → текст → вставка

### Ключевые компоненты — macOS

| Компонент | Статус | Технология |
|---|---|---|
| Трей (меню-бар) | ✅ | NSStatusBar + NSMenu (objc2-app-kit 0.3) |
| Глобальный хоткей | ✅ | CGEventTap (core-graphics) + CFRunLoop |
| Вставка текста | ✅ | pbcopy/pbpaste + CGEventPost (Cmd+V) |
| Unix domain socket IPC | ✅ | вместо Named Pipe |
| AudioCapture Send | ✅ | unsafe impl Send (CoreAudio thread-safe) |
| NSApp lifecycle | ✅ | finishLaunching + autoreleasepool |
| Настройки (Fenestra) | ⚪ | пока не портировано |
| VAD | ⚪ | Hold-режим работает, VAD не тестировался |

### Ключевые компоненты — Windows

| Компонент | Статус | Технология |
|---|---|---|
| Аудио-захват | ✅ | cpal (WASAPI) |
| VAD (Автостоп) | ✅ | RMS-based |
| Распознавание | ✅ | whisper-server HTTP:8178 / one-shot CLI |
| Глобальный хоткей | ✅ | Win32 WH_KEYBOARD_LL |
| Трей-иконка | ✅ | Win32 NOTIFYICONDATAW |
| Окно настроек (Fenestra) | ✅ | отдельный .exe, Named Pipe IPC |
| Named Pipe IPC | ✅ | \\.\pipe\VoxMiMSettings |
| Пользовательский словарь | ✅ | dicts/user_dict.json |
| Локализация (i18n) | ✅ | lang/ru.json + en.json |
| Команды голосом | ✅ | 199 команд |

### Архитектура

```
Потоки (macOS): main (NSApp) | app (event loop) | hotkey (CGEventTap) | whisper | pipe (Unix socket) | inserter
Потоки (Windows): main | audio-accum | whisper | hotkey | tray | wake-detect
Каналы: crossbeam-channel (cmd_tx/rx, whisper_tx/rx), mpsc (аудио), Named Pipe / Unix socket (settings IPC)
```

### Файлы (macOS)

```
src/
├── main.rs                   # NSApp на главном потоке (macOS), app в фоне
├── app.rs                    # Event loop + state machine (общий)
├── config.rs                 # Config + общие настройки
├── download_macos.rs         # Сборка whisper из исходников + fix_rpath dylib
├── download_windows.rs       # Авто-скачивание whisper-cli
├── ui/tray.rs                # Диспетчер (cfg → tray_windows / tray_macos)
├── ui/tray_macos.rs          # NSStatusBar + NSMenu + иконки
├── pipe_macos.rs             # Unix domain socket IPC
├── ui/dialog_macos.rs        # Заглушка (команды через AppCommand)
├── input/hotkeys.rs          # CGEventTap (Cmd+Esc) на macOS, WH_KEYBOARD_LL на Windows
├── input/inserter.rs         # pbcopy + CGEventPost на macOS, Win32 Clipboard на Windows

scripts/
├── run.sh                    # cargo build --release && ./target/release/voxmim
├── make-app.sh               # сборка + создание VoxMiM.app
├── fix-permissions.sh        # инструкция по правам macOS
```

### Версии

- **voxmim**: 0.10.0 (macOS порт: трей, hotkey, вставка, platform splits, dylib-fix)
- **voxmim-settings**: 1.3.0 (не портировано на macOS)

### Сборка

```bash
cargo build --release                          # macOS (arm64)
cargo check --release --target x86_64-pc-windows-msvc  # Windows (cross)
bash scripts/make-app.sh                       # macOS .app bundle
```

## Backlog

- [ ] Трей-меню macOS — все пункты (настройки, VAD, словари, автозагрузка)
- [ ] Уменьшение иконки (24→18px) + отступы
- [ ] Settings IPC на macOS — полноценный диалог
- [ ] VAD на macOS
- [ ] Автозагрузка macOS (launchd plist)
- [ ] Windows CI сборка
