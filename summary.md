# VoxMiM v0.9.8 — Итоговая сводка

## Что сделано

**VoxMiM** — голосовой ввод текста на Rust. По мотивам VoxBee с исправлением ключевой ошибки (разорванные слова).

> **v0.9.5:** Pending.raw (сырые f32+rate, без ресемпла). Transcribe_server без диска. Детальный лог на каждом шаге ошибки. Починен двойной ресемпл.
>
> **v0.9.2:** Таймаут распознавания whisper (настраивается 10–180 сек). Retry-цикл 3 попытки при сбое сервера с backoff. Логи `warn!`/`error!` на каждый сбой. UI-настройка на вкладке «Постобработка».
>
> **v0.9.1:** Clipboard-операции вынесены в отдельный поток `inserter`. Исправлено зависание программы: main thread больше не блокируется на `OpenClipboard`/`SetClipboardData`. Хоткеи, запись и трей работают, даже если clipboard недоступен.
>
> **v0.9.0:** VAD: агрессивность (0–3) → прямой порог RMS (f32) с шагом 0.002, дефолт 0.008. Новое поле «Чувствительность микрофона» со спин-кнопками (▲/▼) и сбросом (↩). Реактивная синхронизация между окном настроек и треем (фоновый polling config.json каждые 500ms). Исправлена миграция: `aggressiveness` больше не перезаписывает пользовательский `threshold` при каждом запуске. Startup: в server-режиме `ensure_whisper_bins` не вызывается, лог модели убран в `debug!`.
>
> **v0.8.0:** Детекция через whisper-server (HTTP:8178), расширение словаря галлюцинаций (сортировка, диалог добавления, регистронезависимые regex, все фразы в файле), спин-кнопки таймаутов (icon_button Xs Ghost), переименование пунктов трея, silence_duration_secs 1.5 по умолчанию, сохранение настроек при закрытии, радио-кнопки engine mode, build.rs не затирает пользовательские файлы, `__sync_dict.bat` копирует hallucinations.txt.
>
> **Wake Word откачен до v0.7.5** — полная переработка заморожена из-за каскада ошибок (prebuf ломал VAD, `IS_PROCESSING` блокировал запись, `SetTab` сбрасывал engine_mode). Оставлена оригинальная реализация с 0.5s окном и whisper-cli.
>
> **v0.7.4:** VAD полностью переработан и работает стабильно. Ключевое исправление: VAD не слышал микрофон из-за неправильной формулы (сырая энергия mean(s²) на микро-кусочках). Переведён на RMS + накопление 100мс чанков. Добавлена настройка keep_wav для сохранения WAV-файлов. Исправлены гонки состояния (двойной дренаж, stale RecordingResult, HOOK_REC в tap-mode).

### Рабочий процесс (MVP)

```
Ctrl+Insert → запись → отпустить → whisper (GPU) → текст → вставка в окно
```

### Ключевые компоненты

