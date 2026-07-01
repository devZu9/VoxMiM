# Changelog

All notable changes to VoxMiM will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
