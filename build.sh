#!/bin/bash
set -e

TARGET=thumbv7m-none-eabi
PROJECT_NAME=embedded  # replace with your binary name

echo "[*] Building project..."
cargo build --release --target $TARGET