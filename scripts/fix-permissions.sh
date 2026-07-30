#!/bin/bash

PROJECT="$(cd "$(dirname "$0")/.." && pwd)"
BINARY="$PROJECT/target/release/voxmim"
APP="$PROJECT/VoxMiM.app"

echo "=== VoxMiM: Настройка прав macOS ==="
echo ""

if [ ! -f "$BINARY" ]; then
    echo "Сначала собери проект: bash scripts/make-app.sh"
    echo "Файл $BINARY не найден."
    exit 1
fi

echo "VoxMiM использует shell-скрипт, который запускает бинарник:"
echo "  $BINARY"
echo ""
echo "Добавь в разрешения именно ЭТОТ файл, а не VoxMiM.app:"
echo ""
echo "  Шаг 1: Системные настройки → Конфиденциальность → Универсальный доступ"
echo "  Шаг 2: Нажми '+' (плюс)"
echo "  Шаг 3: Нажни Cmd+Shift+G и вставь:"
echo "    $BINARY"
echo "  Шаг 4: Открыть → поставить галочку"
echo ""
echo "  Шаг 5: Системные настройки → Конфиденциальность → Мониторинг ввода"
echo "  Шаг 6: Нажми '+' → Cmd+Shift+G → вставь тот же путь → Открыть → галочка"
echo ""
echo "  ИЛИ просто открой папку и перетащи файл мышкой в окно настроек:"
echo "    open '$PROJECT/target/release/'"
echo ""
echo "После этого VoxMiM можно запускать из Dock."
echo ""
