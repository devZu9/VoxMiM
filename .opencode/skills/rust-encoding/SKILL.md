---
name: rust-encoding
description: Кодировка UTF-8, работа с датой-временем в Rust проекте. Применять при создании файлов, настройке обмена данными, работе с JSON.
---

# Rust — Кодировка и протокол

## Кодировка
- Весь обмен данными между Rust и текстовыми файлами (включая python, bat (cmd), ps1, json) — только **UTF-8 без BOM**.
- `sys.stdin.buffer` читать как `raw_line.decode("utf-8")`.
- `stdout.buffer.write()` с `json.dumps(..., ensure_ascii=False).encode("utf-8")`.
- ❌ Никогда не полагаться на `sys.stdin.encoding` по умолчанию (на русской Windows это cp1251).

## Дата и время
- При работе с датой-временем (кэш, логи, файлы) использовать **локальное время**: `chrono::Local::now()` вместо `Utc::now()`.

