# -*- coding: utf-8 -*-
"""Скриншоты состояний иконки трея VoxMiM с таймингом по session.log.

Клавишу (Ctrl+Insert) нажимаешь вручную: VoxMiM фильтрует инжектированные
события клавиатуры (LLKHF_INJECTED, защита с v0.4.0). Скрипт снимает
иконку непрерывно, а момент состояния берёт из лога приложения.

Режимы:
  capture  — непрерывная съёмка иконки (~70мс/кадр) в tests/tray_snapshots/seq/
  analyze  — по session.log находит окна состояний и сохраняет
             recording.png / loading.png из ближайших кадров

Запуск:
    python scripts/tray_smoke/capture_states.py capture --seconds 45
    python scripts/tray_smoke/capture_states.py analyze --exe-dir <папка с exe>

Требуется: pip install pywinauto pillow
"""

import argparse
import pathlib
import re
import sys
import time
from datetime import datetime

from PIL import ImageGrab

from test_tray import APP_NAME, find_tray_icon

TS_RE = re.compile(r"\[(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3})")


def parse_ts(text):
    """Строка из лога → эпоха (сек)."""
    return datetime.strptime(text, "%Y-%m-%dT%H:%M:%S.%f").timestamp()


def parse_frame_ts(name):
    """Имя кадра (frame_)YYYYMMDD_HHMMSS_mmm.png → эпоха (сек)."""
    text = name[:-len(".png")]
    if text.startswith("frame_"):
        text = text[len("frame_"):]
    return datetime.strptime(text, "%Y%m%d_%H%M%S_%f").timestamp()


def epoch_to_name(epoch):
    return "frame_" + datetime.fromtimestamp(epoch).strftime("%Y%m%d_%H%M%S_%f")[:-3] + ".png"


def capture(args):
    outdir = pathlib.Path(args.seq_dir)
    outdir.mkdir(parents=True, exist_ok=True)
    print(f"Съёмка {args.seconds}с в {outdir} (клавишу жми вручную)...")
    icon = find_tray_icon(args.icon_timeout)
    if icon is None:
        print(f"FAIL: иконка «{APP_NAME}» не появилась за {args.icon_timeout}с")
        sys.exit(1)
    rect = icon.rectangle()
    deadline = time.time() + args.seconds
    count = 0
    while time.time() < deadline:
        img = ImageGrab.grab(bbox=(rect.left, rect.top, rect.right, rect.bottom)).convert("RGB")
        img.save(outdir / epoch_to_name(time.time()))
        count += 1
        time.sleep(0.07)
    print(f"OK: снято {count} кадров")


def find_log(exe_dir):
    base = pathlib.Path(exe_dir)
    for candidate in (base / "logs" / "session.log", base / "session.log"):
        if candidate.exists():
            return candidate
    return None


