# VoxMiM v0.4.0 — Итоговая сводка

## Что сделано

**VoxMiM** — голосовой ввод текста на Rust. Порт VoxBee с исправлением ключевой ошибки (разорванные слова).

### Рабочий процесс (MVP)

```
Ctrl+Insert → запись → отпустить → whisper (GPU) → текст → вставка в окно
```

### Ключевые компоненты

| Компонент | Статус | Технология |
|---|---|---|
| Аудио-захват | ✅ | cpal (WASAPI), автоподбор частоты |
| Распознавание | ✅ | whisper-cli, CUDA 12.4, RTX 3080 Ti |
| Склейка слов | ✅ | 200K словарь + эвристики + SymSpell |
| Глобальный хоткей | ✅ | Win32 WH_KEYBOARD_LL, GetAsyncKeyState (без рассинхронизации) |
| Вставка текста | ✅ | Win32 Clipboard + Save/Restore |
| Трей-иконка | ✅ | Win32 NOTIFYICONDATAW + переключение IDLE/RECORDING/загрузка |
| Иконка загрузки | ✅ | hourglass-fill.png мигает до готовности моделей |
| Команды голосом | ✅ | 199 команд из VoxBee |
| Буфер обмена | ✅ | Сохраняется и восстанавливается |
| Smart Spacing | ✅ | AUTO-пробел перед вставкой |
| Console toggle | ✅ | Показать/скрыть консоль из трея |
| Иконка .exe | ✅ | vox-mim.ico вшита в бинарник |
| Авто-загрузка | ✅ | whisper-cli скачивается при первом запуске |

### Архитектура

```
Потоки: main | audio-accum | whisper | hotkey | tray | wake-detect
Каналы: crossbeam-channel (cmd_tx/rx, whisper_tx/rx)
Память: обе модели в GPU (3.4 GB из 12 GB)
Сборка: cargo, без LIBCLANG_PATH, без bindgen
```

### Файлы

```
assets/
├── blue-voice.png            # Иконка трея (IDLE)
├── microphone-stage-light.png# Иконка трея (RECORDING)
├── hourglass-fill.png        # Иконка трея (загрузка)
├── hand-palm.png             # Запасная иконка загрузки
├── vox-mim.ico               # Иконка .exe
├── ru_words_utf8.txt         # Словарь ~2.4M слов (выборка 200K)
├── russian.txt               # Исходный словарь cp1251
└── russian_surnames.txt      # Фамилии cp1251

resource/
└── resource.rc               # Windows resource file (иконка)

src/
├── main.rs                   # Точка входа + скрытие консоли
├── config.rs                 # Config + миграция из VoxBee
├── download.rs               # Авто-скачивание whisper-cli
├── app.rs                    # Event loop + wake word
├── audio/capture.rs          # cpal захват
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
├── input/hotkeys.rs          # WH_KEYBOARD_LL
├── input/simulation.rs       # enigo
├── commands/executor.rs      # JSON-команды
├── commands/math.rs          # Математический режим
├── ui/tray.rs                # Win32 трей
└── ui/settings.rs            # egui (заглушка)
```

### Тесты

- **18 unit-тестов** — все проходят
- Модули: space_fixer, hallucinations, repetitions, punctuation, math, vad

### Сборка

```bash
cargo build              # debug (без LIBCLANG_PATH)
cargo build --release    # release с LTO
cargo test               # 18 тестов
```

## Что не сделано / Backlog

- [ ] **Окно настроек** (egui) — заглушка через MessageBox
- [ ] **VAD-режим** — детектор написан, не подключён к пайплайну
- [ ] **Wake word** — код готов, требует `wake_mode: true` + модель
- [ ] **README.md** (EN + RU)
- [ ] **LICENSE** (GPL-3.0)
- [ ] **macOS/Linux** порт
- [ ] **Английский язык** — словарь + space fixer
- [ ] **CI/CD** (GitHub Actions)
