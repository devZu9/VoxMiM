# Changelog

All notable changes to VoxMiM will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.7.2] — 2026-07-03

### Added
- **Отдельное окно настроек (Fenestra)** — `voxmim-settings.exe` с собственным UI на Fenestra
- **Named Pipe IPC** — связь между `voxmim.exe` и `voxmim-settings.exe` через Win32 Named Pipe
- **Named Mutex** — single instance для окна настроек (``Local\VoxMiMSettingsInstance``)
- **Локализация окна настроек** — `lang/ru.json` + `lang/en.json` через `include_str!`
- **Тёмная тема** — переключение в окне настроек, применяется через `Theme::dark()`
- **Always-on-top** — `WH_CBT` hook с `WS_EX_TOPMOST` + `SetWindowPos(HWND_TOPMOST)` для окна настроек
- **Кастомная кнопка закрытия** — через Fenestra `Msg::Close` → `std::process::exit(0)`
- **`install_topmost_hook()`** — CBT-хук на `HCBT_CREATEWND` + `HCBT_ACTIVATE` для гарантированного «поверх всех»
- **`__run.bat` / `__run_debug.bat`** — `cargo build --workspace`, вывод в консоль через Tee-Object

### Changed
- **Отступы в окне настроек** — `SP1` (4px) → `SP2` (8px) для gap между элементами
- **Файлы сборки** — `build.rs` копирует `settings/` и `lang/` в `target/debug/`
- **Пользовательский словарь** — `save()` сортирует ключи: ASCII → Кириллица
- **Настройки** — `reload_config()` логирует только изменённые поля (22 поля)

### Fixed
- **Крэш настроек** — убран субклассинг окна с `WM_NCCALCSIZE`, возвращён `remove_window_caption()` с `EnumWindows` + задержка
- **Окно не поверх всех** — исправлено через CBT-хук (двойная фиксация: стиль + z-order)

### Removed
- **Slint UI** — старый `ui/settings.rs` + `ui/settings.slint` (заменён на Fenestra)
- **`image` крейт** из зависимостей `settings` — иконка вшивается через `embed-resource`
- **Мёртвый код** из `text/dictionary.rs`: `Dictionary::path()`, `Dictionary::new()`, `Dictionary::load_lang()`

## [0.7.1] — 2026-07-02

### Fixed
- **Окно настроек** — `winit` паниковал вне главного потока. Добавлен `any_thread(true)` через `event_loop_builder`
- **Локализация** — `lang::load_locale()` вызывалась после старта трея, меню показывало ключи вместо строк. Вызов перенесён до спавна трея
- **Повторное открытие** — окно больше не крашится при повторном вызове (hide/reopen). `Visible` → `Minimized` для корректной работы event loop
- **Локали без файлов** — встроены в `.exe` через `include_str!`
- **Размер бинарника** — `rfd` заменён на Win32 `SHBrowseForFolderW`, лишние Linux-зависимости удалены

### Added
- **Два независимых WhisperEngine** — транскрайбер и детектор не блокируют друг друга
- **Persistent-режим whisper** — модель загружается один раз и остаётся в памяти (`keep_model_loaded`)
- **Чекбоксы в настройках** — «Не выгружать модель из памяти» и «Не выгружать модель детектора»
- **Предупреждение о большой модели** — лог-сообщение, если `detector_model` > 500 МБ
- **Окно настроек (egui):** иконка `blue-voice.png`, hide/reopen с `Minimized`
- **Файловый логгер** — пишет в `logs/voxmim.log` рядом с `.exe`, переключается в настройках
- **build.rs** — копирует `lang/*.json` в `target/debug/` при сборке

## [0.7.0] — 2026-07-02
  - VAD только автостоп: Insert (tap) → говоришь → тишина → само остановилось
  - Tap-режим: Insert переключает запись, повторный Insert = принудительная остановка
  - Hold-режим (VAD выкл): прежнее поведение (зажал → говоришь → отпустил)
- **Wake Word:** единый аудио-захват через `start_capture_multi()` (fan-out)
  - Убран второй AudioCapture — больше нет WASAPI-конфликтов
  - Wake детекция + транскрибация команды
- **Единый аудио-пайплайн:** `audio/capture.rs` → `start_capture_multi(txs)`
- **Локализация (i18n):** новый модуль `src/lang.rs` + `lang/ru.json` + `lang/en.json`
  - Трей-меню и диалог «Добавить слово» читают строки из локали
  - Переключение через `config.language`
- **Трей-меню с чекбоксами:** галочки для «Автостоп» и «Голосовая активация»
- **`input/hotkeys.rs`:** VAD_ENABLED статик, tap-режим при VAD

### Changed
- `config.vad.enabled` → включает/выключает автостоп
- `AppCommand::ToggleVad` / `ToggleWake` / `ToggleMathMode` — теперь без параметра (toggle)
- `app.rs` — `on_start()` при VAD: повторный Insert = force_stop

