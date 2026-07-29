#!/bin/bash
set -e
cd "$(dirname "$0")/.."
cargo build --release
./target/release/voxmim
