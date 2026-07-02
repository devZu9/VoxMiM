# AGENTS.md — VoxMiM

> Голосовой ввод текста на Rust. По мотивам VoxBee с исправлением разорванных слов.

## Основные правила

- **Язык проекта — русский.** Все комментарии, логи, UI, переменные (где уместно) — на русском.
  - Исключение: названия крейтов, технические идентификаторы, общепринятые термины (STT, VAD, GPU).
- **KISS и DRY** — главные принципы. Не усложнять без необходимости.
- Если можно решить без кода — объяснить, не писать код.
- Файлы — не больше 250 строк. При превышении — выносить в хелперы или модуль.
- UTF-8 без BOM. Даты/время — локальные (`chrono::Local`).
- Версия — только в `Cargo.toml`, в коде через `env!("CARGO_PKG_VERSION")`.

## Стек проекта

- **STT:** `whisper-rs` (FFI к whisper.cpp, без внешнего сервера)
- **Аудио:** `cpal` + `webrtc-audio-processing` (шумоподавление + VAD)
- **Текст:** `phf`-словарь русских слов + `regex` для постобработки
- **Ввод:** Win32 Clipboard API (`windows-sys`) + `enigo` + `rdev`
- **UI:** `tray-icon` (трей) + `egui`/`eframe` (окно настроек)
- **Конфиг:** `serde` + `serde_json`
- **Пути:** `dirs` крейт, никаких `C:\...` хардкодов

## Ключевая фича (не забывать)

Главное исправление относительно VoxBee — **склейка разорванных слов** в русском тексте.
"произволь ных" → "произвольных". Реализовано в `text/space_fixer.rs` через:
1. `phf`-словарь ~150K русских слов
2. Грамматические эвристики
3. SymSpell fallback

## Архитектура

```
audio/     — cpal capture + ring_buffer + noise_filter
stt/       — whisper-rs model load + transcribe
vad/       — webrtc-audio-processing VAD
text/      — fix_text orchestrator + space/dict/hallucinations/repetitions/punctuation
input/     — Win32 clipboard + Ctrl/V + rdev hotkeys + enigo sim
commands/  — JSON-команды + math mode
ui/        — tray-icon меню + egui settings window
```

## Потоки

- **Main** — event loop, state machine (crossbeam-channel)
- **Audio** — cpal callback
- **Worker** — whisper inference (spawn per request)
- **Hotkey** — rdev listen
- **Tray** — tray-icon event loop
- **Settings** — egui окно (по требованию)

## Принципы работы с данными

- Настройки — единая точка: `config.rs` → `serde_json` → `config.json` в `dirs::config_dir()`
- Путь до данных — через `dirs` крейт, не хардкодить
- Каждая переменная хранит собственное значение, не ссылку на другую
- Списки >3 элементов — выносить в отдельный JSON