### Docs
- `ROADMAP.md` — Фазы 11 (Wake), 11a (Wake+VAD), 13 (i18n) — 🟢
- `TECHNICAL_SPECIFICATION.md` — разделы 13 (VAD+Wake), 14 (i18n)
- `README.md` / `README_EN.md` — VAD, многоязычный интерфейс
- `summary.md` — v0.7.0

## [0.6.0] — 2026-07-02

### Added
- **Пользовательский словарь:** новый модуль `text/user_dict.rs` — загрузка пар «как распознано» → «правильный вариант» из `dicts/user_dict.json`
- **Диалог добавления слов:** Win32-окно «Добавить слово» через трей → иконка, два поля, кнопки «Добавить»/«Отмена»
- **Пункты меню в трее:** «Добавить слово...» и «Редактировать словарь» (открывает JSON в блокноте)
- **Кеш regex'ов**: предварительная компиляция + ручная проверка Unicode-границ (`is_alphabetic`) вместо неподдерживаемого lookaround

## [0.5.2] — 2026-07-02

### Fixed
- **Ложная склейка:** убрана эвристика согласная→гласная — склеивала «нем есть» → «неметь»
- **SymSpell:** удалена генерация однобуквенных вариантов. Теперь только проверка точного совпадения со словарём

## [0.5.1] — 2026-07-02

### Fixed
- **Склейка предлогов:** space_fixer больше не склеивает слова короче 3 символов. Предлоги «в», «на», «у», «с», «об», «и», «к», «о» и прочие короткие слова исключены из склейки

### Docs
- `ROADMAP.md` — добавлен план многоуровневых эвристик склейки (space_fixer_level 0–3)
- `.gitignore` — добавлены `opencode.json`, `AGENTS.md`, `.opencode/` в исключения

## [0.5.0] — 2026-07-01

### Portable-сборка + single-instance + фильтр галлюцинаций

#### Core
- **Portable-сборка:** все пути относительно `.exe`, `%APPDATA%` не используется. Папки: `dicts/` (словари), `bins/` (CUDA DLL + whisper-cli), `models/` (GGML)
- **Single-instance:** исправлен `CreateMutexW` — добавлен `WaitForSingleObject` с проверкой `WAIT_ABANDONED`. Больше не пропускает второй процесс, но корректно восстанавливается после краша
- **Грациозный выход:** `std::process::exit(0)` заменён на `PostMessageW(WM_DESTROY)` → трей сам удаляет иконку, процесс завершается нормально
- **Иконки вшиты в .exe:** `include_bytes!` вместо чтения PNG с диска. Папка `assets/` не нужна
- **Удалена зависимость `directories`** — больше не используем `ProjectDirs`
- **Удалена папка `data/`** (пустая)

#### Text Fixer
- **Кастомные галлюцинации:** `load_custom_phrases()` читает `dicts/hallucinations.txt`, мержит со встроенным списком. Если файла нет — создаётся с примерами. Пользователь может редактировать
- **Фильтр суффиксов:** галлюцинации удаляются только в конце текста, не трогая обычные слова в середине

#### Docs
- `TECHNICAL_SPECIFICATION.md` — добавлен раздел 12 «HTTP API (план)»
- `ROADMAP.md` — обновлён backlog (HTTP API, авто-скачивание моделей)
- `summary.md` — обновлена версия, добавлены новые файлы
- `opencode.json` — добавлен LSP (`rust-analyzer`), убран MCP

## [0.4.0] — 2026-07-01

### Иконка трея + исправление горячей клавиши + анимация загрузки

#### UI
- **Переключение иконок:** IDLE (`blue-voice.png`) ↔ RECORDING (`microphone-stage-light.png`) — через `WM_TIMER` каждые 300мс
- **Анимация загрузки:** `hourglass-fill.png` мигает каждые 600мс, пока не загружены модели Whisper
- **Флаг готовности:** трей стартует сразу (ещё до загрузки моделей); после `ready = true` переключается на `blue-voice.png`
- Исправлен `NIM_MODIFY` — добавлен `NIF_GUID` + `guidItem` для корректного обновления иконки
- `build.rs` — фикс предупреждения `unused_must_use` для `embed_resource::compile`

#### Hotkeys
- **Убран `HOOK_CTRL`** — предыдущий хранил состояние Ctrl и сбивался injected-событиями от `send_ctrl_v()`
- **`GetAsyncKeyState`** — проверка физического состояния Ctrl в момент нажатия Insert (а не хранимого флага)
- **`LLKHF_INJECTED`** — инжектированные SendInput-события игнорируются хук-процедурой
- Конфигурируемые хоткеи в будущем — `GetAsyncKeyState` не привязан к Ctrl+Insert

#### Docs
- `README.md` — добавлен раздел «Словари», таблица «Полезные ссылки» (danakt, whisper.cpp, huggingface)
- `TECHNICAL_SPECIFICATION.md` — обновлён раздел 5.3 (словарь), добавлен раздел 12 «Полезные ссылки»

