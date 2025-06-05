#!/bin/bash
set -e

TARGET=thumbv7m-none-eabi
PROJECT_NAME=embedded  # replace with your binary name

echo "[*] Building project..."
cargo build --release --target $TARGET

echo "[*] Running on QEMU..."
qemu-system-arm \
    -cpu cortex-m3 \
    -machine lm3s6965evb \
    -nographic \
    -semihosting-config enable=on,target=native \
    -kernel target/$TARGET/release/$PROJECT_NAME
