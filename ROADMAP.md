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

## Фаза 4: VAD (энергетический) 🟢

> **v0.7.4: Полностью работоспособен. Исправлена ключевая ошибка — VAD не слышал микрофон.**
> **v0.9.0: Агрессивность (0–3) заменена на прямой порог RMS (f32) — поле «Чувствительность микрофона» со спин-кнопками и сбросом.**
> **Причина:** сырая средняя квадратов (`mean(s²)`) на микро-кусочках 1-10мс давала значения ниже порога, хотя речь в буфере была нормальной.
> **Исправление:** накопление чанков до 100мс + RMS (`√mean(s²)`) с порогами под реальный сигнал микрофона.

- [x] `vad/detector.rs`
  - [x] VAD на RMS (root-mean-square) вместо сырой энергии
  - [x] Пороги: aggr 0=0.05, 1=0.03, 2=0.015, 3=0.008 (RMS) → threshold f32 (0.008 по умолч.)
  - [x] `VadEvent { Silence, SpeechStart, Speech }`
  - [x] Короткая речь (опционально)
- [x] Интеграция с `app.rs` — AudioProcessor + VAD автостоп
- [x] Pre-speech таймаут — `start_timeout_secs` (2 сек, настраивается)
- [x] Фикс автоповтора Insert — `VAD_KEY_LOCK` фильтр
- [x] Сброс HOOK_REC — `reset_recording_state()` при автостопе
- [x] Накопление чанков ~100мс перед VAD (сглаживает микро-провалы внутри речи)
- [x] Исправление гонки состояния: stale RecordingResult не ломает новую запись
- [x] Исправление двойного дренажа audio_buf при VAD-автостопе
- [x] VAD tap-mode: Insert всегда шлёт StartRecording (force-stop делает on_start)

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
- [x] `ui/settings.rs`
  - [x] egui/eframe окно настроек (заглушка через MessageBox)
  - [x] **Fenestra-окно настроек** — отдельный .exe, Named Pipe IPC, локализация, тёмная тема, всегда поверх

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
- [x] Вычищен мёртвый код (Slint → Fenestra) — v0.7.5
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

## Фаза 11: Триггер-фраза (Wake Word) ❌ откат до v0.7.5

> **Попытка полной переработки в v0.8.0 заморожена. Возврат к оригинальной реализации v0.7.5.**

### Что было сделано в v0.8.0:
- [x] Скользящее окно 0.8с с шагом 100ms (вместо 0.5с без перекрытия)
- [x] Runtime-переключение через статический флаг `WAKE_ENABLED`
- [x] Wake → только детектор → `StartRecording` → VAD берёт на себя запись
- [x] Prebuf из ring_buf AudioProcessor при старте
- [x] UserDict.apply() в wake-треде
- [x] Два whisper-server (8178+8179): small для детекции, large для транскрипции
- [x] Вся детекция в памяти, без диска
- [x] Триггер-фразы, окно детекции, таймаут — настройки в Fenestra
- [x] Регистронезависимая нормализация + fuzzy match (Levenshtein)

### Почему откатили:
- **Prebuf ломал VAD** — AudioProcessor копировал ring_buf в audio_buf при старте записи, добавляя секунду тишины в начало. Whisper обрабатывал лишние данные, отчего транскрипция тормозила с 2с до 7-8с.
- **`IS_PROCESSING` блокировал запись** — новый `StartRecording` не мог начаться пока обрабатывался предыдущий результат, хотя VAD уже закончил.
- **`SetTab` сбрасывал engine_mode** — `set_from_value()` перезаписывал режим сервера на one-shot.
- **Настройки не сохранялись** — `Close` не вызывал `apply()`.
- **Каскад ошибок** — каждое исправление ломало что-то ещё, итерация затянулась.

### Решение: полный откат `git checkout -- .` до v0.7.5. Оригинальная реализация Wake Word с 0.5s окном и whisper-cli осталась в коде, но отключена по умолчанию. Wake Word возвращается в backlog.

---

## Фаза 11a: Wake Word + VAD Integration ❌ suspended

---

## Фаза 12: REST API / DCP-интеграция 🟠

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

## Фаза 13: Локализация (i18n) 🟢

