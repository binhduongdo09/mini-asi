#!/bin/bash
# Chạy Mini-Asi ở chế độ phát triển (tự nạp lại khi sửa giao diện)
set -e
cd "$(dirname "$0")/.."
[ -d node_modules ] || npm ci
npx tauri dev
