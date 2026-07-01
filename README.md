# VoxMiM

**Голосовой ввод текста на Rust.** Порт [VoxBee](https://github.com/boris-agent007/Voxbee) с исправлением ключевой ошибки: произвольные пробелы внутри распознанных слов.

## Возможности

- **Push-to-talk:** Ctrl+Insert → говорите → отпустите → текст в активном окне
- **Wake Word (опционально):** «Слушай», «Бро, запиши» — голосовая активация без кнопки
- **Распознавание:** Whisper large-v3 через CUDA 12.4 (NVIDIA GPU)
- **Склейка слов:** словарь 200K+ русских слов + эвристики + SymSpell
- **Голосовые команды:** 199 команд из VoxBee (paste, hotkey, mouse, scroll, grid)
- **Математический режим:** «два плюс три» → `2 + 3`
- **Smart Spacing:** автопробел перед вставкой
- **Буфер обмена:** сохраняется и восстанавливается после вставки

## Скриншот

_Скоро будет._

## Системные требования

| Компонент | Минимум | Рекомендуется |
|---|---|---|
| ОС | Windows 10 x64 | Windows 11 x64 |
| GPU | Не требуется | NVIDIA с 4+ GB VRAM |
| RAM | 2 GB | 8 GB |
| Модель Whisper | GGML (75 MB tiny) | GGML large-v3 (3.1 GB) |
| CUDA | — | CUDA 12.4 runtime |
| Микрофон | Любой | USB-микрофон |

## Быстрый старт

```bash
# Сборка
cargo build --release

# Запуск
target\release\voxmim.exe
```

Нажмите **Ctrl+Insert** → говорите → отпустите → текст появится в активном окне.

### Конфигурация

Первый запуск создаёт `%APPDATA%\voxmim\VoxMiM\config\config.json` с настройками по умолчанию. Если есть VoxBee — конфиг мигрируется автоматически.

Основные поля:

```json
{
  "mic_name": "Микрофон (USB PnP Audio Device)",
  "model_path": "C:\\_workPortable\\WhisperCpp\\models\\ggml-large-v3-turbo-q8_0.bin",
  "use_gpu": true,
  "trigger": { "button": "Keyboard", "keyboard": "ctrl+insert" },
  "wake_mode": false,
  "wake_words": ["слушай", "бро запиши", "записывай"],
  "language": "ru",
  "vad": { "enabled": false, "aggressiveness": 1 },
  "text_fix": { "fix_spaces": true, "fix_hallucinations": true },
  "command_max_words": 3
}
```

## Установка моделей

Скачайте GGML-модели с [huggingface.co/ggerganov/whisper.cpp](https://huggingface.co/ggerganov/whisper.cpp/tree/main):

- `ggml-large-v3-russian.bin` — русская модель (3.1 GB)
- `ggml-large-v3-turbo-q8_0.bin` — быстрая (874 MB)
- `ggml-small-q8_0.bin` — для детектора wake word (265 MB)

Положите в папку и укажите путь в `config.json`.

### Словари

Словарь русских словоформ (~200K записей) получен из репозитория [danakt/russian-words](https://github.com/danakt/russian-words) (исходно ~1.5M словоформ, лицензия MIT). Файлы `assets/russian.txt` и `assets/russian_surnames.txt` — оригиналы в cp1251. При необходимости можно использовать полный словарь (все 1.5M+) — пока работает и на выборке 200K.

## Архитектура

```
Потоки: main | audio-accum | whisper | hotkey | tray | wake
Каналы: crossbeam-channel
Сборка: cargo (без LIBCLANG_PATH, без bindgen)
CUDA: cu-bin-blas12.4 (pre-built DLL)
Трей: Win32 NOTIFYICONDATAW + CreatePopupMenu
Хоткей: WH_KEYBOARD_LL (без прав администратора)
```

### whisper.cpp

VoxMiM использует:
- **Бинарник `whisper-cli.exe`** — загружается автоматически из [релизов whisper.cpp](https://github.com/ggml-org/whisper.cpp/releases) (cu-bin-blas12.4 для CUDA 12.4)
- **GGML-модели** — скачиваются вручную с [huggingface.co/ggerganov/whisper.cpp](https://huggingface.co/ggerganov/whisper.cpp/tree/main)

## Полезные ссылки

| Ресурс | Ссылка |
|---|---|
| Словарь русских слов | [danakt/russian-words](https://github.com/danakt/russian-words) |
| whisper.cpp релизы (бинарник) | [github.com/ggml-org/whisper.cpp/releases](https://github.com/ggml-org/whisper.cpp/releases) |
| GGML-модели Whisper | [huggingface.co/ggerganov/whisper.cpp](https://huggingface.co/ggerganov/whisper.cpp/tree/main) |

## Лицензия

GNU General Public License v3.0
