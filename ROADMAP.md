# Roadmap

> План разработки VoxMiM — порта VoxBee на Rust с исправлением ключевых ошибок.

## Тип модуля

**[SENSE]** — слух. Распознавание речи, голосовой ввод, wake word. Часть экосистемы DJA.

## Легенда

| Метка | Значение |
|---|---|
| 🟢 Ready | Код написан, протестирован |
| 🟡 In Progress | В разработке |
| 🟠 Planned | Запланировано |
| ⚪ Backlog | Будет рассмотрено позже |

---

## Фаза 1: Каркас проекта 🟢

- [x] Инициализация Cargo-проекта, структура директорий
- [x] `Cargo.toml` со всеми зависимостями
- [x] `config.rs` — `Config` struct + `serde` + JSON IO
- [x] `main.rs` — single instance mutex, DPI awareness, точка входа
- [x] `app.rs` — `AppState`, `AppCommand`, `App` struct, event loop
- [x] `crossbeam-channel`: `cmd_tx/rx`, `result_tx/rx`
- [x] Логирование: `log` + `env_logger` + файловый sink
- [x] Определение путей через `directories` (BIN_DIR, MODELS_DIR, DATA_DIR, CONFIG_PATH)

## Фаза 2: Аудио-пайплайн 🟢

- [x] `audio/capture.rs`
  - [x] Enum микрофонов через `cpal`
  - [x] Открытие `InputStream` (48000Hz, f32, mono с авто-подбором частоты)
  - [x] `start_capture()` / кеширование частоты в config.json
  - [x] Резолв устройства по имени (с fallback на индекс)
- [x] `audio/ring_buffer.rs` — написан (ожидает интеграции)
- [x] `audio/noise_filter.rs` — написан (ожидает интеграции)

## Фаза 3: STT Engine 🟢

- [x] `stt/engine.rs`
  - [x] `load_model(path, use_gpu) → Result` — через whisper-cli subprocess
  - [x] `transcribe(&[f32]) → String` — 48→16kHz ресемплинг + WAV temp file
  - [x] Параметры: `language`, `n_threads`, `input_rate`
  - [x] Детектор + транскрайбер (две модели, обе в GPU)
  - [x] Warmup при старте
- [x] GPU через whisper-cli `cu-bin-blas12.4` (CUDA 12.4)

## Фаза 4: VAD (энергетический) 🟡

- [x] `vad/detector.rs`
  - [x] Энергетический VAD (aggressiveness 0-3)
  - [x] `VadEvent { Silence, SpeechStart, Speech }`
  - [x] Короткая речь (опционально)
- [ ] Интеграция с `app.rs` — SpeechStart/End (ожидает подключения)

## Фаза 5: Text Fixer 🟢

### 5a. Инфраструктура
- [x] `text/mod.rs` — `fix_text()` orchestrator
- [x] Словарь 200K+ слов из `assets/ru_words_utf8.txt`

### 5b. Space Fixer ★
- [x] `text/space_fixer.rs`
  - [x] Словарный проход (HashSet runtime)
  - [x] Эвристики: согласная→гласная, короткие токены (регистронезависимые)
  - [x] SymSpell fallback (без double-push)
  - [x] Конфигурация: on/off toggle
  - [x] Тесты: "произволь ных" → "произвольных", "и их" → "и их"

### 5c. Остальные этапы
- [x] `text/hallucinations.rs` — удаление галлюцинаций
- [x] `text/dictionary.rs` — встроенный словарь (питон→Python)
- [x] `text/repetitions.rs` — схлопывание повторов
- [x] `text/punctuation.rs` — капитализация, точка, пробелы

## Фаза 6: Input 🟢

- [x] `input/inserter.rs`
  - [x] Win32 Clipboard API: сохранение, запись, восстановление
  - [x] `SendInput` для Ctrl+V, Backspace
  - [x] Smart spacing (символ слева от каретки через `EM_GETSEL`)
  - [x] `AttachThreadInput` для фокуса окна
- [x] `input/hotkeys.rs`
  - [x] **Win32 WH_KEYBOARD_LL** (замена rdev)
  - [x] Ctrl+Insert — Start/StopRecording
  - [x] Mouse button (middle/right/extra) через rdev
- [x] `input/simulation.rs` — написан (ожидает подключения)

## Фаза 7: UI 🟢

- [x] `ui/tray.rs`
  - [x] **Win32 NOTIFYICONDATAW** (замена tray-icon)
  - [x] Меню: Версия, Настройки, VAD, Math Mode, Выход
  - [x] `TPM_BOTTOMALIGN` — меню не уходит за экран
  - [x] Иконки IDLE/RECORDING из PNG
- [ ] `ui/settings.rs`
  - [ ] egui/eframe окно настроек (заглушка через MessageBox)

## Фаза 8: Команды 🟢

- [x] `commands/executor.rs`
  - [x] Загрузка JSON-команд (VoxBee-формат)
  - [x] `try_execute(text) → Option<Command>` с защитой от ложных срабатываний
  - [x] Command types: paste, hotkey, mouse, focus, script, grid, math toggle
  - [x] Multilingual triggers (common/ru/en)
- [x] `commands/math.rs`
  - [x] Числительные: "два"→2
  - [x] Операторы: плюс→+, минус→−

## Фаза 9: Полировка 🟢

