---
name: rust-github
description: Работа с GitHub в Rust проекте. Применять при пуше, создании релизов, работе с токеном и публикацией кириллицы.
---

# Rust — GitHub

- **Пуш на GitHub только после явного подтверждения пользователя.** Никогда не пушить без разрешения.
- Напомнить о пуше после каждых 5 изменённых файлов. Если изменений меньше 5, но пользователь говорит «готово», «заканчиваем», «на сегодня хватит» — тоже спросить про пуш.
- Токен: `%USERPROFILE%\.github_token` (ограничен по времени). Токен — Personal Access Token без `read:org`.

## Работа с токеном

**НЕ ИСПОЛЬЗОВАТЬ `gh auth login --with-token`** — токен не имеет scopes `read:org`, `gh` отклоняет его и запускает интерактивный device-flow (браузер).

### Правильные способы:

**Push через git:**
```bash
git remote add origin "https://USERNAME:TOKEN@github.com/USERNAME/REPO.git"
git push
git remote set-url origin "https://github.com/USERNAME/REPO.git"  # убрать токен из URL
```

**GitHub API через curl:**
```bash
curl -s -H "Authorization: token TOKEN" https://api.github.com/...
```

**JSON Body для API — через файл (избегать передачи кириллицы в shell):**
```python
import json
body = json.dumps({"key": "русский текст"}).encode("utf-8")
```
Или через Unicode escape в JSON:
```json
{"key": "\u0440\u0443\u0441\u0441\u043A\u0438\u0439"}
```

### Запрещено:
- ❌ `gh auth login --with-token` — триггерит браузерную авторизацию
- ❌ `Invoke-RestMethod` (PowerShell) — кодирует тело в cp1251, кириллица превращается в кракозябры
- ❌ Передавать кириллицу в аргументах командной строки — PowerShell/CMD искажают кодировку

## Релизы

- ✅ Для публикации релизов с кириллицей — `curl` с JSON из файла (UTF-8 без BOM)
- ✅ Тело релиза в отдельном файле, UTF-8 без BOM, флаг `--notes-file` для `gh release create` (если `gh` авторизован отдельно)
