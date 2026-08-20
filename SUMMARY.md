# SUMMARY — VoxMiM

> Памятка о текущем устройстве: архитектура, ключевые решения, состояние. НЕ история и НЕ процесс (их — в SESSIONS/CHANGELOG).

## Что это

**VoxMiM** — голосовой ввод текста на Rust (Windows + macOS). Push-to-talk → whisper (GPU) → текст → вставка в активное поле. По мотивам VoxBee, с исправлением разорванных слов.

**Текущая версия:** `voxmim` 0.12.1 · `voxmim-settings` 1.5.1 (Cargo.toml — единственный источник версии).

## Ключевые решения

- **Склейка разорванных слов решена на уровне whisper-server** — `text/space_fixer.rs` отключён, `\n` удаляются сервером. Не пересобирать склейку в приложении.
- **Окно настроек — отдельный процесс** `voxmim-settings` на fenestra (Elm-подобный: `update(msg)` + `view()`), связь по Named Pipe (Windows) / Unix socket (macOS), настройки — через `config.json` + mtime-watcher (`reload_config()`).
- **Трей — основная точка управления** (Win32 NOTIFYICONDATAW / NSStatusBar), состояния IDLE/RECORDING/LOADING.
- **Пользовательский словарь:** `dicts/user_dict.json` в корне; после добавления через трей `__sync_dict.bat` сохраняет в корень; `build.rs` копирует при сборке.
- **Настройки — единая точка:** `config.rs` → `serde_json` → `config.json` в `dirs::config_dir()`. Пути — через `dirs`, без хардкодов.

## Архитектура

```
audio/     — cpal capture + ring_buffer + noise_filter
stt/       — whisper-server/cli (HTTP localhost:8178) + transcribe
vad/       — RMS / webrtc-audio-processing VAD
text/      — fix_text orchestrator + dict/hallucinations/repetitions/punctuation
input/     — Win32 clipboard + Ctrl/V + rdev hotkeys + enigo sim
commands/  — JSON-команды + math mode
ui/        — tray (Win32/macOS) + tray_menu (модель меню) + настройки fenestra (отдельный процесс)
```

**Потоки:** Main (event loop, state machine) · Audio (cpal callback) · Worker (whisper, per request) · Hotkey (rdev/Win32) · Tray (Win32 message loop / NSStatusItem) · Settings (отдельный процесс).

**Каналы:** crossbeam-channel (`cmd_tx/rx`, `result_tx/rx`), mpsc (аудио), Named Pipe / Unix socket (settings IPC).

## Ключевые компоненты

| Компонент | Windows | macOS |
|---|---|---|
| Аудио-захват | cpal (WASAPI) | cpal (CoreAudio, AudioCapture Send) |
| Глобальный хоткей | WH_KEYBOARD_LL | CGEventTap (Cmd+Esc) |
| Трей | NOTIFYICONDATAW | NSStatusBar + NSMenu |
| Вставка текста | Win32 Clipboard | pbcopy + CGEventPost (Cmd+V) |
| Распознавание | whisper-server HTTP / one-shot CLI | то же |
| Окно настроек | fenestra, Named Pipe | ⚪ не портировано |
| VAD (Автостоп) | ✅ | ⚪ не тестирован |
| Локализация | lang/ru.json + en.json | |

## Состояние

- Юнит-тесты: 57+ `#[test]` (`cargo test --workspace` на Windows; на macOS — `cargo test -p voxmim --bin voxmim`, т.к. `voxmim-settings` не собирается без cfg-гейтов — задание 11 аудита).
- Покрытие: llvm-cov ~35% строк (точка отсчёта 31.07.2026).
- Внедрение канона `_for_OpenCode`: актуально (apply-audit 2.7.2, 21.08.2026) — скиллы-линки, команды codecheck/testing, аудит `_for_OpenCode/audits/VoxMiM.md`.
- Последняя сессия: 21.08.2026 — актуализация внедрения канона.

## Сборка и запуск

```bash
cargo build --release                  # Windows / macOS
cargo test --workspace                 # Windows
python scripts/tray_smoke/test_tray.py --exe-dir <папка приложения>   # smoke трея (вручную)
bash scripts/make-app.sh               # macOS .app bundle
```

Запуск: exe рядом с `config.json`; скрипты `__run.bat` / `__run_debug.bat` (Windows), `scripts/run.sh` (macOS).

## Документация

- `AGENTS.md` — правила работы агента; `ROADMAP.md` + `ROADMAP-IDEAS.md` — план; `SESSIONS.md` — журнал; `CHANGELOG.md` — сухие итоги по версиям; `TECH_SPECIFICATION.md` — ТЗ.
- Канон аудита: `C:\_dev\_for_OpenCode\audits\VoxMiM.md`.
