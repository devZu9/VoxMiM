#!/bin/bash
set -e

echo "=== VoxMiM: Настройка прав macOS ==="
echo ""
echo "VoxMiM нужны два разрешения macOS:"
echo ""
echo "  1. Универсальный доступ (Accessibility)"
echo "     → Системные настройки → Конфиденциальность → Универсальный доступ"
echo "     → Добавьте Terminal.app (или VoxMiM.app)"
echo ""
echo "  2. Мониторинг ввода (Input Monitoring) — macOS 15+"
echo "     → Системные настройки → Конфиденциальность → Мониторинг ввода"
echo "     → Добавьте Terminal.app (или VoxMiM.app)"
echo ""
echo "Примечание: TCC-level permission для бинарника по пути:"
BIN_PATH=$(cd "$(dirname "$0")/.." && pwd)/target/release/voxmim
if [ -f "$BIN_PATH" ]; then
    echo "  $BIN_PATH"
    echo "  (Это может облегчить добавление — перетащите файл в окно настроек)"
    echo ""
    echo "Можно открыть панель настроек одной командой:"
    echo "  open 'x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility'"
fi
echo ""
echo "После предоставления прав — перезапустите VoxMiM."
echo ""
