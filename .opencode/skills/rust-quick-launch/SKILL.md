---
name: rust-quick-launch
description: Создание bat-файла с прекомпиляцией и запуском Rust проекта. Применять при начальной настройке проекта.
---

# Rust — Быстрый запуск

Создать `run.bat` в корне проекта:

```batch
@echo off
chcp 65001 >nul 2>&1
cd /d "%~dp0"

echo [1/2] Sborka...
cargo build
if %errorlevel% neq 0 (
    echo [ERROR] Sborka ne udalas!
    pause
    exit /b 1
)

echo.
echo Zapusk...
target\debug\project.exe
pause
```

## Важно
- **Транслитерация:** все русские сообщения в bat-файле писать латиницей (Sborka, Zapusk, ne udalas), а не кириллицей — Windows cmd не отображает русские буквы корректно.
- Для правил кодировки и UTF-8 загрузи `rust-encoding`.
