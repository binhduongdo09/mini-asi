#!/bin/bash
# Đóng gói Mini-Asi trên chính máy Mac của bạn → file .dmg
# Cách chạy: mở Terminal, kéo thư mục mini-asi vào, rồi gõ:  bash scripts/build-mac.sh
set -e
cd "$(dirname "$0")/.."

echo "① Kiểm tra Xcode Command Line Tools…"
xcode-select -p >/dev/null 2>&1 || { xcode-select --install; echo "→ Cài xong Command Line Tools rồi chạy lại script."; exit 1; }

echo "② Kiểm tra Rust…"
[ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env"
if ! command -v cargo >/dev/null; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  source "$HOME/.cargo/env"
fi
cargo --version
# Có rustup → đóng gói bản chạy cả chip Apple lẫn Intel; không có (Rust cài qua Homebrew) → bản cho chip của máy này
TARGET_ARGS=""
OUT_DIR="target/release/bundle/dmg"
if command -v rustup >/dev/null; then
  rustup target add aarch64-apple-darwin x86_64-apple-darwin && TARGET_ARGS="--target universal-apple-darwin" && OUT_DIR="target/universal-apple-darwin/release/bundle/dmg"
fi

echo "③ Kiểm tra Node.js…"
command -v npm >/dev/null || { echo "→ Cài Node.js 20+ (https://nodejs.org hoặc: brew install node) rồi chạy lại."; exit 1; }
npm ci

echo "④ Chạy kiểm thử…"
cargo test --workspace -q

echo "⑤ Đóng gói…"
npx tauri build $TARGET_ARGS

DMG=$(ls $OUT_DIR/*.dmg | head -1)
echo ""
echo "✅ Xong! File cài đặt: $DMG"
echo "   Mở file .dmg → kéo Mini-Asi vào Applications."
echo "   Lần đầu mở: chuột phải vào Mini-Asi → Open (vì app chưa ký số Apple)."
open -R "$DMG"
