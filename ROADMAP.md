# Roadmap

> План разработки VoxMiM — порта VoxBee на Rust с исправлением ключевых ошибок.

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
- [ ] `README.md` (EN + RU)
- [ ] `LICENSE` (GPL-3.0)
- [ ] Интеграционное тестирование (end-to-end)
- [ ] Обработка ошибок во всех модулях
- [ ] Оптимизация: ленивая загрузка, кэширование

---

## Вехи (Milestones)

| Веха | Фазы | Критерий готовности | Статус |
|---|---|---|---|
| **MVP** | 1, 2, 3, 5, 6, 8 | Push-to-talk → запись → распознавание → вставка текста | 🟢 **v0.2.0** |
| **VAD** | +4 | VAD-режим: речь → авто-распознавание | 🟡 |
| **UI** | +7 | Трей-иконка + окно настроек | 🟡 |
| **Full** | +8 (done) | Команды + математический режим | 🟢 |
| **Release** | +9 | 1.0.0: .exe, README, тесты | 🟡 |

---

## Технический долг / Backlog ⚪

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