def analyze(args):
    log = find_log(args.exe_dir)
    if log is None:
        print(f"FAIL: session.log не найден в {args.exe_dir}")
        sys.exit(1)
    lines = open(log, encoding="utf-8").read().splitlines()

    events = []
    for ln in lines:
        m = TS_RE.search(ln)
        if m:
            events.append((parse_ts(m.group(1)), ln[m.end() :]))

    starts = [e for e in events if "Запись началась" in e[1]]
    if not starts:
        print("FAIL: в логе нет «Запись началась» — записи не было")
        sys.exit(1)
    t0, line0 = starts[-1]
    after = [e for e in events if e[0] > t0]

    end_rec = next((e for e in after if "Записано" in e[1]), None)
    wav = next((e for e in after if "WAV:" in e[1]), None)
    result = next((e for e in after if "📝" in e[1]), None)

    if end_rec is None:
        print("FAIL: нет строки «Записано» после старта записи")
        sys.exit(1)
    t1 = end_rec[0]
    t2 = wav[0] if wav else None
    t3 = result[0] if result and (t2 is None or result[0] > t2) else None

    print(f"окна: запись [{t0:.1f} .. {t1:.1f}]"
          + (f", распознавание [{t2:.1f} .. {t3:.1f}]" if t2 else ", LOADING не найден"))

    frames = sorted(pathlib.Path(args.seq_dir).glob("*.png"))
    if not frames:
        print(f"FAIL: в {args.seq_dir} нет кадров — сначала запусти capture")
        sys.exit(1)

    def nearest(epoch):
        best, best_d = None, 1e9
        for f in frames:
            ts = parse_frame_ts(f.name)
            d = abs(ts - epoch)
            if d < best_d:
                best_d, best = d, f
        assert best is not None
        return best, best_d

    outdir = pathlib.Path(args.outdir)
    outdir.mkdir(parents=True, exist_ok=True)

    target_rec = t0 + min(1.0, (t1 - t0) / 2)
    rec_frame, d = nearest(target_rec)
    img = ImageGrab_None(rec_frame)
    img.save(outdir / "recording.png")
    print(f"OK: recording.png из кадра {rec_frame.name} (отклонение {d*1000:.0f}мс)")

    if t2 and t3:
        win = t3 - t2
        target_load = t2 + max(0.1, win / 2)
        load_frame, d = nearest(target_load)
        ImageGrab_None(load_frame).save(outdir / "loading.png")
        print(f"OK: loading.png из кадра {load_frame.name} (отклонение {d*1000:.0f}мс)")
    else:
        print("WARN: LOADING не зафиксирован (нет WAV/результата) — loading.png не создан")


def ImageGrab_None(path):
    from PIL import Image

    return Image.open(path)


def find_hourglass(args):
    """Ищет кадр песочных часов (макс. ярких пикселей) — старт/восстановление."""
    seq = pathlib.Path(args.seq_dir)
    frames = sorted(seq.glob("*.png"))
    if not frames:
        print(f"FAIL: в {args.seq_dir} нет кадров — сначала запусти capture")
        sys.exit(1)
    best, best_n = None, 0
    for f in frames:
        im = ImageGrab_None(f).convert("RGB")
        n = sum(1 for y in range(im.height) for x in range(im.width)
                if im.getpixel((x, y))[2] > 100)
        if n > best_n:
            best_n, best = n, f
    if best is None or best_n == 0:
        print("WARN: песочные часы не найдены (все кадры пустые)")
        sys.exit(1)
    outdir = pathlib.Path(args.outdir)
    outdir.mkdir(parents=True, exist_ok=True)
    ImageGrab_None(best).save(outdir / "loading.png")
    print(f"OK: loading.png из кадра {best.name} (ярких пикселей: {best_n})")


def main():
    parser = argparse.ArgumentParser(description="Скриншоты состояний иконки трея VoxMiM")
    sub = parser.add_subparsers(dest="mode", required=True)

    p_cap = sub.add_parser("capture", help="непрерывная съёмка иконки")
    p_cap.add_argument("--seconds", type=float, default=45.0)
    p_cap.add_argument("--seq-dir", default="tests/tray_snapshots/seq")
    p_cap.add_argument("--icon-timeout", type=int, default=20)
    p_cap.set_defaults(func=capture)

    p_an = sub.add_parser("analyze", help="выемка кадров по session.log")
    p_an.add_argument("--exe-dir", default="target/debug", help="папка с exe (где logs/session.log)")
    p_an.add_argument("--seq-dir", default="tests/tray_snapshots/seq")
    p_an.add_argument("--outdir", default="tests/tray_snapshots")
    p_an.set_defaults(func=analyze)

    p_hg = sub.add_parser("hourglass", help="выемка песочных часов (старт/восстановление)")
    p_hg.add_argument("--seq-dir", default="tests/tray_snapshots/seq")
    p_hg.add_argument("--outdir", default="tests/tray_snapshots")
    p_hg.set_defaults(func=find_hourglass)

    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