| Компонент | Статус | Технология |
|---|---|---|
| Аудио-захват | ✅ | cpal (WASAPI), fan-out для VAD + Wake |
| VAD (Автостоп) | ✅ | RMS-based, накопление 100мс чанков, pre-speech таймаут, silence timeout, tap-режим |
| Wake Word | ⚠️ код написан, не тестирован | whisper-cli small, общий аудио-стрим |
| Распознавание | ✅ | whisper-cli, CUDA 12.4, RTX 3080 Ti |
| Склейка слов | ✅ | 200K словарь + эвристики + SymSpell **(избыточно после v0.7.2)** |
| Удаление `\n` из ответа сервера | ✅ | whisper server резал слова фиксированной шириной — `\n` убирается, слова не разрываются |
| Глобальный хоткей | ✅ | Win32 WH_KEYBOARD_LL, GetAsyncKeyState (без рассинхронизации) |
| Вставка текста | ✅ | Win32 Clipboard + Save/Restore |
| Трей-иконка | ✅ | Win32 NOTIFYICONDATAW + переключение IDLE/RECORDING/загрузка |
| Иконка загрузки | ✅ | hourglass-fill.png мигает до готовности моделей |
| Команды голосом | ✅ | 199 команд из VoxBee |
| Буфер обмена | ✅ | Сохраняется и восстанавливается |
| Пользовательский словарь | ✅ | dicts/user_dict.json + диалог добавления |
| Smart Spacing | ✅ | AUTO-пробел перед вставкой |
| Локализация (i18n) | ✅ | lang/ru.json + en.json, Localizer-синглтон |
| Console toggle | ✅ | Показать/скрыть консоль из трея |
| Иконка .exe | ✅ | vox-mim.ico вшита в бинарник |
| Авто-загрузка | ✅ | whisper-cli скачивается при первом запуске |
| Portable сборка | ✅ | Всё в папке с .exe: config, dicts, bins, models |
| Single-instance | ✅ | WaitForSingleObject + WAIT_ABANDONED |
| Кастомные галлюцинации | ✅ | hallucinations.txt в dicts/ |
| Грациозный выход | ✅ | Нормальное завершение через WM_DESTROY |
| Окно настроек (Fenestra) | ✅ | Отдельный .exe, Named Pipe IPC, локализация, тёмная тема |
| Always-on-top | ✅ | CBT-hook с WS_EX_TOPMOST + SetWindowPos |

### Архитектура

```
Потоки: main | audio-accum (AudioProcessor) | whisper | hotkey | tray | wake-detect
Каналы: crossbeam-channel (cmd_tx/rx, whisper_tx/rx), mpsc (аудио fan-out), Named Pipe (settings IPC)
Память: обе модели в GPU (3.4 GB из 12 GB)
Сборка: cargo build --workspace (voxmim + voxmim-settings)
```

### Файлы

```
settings/
├── Cargo.toml               # Fenestra-зависимости
├── build.rs                 # embed-resource (иконка)
├── resource/resource.rc     # .ico resource
└── src/main.rs              # Окно настроек (Fenestra), Named Pipe IPC

### Файлы

```
assets/
├── blue-voice.png            # Иконка трея (IDLE) — вшивается в .exe
├── microphone-stage-light.png# Иконка трея (RECORDING) — вшивается в .exe
├── hourglass-fill.png        # Иконка трея (загрузка) — вшивается в .exe
├── hand-palm.png             # Запасная иконка загрузки
├── vox-mim.ico               # Иконка .exe
├── ru_words_utf8.txt         # Словарь ~2.4M слов (копируется в dicts/)
├── russian.txt               # Исходный словарь cp1251
└── russian_surnames.txt      # Фамилии cp1251

lang/
├── ru.json                   # Русская локаль (UI-строки)
└── en.json                   # Английская локаль (заготовка)

resource/
└── resource.rc               # Windows resource file (иконка)

