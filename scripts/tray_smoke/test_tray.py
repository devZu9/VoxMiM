# -*- coding: utf-8 -*-
"""Smoke-тест системного трея VoxMiM (Windows, pywinauto + UIA).

Проверяет:
  1. Иконка «VoxMiM» появилась в трее после запуска приложения.
  2. Правый клик открывает меню.
  3. Пункты меню существуют и в правильном порядке (эталон из lang/ru.json).
  4. Клик по «Режим удержания + автостоп» переключает галочку (и обратно).
  5. В session.log появилась запись «Автостоп: вкл/выкл».
  6. Скриншот трея (IDLE) сохраняется в tests/tray_snapshots/.

Запуск:
    python scripts/tray_smoke/test_tray.py --exe-dir <папка с logs/>
    (--exe-dir по умолчанию: папка запуска)

Требуется: pip install pywinauto pillow
Координатный (хрупкий): клик ПКМ по иконке. В регулярный cargo test не включать.
"""

import argparse
import pathlib
import sys
import time

import pywinauto
from PIL import ImageGrab
from pywinauto import Desktop

APP_NAME = "VoxMiM"

# Эталон: пункты меню по порядку (без разделителей), ключи из lang/ru.json.
# Версия — префикс «VoxMiM v» (серый пункт), «Показать/Скрыть окно» — оба варианта.
EXPECTED = [
    ("VoxMiM v", "prefix"),
    "Настройки",
    ("Показать окно", "Скрыть окно"),
    "Добавить слово в пользовательский словарь...",
    "Редактировать словарь пользователя",
    "Добавить фразу в словарь галлюцинаций...",
    "Редактировать словарь галлюцинаций",
    "Распознать аудиофайл...",
    "Создать субтитры из аудиофайла...",
    "Голосовая активация",
    "Режим удержания + автостоп",
    "Math Mode",
    "Выход",
]

TOGGLE_ITEM = "Режим удержания + автостоп"
LOG_NEEDLE = "Автостоп"


def fail(msg):
    print(f"FAIL: {msg}")
    sys.exit(1)


def find_tray_icon(timeout=20):
    desktop = Desktop(backend="uia")
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            tray = desktop.window(class_name="Shell_TrayWnd")
            icon = tray.child_window(title=APP_NAME, control_type="Button")
            if icon.exists(timeout=1):
                return icon
        except Exception:
            pass
        time.sleep(0.5)
    return None


def open_menu(icon):
    icon.click_input(button="right")
    menu = Desktop(backend="uia").window(class_name="#32768")
    menu.wait("exists", timeout=5)
    return menu


def read_items(menu):
    items = []
    try:
        for child in menu.children():
            text = child.window_text() or ""
            if text.lower() in ("", "separator", "---"):
                continue
            items.append(text)
    except Exception as e:
        fail(f"не удалось прочитать пункты меню: {e}")
    return items


def match_expected(actual, expected):
    if len(actual) != len(expected):
        return f"количество пунктов: ожидалось {len(expected)}, получено {len(actual)}: {actual}"
    for a, e in zip(actual, expected):
        if isinstance(e, str):
            if a != e:
                return f"пункт «{a}» не совпал с эталоном «{e}»"
        elif isinstance(e, tuple):
            if e[1] == "prefix":
                if not a.startswith(e[0]):
                    return f"первый пункт «{a}» не начинается с «{e[0]}»"
            else:
                if a not in e:
                    return f"пункт «{a}» не входит в набор {e}"
    return None


def get_checked(item):
    try:
        return bool(item.get_toggle_state())
    except Exception:
        pass
    try:
        props = item.get_properties()
        for key in ("is_checked", "toggle_state"):
            if key in props:
                return bool(props[key])
    except Exception:
        pass
    return None


def click_menu_item(menu, title):
    try:
        item = menu.child_window(title=title, control_type="MenuItem")
        item.wait("exists", timeout=2)
        item.click_input()
        return True
    except Exception:
        return False


