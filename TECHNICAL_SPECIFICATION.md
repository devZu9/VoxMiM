# VoxMiM — Техническое задание

> **VoxMiM** (Vox — голос, MiM — Mouse Input Mic) — голосовой ввод текста и управление компьютером на Rust.
> Полноценный порт [VoxBee](https://github.com/boris-agent007/Voxbee) с исправлением ключевой ошибки: произвольные пробелы внутри распознанных слов.

---

## 1. Проблема (Motivation)

### 1.1. Исходная ошибка VoxBee

В любой момент распознавания в произвольных словах (например, `"произволь ных"`) появляются лишние пробелы. Это делает текст непригодным без ручного исправления.

### 1.2. Причина

Whisper (whisper.cpp) выдает текст блоками (BPE-токенами) без пробелов между ними. Программа-обёртка должна сама корректно соединять эти токоны. В VoxBee постпроцессор (`text_fixer.py`) **не содержит логики склейки разорванных слов** — он только меняет слова по словарю, убирает галлюцинации, повторы и исправляет пунктуацию.

В whisper.cpp для русского языка BPE-токенизация иногда разрывает слова на части (например, `произ-воль-ных` или `произволь-ных`), а декодер не всегда правильно их соединяет.

### 1.3. Решение

Реализовать в постпроцессоре **гибридный алгоритм склейки разорванных слов**:
1. Словарный проход — `HashSet<String>` из `assets/{lang}_words_utf8.txt`
2. Грамматические эвристики (согласная→гласная, короткие токены) — регистронезависимые
3. SymSpell fallback для fuzzy matching

---

## 2. Стек технологий

| Компонент | Крейт / Инструмент | Назначение |
|---|---|---|
| Язык | Rust 2024 edition | Основной язык |
| Аудио (захват) | `cpal` 0.15 | Запись с микрофона через WASAPI |
| Шумоподавление | Встроенный (энергетический гейт) | Простая замена webrtc-audio-processing |
| STT | `whisper-cli.exe` (subprocess) | Транскрибация через GPU (CUDA 12.4) |
| VAD | Собственная реализация (энергетический) | Voice Activity Detection |
| Текст (словарь) | `HashSet<String>` (runtime) | Загрузка из `assets/{lang}_words_utf8.txt` |
| Текст (regex) | `regex` 1 | Очистка, пунктуация, замена |
| Буфер обмена | `windows-sys` 0.59 | Win32 Clipboard API |
| Ввод с клавиатуры | `enigo` 0.6 | Симуляция Ctrl+V, хоткеи, мышь |
| Глобальные хоткеи | `WH_KEYBOARD_LL` (Win32) | Через SetWindowsHookExW |
| Трей-иконка | Win32 API (NOTIFYICONDATAW) | Системный трей (Windows) |
| Окно настроек | `egui` 0.30 / `eframe` 0.30 | Нативное окно с настройками |
| Конфигурация | `serde` + `serde_json` | JSON-файл конфига |
| Каналы | `crossbeam-channel` 0.5 | Межпоточная коммуникация |
| Пути | `directories` 5 | Стандартные пути ОС |
| Логирование | `log` + `env_logger` | Логи в stdout + файл |

---

## 3. Архитектура

### 3.1. Общая схема

```
                    ┌──────────────────────┐
                    │       main.rs         │
                    │    App state machine  │
                    │   (crossbeam-channel) │
                    └──┬────┬────┬────┬────┘
                       │    │    │    │
              channels │    │    │    │
        ┌──────────────┘    │    │    └──────────────┐
        ▼                   ▼    ▼                   ▼
┌──────────────┐   ┌──────────┐   ┌────────┐   ┌───────────┐
│ audio/       │   │ stt/     │   │ text/  │   │ input/    │
│ ├ capture    │   │ ├ engine │   │ ├ fixer │   │ ├ inserter│
│ ├ ring_buffer│   │ └ warmup │   │ ├ space │   │ ├ hotkeys │
│ └ noise_fltr │   └──────────┘   │ │ dict  │   │ └ simulator
└──────┬───────┘                  │ │ hal.. │   └───────────└
       │                          │ │ rep.. │
       │                          │ └ punct │
       │    ┌─────────────────────┘ └────────┘
       │    │
       ▼    ▼
┌────────────┐   ┌──────────┐   ┌──────────┐
│ vad/       │   │ commands/│   │ ui/      │
│ └ detector │   │ ├ executor│   │ ├ tray   │
└────────────┘   │ └ math    │   │ └ settings│
                 └──────────┘   └──────────┘
```

### 3.2. Модули

```
src/
├── main.rs                 # Entry point, single instance (Win32 Mutex)
│
├── app.rs                  # Application state machine
│   # enum AppState { Idle, Recording, Processing }
│   # struct App { config, dict, inserter, executor, ... }
│   # Central event loop: receive → dispatch → state transition
│
├── config.rs               # Структура Config (serde Deserialize)
│   # JSON config: микрофон, модель, GPU, VAD, text fixes, команды...
│
├── audio/
│   ├── mod.rs
│   ├── capture.rs           # cpal: enum устройств, открытие потока,
│   │                        #       start_capture(), авто-подбор частоты
│   ├── ring_buffer.rs       # Pre-roll буфер (0.5–2 сек до триггера)
│   └── noise_filter.rs      # Энергетический шумовой гейт
│
├── stt/
│   ├── mod.rs
│   └── engine.rs            # WhisperEngine: две модели (детектор + транскрайбер)
│                             # whisper-cli subprocess + WAV temp file
│
├── vad/
│   ├── mod.rs
│   └── detector.rs          # Энергетический VAD
│                             # process_chunk() → SpeechStart/SpeechEnd
│
├── text/
│   ├── mod.rs               # fix_text() orchestrator
│   ├── space_fixer.rs       # ★ СКЛЕЙКА РАЗОРВАННЫХ СЛОВ
│   │                        #    словарь (runtime HashSet) + эвристики + SymSpell
│   ├── dictionary.rs        # Dictionary struct (RwLock) + словарные замены
│   ├── hallucinations.rs    # Удаление галлюцинаций Whisper
│   ├── repetitions.rs       # Схлопывание повторов (да-да-да → да)
│   └── punctuation.rs       # Капитализация, точка в конце, пробелы
│
├── input/
│   ├── mod.rs
│   ├── inserter.rs          # Win32 Clipboard + Ctrl+V
│   │                        # сохранение/восстановление буфера
│   │                        # Smart spacing (символ слева от каретки)
│   ├── hotkeys.rs           # Win32 WH_KEYBOARD_LL (Ctrl+Insert)
│   │                        # + rdev для кнопки мыши
│   └── simulation.rs        # enigo: keyboard/mouse команды
│
├── commands/
│   ├── mod.rs
│   ├── executor.rs          # JSON-команды: paste, hotkey, mouse,
│   │                        # focus, script, none, grid, math toggle
│   └── math.rs              # Математический режим
│                             # "два плюс три" → "2 + 3"
│
└── ui/
    ├── mod.rs
    ├── tray.rs              # Win32 NOTIFYICONDATAW + CreatePopupMenu
    │                        # иконка + контекстное меню
    └── settings.rs          # egui: окно настроек (заглушка)
```

### 3.3. Потоки

| Поток | Назначение | Технология |
|---|---|---|
| **Main** | Event loop, state machine, dispatch | `crossbeam-channel::Receiver` |
| **Audio** | cpal callback (реальное время) | `cpal::Stream::play()` |
| **Whisper** | Whisper inference (блокирующий) | `std::thread::spawn` per request |
| **Hotkey** | WH_KEYBOARD_LL | Win32 SetWindowsHookExW |
| **Tray** | Win32 NotifyIcon + message pump | GetMessageW / DispatchMessageW |
| **Wake** | Wake word detection (small модель) | Отдельный поток захвата |
| **Accum** | PTT накопление аудио | mpsc channel |

### 3.4. Каналы (crossbeam-channel)

```rust
// Команды из UI/хоткеев в главный поток
let (cmd_tx, cmd_rx): (Sender<AppCommand>, Receiver<AppCommand>);

// Сырые аудио сэмплы в whisper worker
let (whisper_tx, whisper_rx): (Sender<Vec<f32>>, Receiver<Vec<f32>>);

// enum AppCommand {
//     StartRecording,
//     StopRecording,
//     ChangeMic { name, index },
//     ChangeModel(String),
//     ToggleGpu(bool),
//     ToggleVad(bool),
//     OpenSettings,
//     ReloadDictionary,
//     ReloadCommands,
//     ToggleMathMode(bool),
//     RecordingResult(String),
//     Quit,
// }
```

---

## 4. Поток обработки (основной сценарий)

### 4.1. Push-to-talk (Ctrl+Insert)

```
[Пользователь нажимает Ctrl+Insert]
      │
      ▼
WH_KEYBOARD_LL hook (поток hotkeys)
      │ cmd_tx.send(StartRecording)
      ▼
main::on_start()
      ├─ recording = true (атомик)
      └─ audio-accum поток копит чанки в Vec<f32>

[Пользователь держит Ctrl+Insert — говорит]

[Пользователь отпускает Insert]
      │
      ▼
WH_KEYBOARD_LL hook
      │ cmd_tx.send(StopRecording)
      ▼
main::on_stop()
      ├─ recording = false
      ├─ drain audio_buf → Vec<f32>
      └─ whisper_tx.send(samples)

      ▼ [whisper worker thread]
    whisper.transcribe(&samples)
      │ 48kHz → 16kHz ресемплинг
      │ WAV → temp file → whisper-cli
      │ fix_text(raw_text, config, &dict)
      │   ├─ remove_hallucinations
      │   ├─ merge_broken_words     ★
      │   ├─ apply_dictionary
      │   ├─ remove_repetitions
      │   └─ fix_punctuation
      │ cmd_tx.send(RecordingResult(text))
      ▼
main::on_result()
      ├─ try_execute(text) → команда?
      │   └─ да: выполнить (paste/hotkey/...)
      ├─ math_mode? → convert_math
      └─ inserter.insert_text(text)
           ├─ save clipboard
           ├─ copy_to_clipboard(final_text)
           ├─ send_ctrl_v()
           └─ restore clipboard
```

### 4.2. Wake Word режим

```
[Тишина] → 2-й поток захвата, кольцевой буфер 0.5 сек
[Звук] → whisper-small.detect(chunk)
  ├─ содержит "слушай" / "бро запиши" / "записывай"?
  │   └─ да → накопление до тишины → whisper-large → текст
  └─ нет → слушаем дальше
```

---

## 5. Text Fixer — детально

### 5.1. fix_text() orchestrator

```rust
pub fn fix_text(text: &str, config: &TextFixConfig, dict: &Dictionary) -> String {
    // 1. Удаление галлюцинаций
    // 2. Базовая очистка (множественные пробелы)
    // 3. СКЛЕЙКА РАЗОРВАННЫХ СЛОВ (главная фича)
    // 4. Встроенный словарь (питон → Python)
    // 5. Пользовательский словарь (из файла)
    // 6. Схлопывание повторов
    // 7. Пунктуация (капитализация, точка)
    // 8. Финальная очистка
}
```

### 5.2. Space Fixer — алгоритм склейки

```
Вход: ["произволь", "ных", "и", "их", "варианты"]
Выход: ["произвольных", "и", "их", "варианты"]

for each adjacent pair (w1, w2):
    merged = w1 + w2

    if merged in dictionary:
        if w1 NOT in dictionary OR w2 NOT in dictionary:
            MERGE (высокая уверенность)
        else:
            KEEP (оба слова валидны)

    else:
        if is_short_token(w1) AND w1 not in {"и","в","с","у","а","о","к","я"}:
            try SymSpell.lookup(merged)
            if found: MERGE

        if w1 ends_with_consonant AND w2 starts_with_vowel:
            try SymSpell.lookup(merged)
            if found: MERGE

        if w2 in COMMON_SUFFIXES {"ный","ная","ное","ные",...}:
            MERGE

        if w1 starts_with COMMON_PREFIX {"наи","само","взаимо",...}:
            MERGE
```

### 5.3. Словарь

- Файл: `assets/{lang}_words_utf8.txt`
- Формат: UTF-8, одно слово на строку
- Загрузка: при старте, `RwLock<HashSet<String>>`
- Переключение: `dict.load_lang("en")` — без перезапуска
- Источник: [danakt/russian-words](https://github.com/danakt/russian-words) (MIT) — исходно ~1.5M словоформ + ~877K фамилий в cp1251 (`assets/russian.txt`, `assets/russian_surnames.txt`). Из них отобрано ~200K наиболее употребительных. При необходимости можно загрузить полный словарь без потери производительности — HashSet поиск остаётся O(1).

---

## 6. GPU-поддержка

Используется pre-built `whisper-cli.exe` из `cu-bin-blas12.4`:
- CUDA 12.4 runtime
- GPU: NVIDIA RTX 3080 Ti (12 GB VRAM)
- Модели: детектор (small, 265 MB) + транскрайбер (large-v3, 3.1 GB)
- Обе модели в GPU одновременно (~3.4 GB из 12 GB)

CUDA DLL копируются в `target/debug/` при сборке через `build.rs`.

---

## 7. Win32 API (windows-sys)

Используется для:

| Функция | Win32 API | Назначение |
|---|---|---|
| Single instance | `CreateMutexW` | Защита от повторного запуска |
| Clipboard | `OpenClipboard`, `SetClipboardData`, `GetClipboardData`, `EmptyClipboard`, `CloseClipboard` | Работа с буфером обмена |
| Global memory | `GlobalAlloc`, `GlobalLock`, `GlobalUnlock`, `GlobalFree` | Управление памятью для clipboard |
| Window focus | `SetForegroundWindow`, `AttachThreadInput` | Фокус окна перед вставкой |
| Global hotkey | `SetWindowsHookExW(WH_KEYBOARD_LL)` | Ctrl+Insert перехват |
| Message pump | `GetMessageW`, `DispatchMessageW` | WH_KEYBOARD_LL + Tray |
| Tray icon | `Shell_NotifyIconW(NOTIFYICONDATAW)` | Системный трей |
| Tray menu | `CreatePopupMenu`, `TrackPopupMenu` | Контекстное меню |
| Smart spacing | `GetGUIThreadInfo`, `EM_GETSEL`, `WM_GETTEXT` | Символ слева от каретки |
| Key events | `SendInput` | Симуляция клавиш (Ctrl+V) |
| Custom icon | `CreateIcon` | Иконка из RGBA (PNG) |
| DPI | `SetProcessDPIAware` | Чёткая иконка в трее |

---

## 8. Settings Window (egui)

Окно настроек открывается по команде из трея. Пока заглушка — MessageBox.

### Секции (запланированы):

1. **Микрофон** — выпадающий список устройств (из cpal)
2. **Модель** — выпадающий список GGML-файлов
3. **GPU** — checkbox
4. **Кнопка записи** — выбор триггера (Ctrl+Insert / Middle / Right)
5. **VAD** — checkbox + агрессивность (0-3) + таймаут тишины
6. **Исправление текста** — checkbox'ы на каждый этап
7. **Команды** — checkbox включения
8. **Шумоподавление** — checkbox
9. **Логирование** — checkbox + путь к папке логов
10. **Математический режим** — checkbox
11. **Показывать результат** — checkbox

---

## 9. Команды и математический режим

### 9.1. Формат команд (JSON)

```json
{
  "открой браузер": {
    "triggers": {
      "ru": ["открой браузер"],
      "en": ["open browser"]
    },
    "type": "hotkey",
    "value": "ctrl+shift+b"
  },
  "сохранить файл": {
    "triggers": {"ru": ["сохрани", "сохранить"]},
    "type": "hotkey",
    "value": "ctrl+s"
  }
}
```

Типы: paste, hotkey, mouse_move, mouse_click, mouse_scroll, mouse_monitor, mouse_continuous, mouse_stop, focus_switch, focus_save, grid, grid_zoom, selection_more, selection_less, toggle_math_mode.

Защита от ложных срабатываний: если слов >= `command_max_words` (default 3) — диктовка, а не команда.

### 9.2. Математический режим

"два плюс три умножить на четыре" → "2 + 3 × 4"

Используется:
- Встроенный словарь числительных (один→1, два→2, ...)
- Операторы (плюс→+, умножить→×, разделить→÷)
- Scoped: работает только в `config.math_mode = true`

---

## 10. Требования к системе

| Параметр | Минимальные | Рекомендуемые |
|---|---|---|
| ОС | Windows 10 x64 | Windows 11 x64 |
| RAM | 2 GB (CPU, tiny model) | 8 GB (GPU, large) |
| GPU | Не требуется | NVIDIA с 4+ GB VRAM (RTX 3080 Ti 12GB) |
| Диск | 200 MB (приложение) + 75 MB (tiny модель) | 500 MB + 3 GB (large model) |
| CUDA | Не требуется | CUDA 12.4 runtime (DLL) |
| Процессор | 2 ядра | 4+ ядер |
| Микрофон | Любой | USB-микрофон |

---

## 11. Ограничения (известные)

1. **Только Windows** — из-за Win32 API. Порт на macOS/Linux через абстракцию — в backlog.
2. **Одноязычность** — первая версия только русский язык. Английский через `load_lang("en")`.
3. **whisper.cpp model** — требует предварительно скачанный GGML-файл.
4. **VAD не подключён** — детектор написан, но не интегрирован в пайплайн.
5. **Wake word** — код написан, требует `wake_mode: true` + модель-детектор.

---

## 12. HTTP API (план)

Запланированный модуль `api/server.rs` для интеграции с внешними программами.

### Принцип

```
POST /transcribe
Content-Type: audio/wav
Body: <WAV-файл (16kHz, mono, f32 или s16)>

→ 200 OK
{"text": "распознанный текст"}
```

### Конфиг

```json
{
  "api_port": 8765
}
```

Если `api_port` не задан (`null`) — сервер не запускается, программа работает в штатном PTT-режиме.

### Стек

- `tiny_http` — минимальная HTTP-библиотека (нет extern-зависимостей за её пределами)
- Эндпоинт: `POST /transcribe` — принимает WAV → `WhisperEngine::transcribe()` → `fix_text()` → JSON
- Поток: отдельный, использует `Arc<WhisperEngine>` и `Arc<Dictionary>` из `App`

### Применение

```bash
curl -X POST --data-binary @speech.wav http://localhost:8765/transcribe
```

Из Python:

```python
import requests
r = requests.post("http://localhost:8765/transcribe", data=open("speech.wav", "rb"))
print(r.json()["text"])
```

Не влияет на PTT-режим — работают параллельно.

## 13. Полезные ссылки

| Ресурс | Ссылка | Назначение |
|---|---|---|
| danakt/russian-words | [github.com/danakt/russian-words](https://github.com/danakt/russian-words) | Исходный словарь русских словоформ ~1.5M (MIT) |
| whisper.cpp релизы | [github.com/ggml-org/whisper.cpp/releases](https://github.com/ggml-org/whisper.cpp/releases) | Бинарник whisper-cli.exe (cu-bin-blas12.4) |
| GGML-модели Whisper | [huggingface.co/ggerganov/whisper.cpp](https://huggingface.co/ggerganov/whisper.cpp/tree/main) | GGML-файлы моделей (large-v3, small, tiny) |

## 13. Лицензия

GNU General Public License v3.0 (как и оригинальный VoxBee).
