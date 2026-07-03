# VoxMiM

[Русская версия](README.md)

> This project was initially made for personal use to learn and gain experience in vibe-coding using [OpenCode](https://opencode.ai/go?ref=DHSKBMGTK0)

**Voice typing on Rust.** Inspired by [VoxBee](https://github.com/boris-agent007/Voxbee). The key improvement — fixing broken/spaced words inside recognized text.

## Features

- **Push-to-talk:** Ctrl+Insert → speak → release → text in active window
- **Wake Word (optional):** "listen", "bro record" — voice activation without button
- **VAD (Auto-Stop):** tap Insert once → speak freely → silence → auto-submit
- **Multilingual UI:** RU/EN, switch in `config.json` (`language`)
- **Recognition:** Whisper large-v3 via CUDA 12.4 (NVIDIA GPU)
- **Word merging:** dictionary 200K+ Russian words + heuristics + SymSpell
- **Voice commands:** 199 commands (paste, hotkey, mouse, scroll, grid)
- **Math mode:** "two plus three" → `2 + 3`
- **User dictionary:** custom phrase replacements via tray dialog or JSON
- **Smart Spacing:** auto-space before insertion
- **Clipboard:** preserved and restored after paste

## Screenshot

_Coming soon._

## System requirements

| Component | Minimum | Recommended |
|---|---|---|
| OS | Windows 10 x64 | Windows 11 x64 |
| GPU | Not required | NVIDIA with 4+ GB VRAM |
| RAM | 2 GB | 8 GB |
| Whisper model | GGML (75 MB tiny) | GGML large-v3 (3.1 GB) |
| CUDA | — | CUDA 12.4 runtime |
| Microphone | Any | USB microphone |

## Quick start

```bash
# Build
cargo build --release

# Run
target\release\voxmim.exe
```

Press **Ctrl+Insert** → speak → release → text appears in active window.

### Configuration

First run creates `config.json` next to the `.exe`.

Main fields:

```json
{
  "mic_name": "Microphone (USB PnP Audio Device)",
  "model_path": "models\\ggml-large-v3-turbo-q8_0.bin",
  "use_gpu": true,
  "trigger": { "button": "Keyboard", "keyboard": "ctrl+insert" },
  "wake_mode": false,
  "wake_words": ["listen", "bro record"],
  "language": "ru",
  "vad": { "enabled": false, "aggressiveness": 1 },
  "text_fix": { "fix_spaces": true, "fix_hallucinations": true },
  "command_max_words": 3
}
```

## Installing models

Download GGML models from [huggingface.co/ggerganov/whisper.cpp](https://huggingface.co/ggerganov/whisper.cpp/tree/main):

- `ggml-large-v3-russian.bin` — Russian model (3.1 GB)
- `ggml-large-v3-turbo-q8_0.bin` — fast model (874 MB)
- `ggml-small-q8_0.bin` — wake word detector (265 MB)

Place them in a folder and set the path in `config.json`.

### Dictionaries

Russian word forms (~200K entries) from [danakt/russian-words](https://github.com/danakt/russian-words) (originally ~1.5M word forms, MIT license). Files `assets/russian.txt` and `assets/russian_surnames.txt` are originals in cp1251.

## Architecture

```
Threads: main | audio-accum | whisper | hotkey | tray | wake
Channels: crossbeam-channel
Build: cargo (no LIBCLANG_PATH, no bindgen)
CUDA: cu-bin-blas12.4 (pre-built DLL)
Tray: Win32 NOTIFYICONDATAW + CreatePopupMenu
Hotkey: WH_KEYBOARD_LL (no admin rights)
```

### whisper.cpp

VoxMiM uses:
- **`whisper-cli.exe` binary** — auto-downloaded from [whisper.cpp releases](https://github.com/ggml-org/whisper.cpp/releases) (cu-bin-blas12.4 for CUDA 12.4)
- **GGML models** — manually downloaded from [huggingface.co/ggerganov/whisper.cpp](https://huggingface.co/ggerganov/whisper.cpp/tree/main)

## Useful links

| Resource | Link |
|---|---|
| Russian words | [danakt/russian-words](https://github.com/danakt/russian-words) |
| whisper.cpp releases (binary) | [github.com/ggml-org/whisper.cpp/releases](https://github.com/ggml-org/whisper.cpp/releases) |
| GGML Whisper models | [huggingface.co/ggerganov/whisper.cpp](https://huggingface.co/ggerganov/whisper.cpp/tree/main) |

## License

MIT
