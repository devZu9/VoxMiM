#!/bin/bash
set -e
cd "$(dirname "$0")/.."

# Бэкап config.json перед сборкой
SAVED_CONFIG=$(mktemp)
if [ -f target/release/config.json ]; then
    cp target/release/config.json "$SAVED_CONFIG"
fi

cargo build --release

# Восстановление config.json после сборки
if [ -f "$SAVED_CONFIG" ]; then
    mkdir -p target/release/
    cp "$SAVED_CONFIG" target/release/config.json
    rm "$SAVED_CONFIG"
fi

./target/release/voxmim