- [x] Ассеты: PNG иконки для трея
- [x] Ассеты: `.ico` для .exe (встроен через embed-resource)
- [x] `.gitignore`
- [x] Юнит-тесты для `text/` модулей (18 тестов)
- [x] `Cargo.toml` profile overrides (release → LTO, optimize)
- [x] Авто-скачивание whisper-cli (download.rs)
- [x] Console toggle (показать/скрыть из трея)
- [x] `README.md`
- [x] `LICENSE` (MIT)
- [ ] Интеграционное тестирование (end-to-end)
- [ ] Обработка ошибок во всех модулях
- [ ] Оптимизация: ленивая загрузка, кэширование

---

## Фаза 10: Пользовательский словарь 🟢 (v0.6.0)

Замена неправильно распознанных слов на корректные. Например: "джа" → "DJA", "вокс мим" → "VoxMiM".

### 10a. Инфраструктура
- [x] `text/user_dict.rs` — загрузка словаря из `user_dict.json`
- [x] `dicts/user_dict.json` — файл пользовательского словаря (создаётся при первом добавлении)
- [x] Интеграция в `fix_text()` — после dictionary::apply_dict, перед repetitions

### 10b. Алгоритм
- [x] Поиск подстрок (регистронезависимый) в распознанном тексте
- [x] Замена на корректное написание из словаря
- [x] Поддержка фраз любой длины: `"вокс мим" → "VoxMiM"`
- [x] Unicode-границы через `char::is_alphabetic()` (без «воксмимолёт»)
- [x] Кеш предварительно скомпилированных regex'ов
- [ ] Поддержка regex (опционально): `"джа(а+)?" → "DJA"`

### 10c. UI
- [x] Диалог «Добавить слово» (Win32, два поля + кнопки, модальный)
- [x] Пункт в трее: «Добавить слово...»
- [x] Пункт в трее: «Редактировать словарь» → блокнот

---

## Фаза 11: Триггер-фраза (Wake Word) 🟠

Активация по голосовой команде вместо горячей клавиши.

- [ ] Интеграция `vad/detector.rs` в `app.rs` (SpeechStart/End)
- [ ] Wake word detection: непрерывное слушание → распознавание триггер-фразы
- [ ] Триггер-фразы: «Слушай», «Бро записывай», «Джа» (настраиваемые)
- [ ] После триггера → запись команды → распознавание → вставка/выполнение
- [ ] Индикатор в трее: «Слушаю триггер» (отдельная иконка)
- [ ] Настройка чувствительности wake word в config.json

---

## Фаза 12: REST API / DCP-интеграция 🟠

VoxMiM как модуль экосистемы DJA. Единый протокол JSON-RPC 2.0.

### 12a. HTTP-сервер
- [ ] `api/server.rs` — axum HTTP-сервер (порт 18430)
- [ ] Запуск в отдельном потоке (не блокирует PTT)
- [ ] `POST /api/rpc` — JSON-RPC 2.0 endpoint

### 12b. DCP-методы
- [ ] `voxmim.stt.transcribe` — распознать аудио (WAV/PCM → текст)
- [ ] `voxmim.command.execute` — выполнить голосовую команду
- [ ] `voxmim.status` — статус (слушает, молчит, записывает)
- [ ] `voxmim.config.get` / `voxmim.config.set` — настройки

### 12c. Регистрация в DJA
- [ ] При запуске: `POST http://localhost:18420/__dja/register`
- [ ] Capabilities: `stt.transcribe`, `command.execute`, `status`
- [ ] Events: `module.up`, `module.down`, `voice.command`

### 12d. Event Bus
- [ ] Публикация событий: `voice.command` (распознана команда)
- [ ] Подписка на события от DJA (например, `system.mute`)

---

## Вехи (Milestones)

| Веха | Фазы | Критерий готовности | Статус |
|---|---|---|---|
| **MVP** | 1, 2, 3, 5, 6, 8 | Push-to-talk → запись → распознавание → вставка текста | 🟢 **v0.2.0** |
| **VAD** | +4 | VAD-режим: речь → авто-распознавание | 🟡 |
| **UI** | +7 | Трей-иконка + окно настроек | 🟡 |
| **Full** | +8 (done) | Команды + математический режим | 🟢 |
| **UserDict** | +10 | Пользовательский словарь (замена "джа" → "DJA") | 🟢 **v0.6.0** |
| **WakeWord** | +11 | Триггер-фраза вместо горячей клавиши | 🟠 |
| **DCP** | +12 | REST API + JSON-RPC 2.0 + регистрация в DJA | 🟠 |
| **Release** | +9 | 1.0.0: .exe, README, тесты | 🟡 |

---

## Технический долг / Backlog ⚪

- [ ] **HTTP API** — сервер для приёма WAV → whisper → fix_text → JSON (порт из конфига, неблокирующий для PTT) → **перенесено в Фазу 12**
- [ ] **Настраиваемые уровни эвристик склейки** (`space_fixer_level` в config.json):
  - **Level 0:** только словарь, min 3 символа
  - **Level 1:** словарь + короткие токены
  - **Level 2:** словарь + короткие токены + согласная→гласная
  - **Level 3:** словарь + короткие токены + согласная→гласная + суффиксы/префиксы (текущее поведение)
- [ ] Авто-скачивание моделей Whisper
- [ ] Поддержка macOS (Core Audio + CGEvent вместо Win32)
- [ ] Поддержка Linux (PulseAudio/ALSA + X11)
- [ ] Английский язык (словарь + space fixer)
- [ ] Горячие клавиши настраиваемые (не только кнопка мыши)
- [ ] Плагины/скрипты на Rust/WASM
- [ ] GUI на native Windows controls вместо egui
- [ ] Авто-обновление
- [ ] Профилирование и оптимизация времени транскрибации
- [ ] Библиотека для повторного использования (crate)
- [ ] CI/CD: GitHub Actions (build + test + release artifact)
