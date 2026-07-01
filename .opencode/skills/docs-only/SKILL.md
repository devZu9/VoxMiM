---
name: docs-only
description: Use ONLY when the task involves editing documentation files (.md, .txt). Работа с Markdown-документацией, CHANGELOG, README, ROADMAP, TECHNICAL_SPECIFICATION, summary, AGENTS. НЕ применять при работе с .rs, .toml, .json и другим исходным кодом.
---

# Docs Only — работа с документацией

## Когда применять

Только если задача **ограничена** файлами документации:
- `*.md` (README, CHANGELOG, ROADMAP, TECHNICAL_SPECIFICATION, summary, AGENTS)
- `*.txt` (заметки, списки, ассеты)

НЕ применять, если в задаче есть хотя бы один `.rs`, `.toml`, `.json`, `.rc`, `.css`, `.html` и т.п.

## Правила

1. **Не собирать проект.** `cargo build`, `cargo test`, `cargo check` — не запускать.
2. **Не килять процессы.** Не трогать voxmim.exe или другие запущенные бинарники.
3. **Не перезапускать.** После редактирования не требуется запуск/перезапуск программы.
4. Если нужен commit — только `git add` → `git commit` → `git push` (без шагов сборки).
5. Кодировка — UTF-8 без BOM.
6. Не генерировать README-файлы и документацию, если пользователь явно не попросил.

## Примеры

✅ Задача: «Добавь ссылку в README» → только README.md → commit → готово.
✅ Задача: «Обнови CHANGELOG и summary» → два .md файла → commit → готово.
✅ Задача: «Запиши идею в roadmap и ТЗ» → ROADMAP.md + TECHNICAL_SPECIFICATION.md → commit → готово.

❌ Задача: «Добавь ссылку в README и поправь функцию в app.rs» → НЕ docs-only, нужна сборка.
❌ Задача: «Напиши новый модуль» → НЕ docs-only.