- [x] `lang/ru.json` + `lang/en.json` — UI-строки трея и диалогов
- [x] `src/lang.rs` — Localizer-синглтон (загрузка, `t()`, `t_utf16()`)
- [x] Трей-меню читает строки из локали
- [x] Диалог «Добавить слово» читает строки из локали
- [ ] Окно настроек (egui) — локализация
- [ ] Локализация логов и сообщений (опционально)

---

## Вехи (Milestones)

| Веха | Фазы | Критерий готовности | Статус |
|---|---|---|---|
| **MVP** | 1, 2, 3, 5, 6, 8 | Push-to-talk → запись → распознавание → вставка текста | 🟢 **v0.2.0** |
| **VAD** | +4 | VAD-режим: автостоп по тишине + pre-speech таймаут | 🟢 **v0.7.4** |
| **UI** | +7 | Трей-иконка + окно настроек | 🟢 **v0.7.2** |
| **Full** | +8 (done) | Команды + математический режим | 🟢 |
| **UserDict** | +10 | Пользовательский словарь (замена "джа" → "DJA") | 🟢 **v0.6.0** |
| **WakeWord** | +11 | Триггер-фраза вместо горячей клавиши | ❌ **v0.8.0: попытка → откат** |
| **Wake+VAD** | +11a | Wake + VAD без конфликтов | ❌ **suspended** |
| **i18n** | +13 | Локализация (RU/EN) | 🟢 |
| **DCP** | +12 | REST API + JSON-RPC 2.0 + регистрация в DJA | 🟠 |
| **Release** | +9 | 1.0.0: .exe, README, тесты | 🟡 |
| **Clipboard** | +9a | Clipboard изолирован от main loop | 🟢 **v0.9.1** |
| **WhisperTimeout** | +9b | Таймаут + retry при зависании whisper-server | 🟢 **v0.9.2** |
| **WhisperFastFail** | +9c | State = Idle на первой ошибке, дефолт 60s, ретраи 30s | 🟢 **v0.9.3** |
| **PendingWav** | +9d | Pending.wav + индикатор перезапуска в трее | 🟢 **v0.9.4** |

---

## Технический долг / Backlog ⚪

- [ ] **HTTP API** — сервер для приёма WAV → whisper → fix_text → JSON (порт из конфига, неблокирующий для PTT) → **Фаза 12**
- [x] **Независимые версии voxmim и voxmim-settings** — основной проект продолжает 0.x.x (SemVer), settings стартует с 1.0.3 как отдельное стабильное приложение со своей версионной линией
- [ ] **Скрытие строки заголовка окна настроек** — `remove_window_caption()` с `EnumWindows` + задержка костыльный (мелькание). Нужно: Fenestra-опция без caption при создании, или `SetWindowLong` до `ShowWindow`, или кастомная область перетаскивания (HTCAPTION)
- [x] **Версионность окна настроек** — отображение `env!("CARGO_PKG_VERSION")` в заголовке (v1.0.3)
- [ ] Авто-скачивание моделей Whisper
- [ ] Поддержка macOS (Core Audio + CGEvent вместо Win32)
- [ ] Поддержка Linux (PulseAudio/ALSA + X11)
- [ ] Английский язык (словарь + space fixer)
- [ ] Горячие клавиши настраиваемые (не только кнопка мыши)
- [ ] Плагины/скрипты на Rust/WASM
- [ ] GUI на native Windows controls вместо egui → **Fenestra** 🟢
- [ ] Авто-обновление
- [ ] Профилирование и оптимизация времени транскрибации
- [ ] Библиотека для повторного использования (crate)
- [ ] CI/CD: GitHub Actions (build + test + release artifact)
- [x] Окно настроек (Fenestra) — локализация + тёмная тема + всегда поверх
- [ ] Поддержка дополнительных языков локали (DE, FR, ES, ...)
- [ ] `lang/` копируется при portable-сборке
- [x] **Clipboard изолирован от main loop** — insert_text() вынесен в отдельный поток (v0.9.1)
- [x] **Таймаут whisper + retry** — `set_read_timeout()`, 3 попытки с backoff, настройка в окне настроек (v0.9.2)
- [x] **State = Idle сразу, дефолт 60s** — первая ошибка сбрасывает блокировку, ретраи 30s (v0.9.3)
- [x] **Pending WAV + индикатор в трее** — фраза сохраняется на диск, hourglass при перезапуске (v0.9.4)