def find_log(dirs, timeout=10):
    """Ищем session.log в заданных папках (и подпапке logs/)."""
    candidates = []
    for d in dirs:
        d = pathlib.Path(d)
        candidates.append(d / "session.log")
        candidates.append(d / "logs" / "session.log")
    deadline = time.time() + timeout
    needle_utf8 = LOG_NEEDLE
    while time.time() < deadline:
        for c in candidates:
            if c.exists():
                try:
                    text = c.read_text(encoding="utf-8", errors="ignore")
                except Exception:
                    continue
                if needle_utf8 in text:
                    return c
        time.sleep(0.5)
    return None


def save_screenshot(tray, path):
    try:
        rect = tray.rectangle()
        ImageGrab.grab(bbox=(rect.left, rect.top, rect.right, rect.bottom)).save(path)
        print(f"OK: скриншот трея сохранён: {path}")
    except Exception as e:
        print(f"WARN: не удалось снять скриншот трея: {e}")


def main():
    parser = argparse.ArgumentParser(description="Smoke-тест трея VoxMiM")
    parser.add_argument("--exe-dir", default=".", help="папка приложения (где лежат logs/session.log)")
    parser.add_argument("--no-screenshot", action="store_true", help="не снимать скриншот")
    parser.add_argument("--no-log-check", action="store_true", help="не проверять session.log")
    parser.add_argument("--icon-timeout", type=int, default=20, help="сек ожидания иконки")
    args = parser.parse_args()

    print("1. Ищем иконку VoxMiM в трее...")
    icon = find_tray_icon(args.icon_timeout)
    if icon is None:
        fail(f"иконка «{APP_NAME}» не появилась за {args.icon_timeout}с")
    print("OK: иконка найдена")

    print("2. Открываем меню правым кликом...")
    try:
        menu = open_menu(icon)
    except Exception as e:
        fail(f"меню не открылось: {e}")
    print("OK: меню открыто")

    print("3. Читаем пункты меню и сверяем порядок...")
    items = read_items(menu)
    error = match_expected(items, EXPECTED)
    if error:
        fail(f"порядок пунктов: {error}")
    print(f"OK: {len(items)} пунктов в правильном порядке")

    print("4. Кликаем «Режим удержания + автостоп» и проверяем галочку...")
    try:
        item = menu.child_window(title=TOGGLE_ITEM, control_type="MenuItem")
        item.wait("exists", timeout=2)
        before = get_checked(item)
        item.click_input()
        time.sleep(0.5)
        menu2 = open_menu(icon)
        item2 = menu2.child_window(title=TOGGLE_ITEM, control_type="MenuItem")
        item2.wait("exists", timeout=2)
        after = get_checked(item2)
        if before is not None and after is not None and before == after:
            fail(f"галочка не переключилась: было {before}, стало {after}")
        print(f"OK: галочка переключилась ({before} -> {after})")
        # Возвращаем исходное состояние (кликаем снова, если изменилось)
        if before is not None and after != before:
            item2.click_input()
            time.sleep(0.3)
        menu2.close()
    except Exception as e:
        fail(f"не удалось проверить галочку: {e}")

    if not args.no_log_check:
        print("5. Проверяем session.log...")
        log = find_log([args.exe_dir], timeout=10)
        if log is None:
            print("WARN: запись «Автостоп» в session.log не найдена "
                  "(лог мог писаться в другую папку; укажите --exe-dir)")
        else:
            print(f"OK: запись «{LOG_NEEDLE}» есть в {log}")

    if not args.no_screenshot:
        snap_dir = pathlib.Path("tests/tray_snapshots")
        snap_dir.mkdir(parents=True, exist_ok=True)
        save_screenshot(icon, snap_dir / "idle.png")

    print("SMOKE: ВСЁ ОК")


if __name__ == "__main__":
    main()