src/
├── main.rs                   # Точка входа + скрытие консоли
├── config.rs                 # Config + миграция из VoxBee
├── download.rs               # Авто-скачивание whisper-cli
├── app.rs                    # Event loop + state machine
├── lang.rs                   # Localizer (загрузка локалей, t(), t_utf16())
├── audio/capture.rs          # cpal захват + fan-out (start_capture_multi)
├── audio/processor.rs        # AudioProcessor: PTT + VAD автостоп + ring buffer
├── audio/ring_buffer.rs      # Pre-roll буфер
├── audio/noise_filter.rs     # Шумовой гейт
├── stt/engine.rs             # Whisper CLI обёртка
├── vad/detector.rs           # VAD детектор
├── text/mod.rs               # fix_text() оркестратор
├── text/space_fixer.rs       # Склейка разорванных слов ★
├── text/dictionary.rs        # Словарь (RwLock) + замены
├── text/hallucinations.rs    # Удаление галлюцинаций
├── text/repetitions.rs       # Схлопывание повторов
├── text/punctuation.rs       # Пунктуация
├── text/aliases.rs           # Фонетические алиасы
├── input/inserter.rs         # Win32 Clipboard
├── input/hotkeys.rs          # WH_KEYBOARD_LL + VAD tap-режим
├── input/simulation.rs       # enigo
├── commands/executor.rs      # JSON-команды
├── commands/math.rs          # Математический режим
├── ui/tray.rs                # Win32 трей + чекбоксы + локализация
├── pipe.rs                   # Named Pipe сервер (IPC с окном настроек)
├── lang.rs                   # Localizer (загрузка локалей, t(), t_utf16())
├── debug_log.rs              # dlog! макрос для отладки
└── app.rs                    # Event loop + state machine
```

### Тесты

- **18 unit-тестов** — все проходят
- **v0.8.0:** галлюцинации через regex (без диска, регистронезависимые), спин-кнопки, сохранение настроек при закрытии, radio вместо segmented
- **v0.7.5:** вычищен мёртвый код Slint, бинарник уменьшен с 35 до 11 МБ
- **v0.7.3:** VAD pre-speech таймаут + фикс автоповтора Insert + сброс HOOK_REC
- **v0.5.1:** space_fixer — защита от склейки предлогов короче 3 символов
- **v0.5.2:** space_fixer — убрана эвристика согласная→гласная, SymSpell только точное совпадение
- **v0.6.0:** `text/user_dict.rs` — пользовательский словарь + кеш regex + границы через `is_alphabetic`
- **v0.7.0:** `vad/detector` + `audio/processor` + `lang` — новые модули
- Модули: space_fixer, hallucinations, repetitions, punctuation, math, vad, user_dict

### Версии

- **voxmim**: 0.9.8 (stop_server условный, fix — не убивает чужой сервер при дропе detector)
- **voxmim-settings**: 1.3.0 (чувствительность микрофона со спин-кнопками, polling config.json, таймаут whisper)

### Сборка

```bash
cargo build --workspace       # debug (voxmim + voxmim-settings)
cargo build --release --workspace  # release с LTO
cargo test                    # 18 тестов
```

Для быстрого запуска: `__run.bat` (сборка + запуск, лог в `_build.log`)
Для отладки: `__run_debug.bat` (сборка + `RUST_LOG=debug`)

## Что не сделано / Backlog

- [ ] **HTTP API** — сервер приёма WAV → whisper → fix_text → JSON (порт из конфига)
- [x] **Окно настроек (Fenestra)** — v0.7.2
- [x] **VAD (Автостоп)** — v0.7.4, RMS + накопление 100мс, стабильно работает
- [-] **Wake Word** — v0.7.0 (попытка переработки в v0.8.0 → откат). Проблемы: prebuf & IS_PROCESSING ломали VAD, настроечный код сбрасывал engine_mode. Решено откатиться и пересобрать базу.
- [x] **Локализация (i18n)** — v0.7.0
- [x] **Always-on-top** — v0.7.2
- [x] **Корень разорванных слов (`\n` в whisper server)** — v0.7.2
- [x] **README.md** (EN + RU)
- [x] **LICENSE** (MIT)
- [x] **Пользовательский словарь** — v0.6.0
- [x] **Словарь галлюцинаций (все фразы в файле, сортировка, регистронезависимые regex)** — v0.8.0
- [x] **Спин-кнопки таймаутов (icon_button, ▲/▼)** — v0.8.0
- [x] **Сохранение настроек при закрытии** — v0.8.0
- [x] **Радио-кнопки engine mode** — v0.8.0
- [x] **Сохранение текущей вкладки** — v0.8.1, SetTab → apply()
- [ ] **macOS/Linux** порт
- [ ] **Английский язык** — словарь + space fixer (низкий приоритет, root cause найден)
- [ ] **CI/CD** (GitHub Actions)
- [x] **Вычищен мёртвый код Slint** — v0.7.5, debug-бинарник уменьшен с 35 до 11 МБ
- [x] **Независимые версии voxmim (0.x.x) и voxmim-settings (1.1.0)** — разные семвер-линии
- [ ] **Скрытие строки заголовка окна настроек** — замена костыльного `remove_window_caption()` (EnumWindows + задержка → мелькание). Нужно: Fenestra-опция без caption, или SetWindowLong до ShowWindow, или HTCAPTION для перетаскивания
- [x] **Версионность окна настроек** — v1.1.0, показывает `env!("CARGO_PKG_VERSION")` в заголовке
