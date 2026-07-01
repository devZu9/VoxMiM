---
name: rust-versioning
description: Версионирование Rust проекта. Применять при работе с версией, создании Cargo.toml, добавлении метаданных.
---

# Rust — Версия — один источник истины

- Номер версии задаётся **только** в `Cargo.toml` (`workspace.package.version` или `package.version`).
- В коде использовать через `env!("CARGO_PKG_VERSION")` или крейт `vergen`.
- Никаких хардкодов версии в исходниках.