## [0.3.0] — 2026-07-01

### Исправление консоли + автозагрузка бинарников

#### Core
- Новый модуль `download.rs` — авто-скачивание whisper-cli с GitHub при первом запуске
- `build.rs` — удалено копирование CUDA DLL (теперь загружаются по требованию)
- `config.rs` — добавлено поле `whisper_bins_path` (кеширование пути к бинарникам)
- Поддержка 4 вариантов: CUDA 12.4 → CUDA 11.8 → CPU+BLAS → CPU

#### UI
- Консоль **автоматически скрывается** при запуске (только трей)
- Пункт меню «Показать окно» / «Скрыть окно» — динамический текст
- Иконка `vox-mim.ico` встроена в `.exe` через `embed-resource`

#### DevOps
- `embed-resource` = "3.0" в build-dependencies для встраивания .ico
- `resource/resource.rc` — файл ресурсов Windows

## [0.2.0] — 2026-07-01

### MVP — Push-to-talk → запись → whisper → текст → вставка

#### Core
- Переход с `whisper-rs` (FFI) на `whisper-cli.exe` subprocess с CUDA
- Удалена зависимость `whisper-rs` (и `bindgen` с `libclang`)
- Удалена зависимость `tray-icon` — трей на Win32 API (`NOTIFYICONDATAW`)
- Удалён `phf` + `phf_codegen` — словарь загружается из файла при старте
- Добавлены: `image` crate (загрузка PNG), `crossbeam-channel`

#### Audio
- Ресемплинг 48kHz → 16kHz (усреднение блоков)
- Подбор частоты захвата (48000 → 32000 → 24000 → 16000 → 8000)
- Сохранение частоты в config.json (не детектится при каждом запуске)
- Отказ от `webrtc-audio-processing` (не собрался на Windows)

#### STT
- `whisper-cli.exe` из `cu-bin-blas12.4` с CUDA 12.4
- Поддержка двух моделей: детектор (small) + транскрайбер (large)
- Транскрипция через временный WAV-файл (stdin не поддерживался)

#### Text Fixer
- Словарь ~200K слов из `assets/ru_words_utf8.txt` (загрузка при старте)
- `RwLock`-безопасное переключение языков в рантайме
- Исправлен `is_short_token` — регистрозависимость (Заглавная И, У)
- Исправлен double-push SymSpell в `space_fixer`
- Исправлен UTF-8 crash в prefix check

#### Input
- **Глобальный хоткей:** Win32 `WH_KEYBOARD_LL` вместо `rdev` (работает без админ-прав)
- **Сохрание/восстановление буфера обмена** — скопированный текст не затирается
- Smart spacing через `EM_GETSEL` + `WM_GETTEXT`

#### UI
- **Трей на Win32 API:** `CreatePopupMenu`, `TrackPopupMenu` с `TPM_BOTTOMALIGN`
- Меню: Версия, Настройки, VAD, Math Mode, Выход
- Иконки: `blue-voice.png` (IDLE) / `microphone-stage-light.png` (RECORDING)
- Message-заглушка для окна настроек
- Иконки копируются в `target/debug/assets/` при сборке

#### Commands
- Парсинг VoxBee-формата команд (multilingual triggers)
- Защита от ложного срабатывания — `command_max_words` (3+ слов = диктовка)

#### Config
- Миграция из VoxBee при первом запуске
- Новые поля: `wake_mode`, `wake_words`, `detector_model`, `command_max_words`, `capture_sample_rate`
- Автопоиск GGML-моделей в `C:\_workPortable\WhisperCpp\models\`

### Fixed
- UTF-8 panic в prefix check `space_fixer` (срез по байтам кириллицы)
- Double-push SymSpell → дублирование слов ("Еще Иеще")
- "И", "У" в начале предложения считались короткими токенами
- Меню трея уходило за экран — `TPM_BOTTOMALIGN`
- `SetForegroundWindow` redeclared warning
- ClipBoard затирался после вставки — сохранение и восстановление
- `unsafe_op_in_unsafe_fn` warnings (Rust 2024 edition)

### Removed
- `whisper-rs`, `whisper-rs-sys`, `bindgen` — не нужны, используем whisper-cli
- `tray-icon` — заменён на Win32 API
- `phf`, `phf_codegen` — словарь из файла
- `rdev` для клавиатуры — заменён на `WH_KEYBOARD_LL`

## [0.1.0] — Начальный каркас

### Added (при переходе от VoxBee к VoxMiM)

#### Core
- Полный порт VoxBee с Python (3.11) на Rust (2024 edition)
- Каркас проекта: модули audio, stt, vad, text, input, commands, ui
- Конфиг в JSON через `serde` + `serde_json`

#### STT
- Интеграция `whisper-rs` (замена на whisper-cli в 0.2.0)

#### UI
- `tray-icon` (замена на Win32 API в 0.2.0)

#### DevOps
- `build.rs` с `phf_codegen` (удалён в 0.2.0)
