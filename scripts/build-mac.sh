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

echo "⑤ Chứng chỉ ký…"
# Ký mọi bản bằng cùng một chứng chỉ (secrets/codesign/, không đưa lên git) → macOS giữ nguyên quyền đã cấp sau mỗi lần cập nhật.
# Lần đầu: nạp chứng chỉ vào Keychain đăng nhập. Không có chứng chỉ → ký tạm như cũ (phải cấp quyền lại sau khi cập nhật).
CS_DIR="secrets/codesign"
if [ -f "$CS_DIR/mini-asi-codesign.p12" ] && [ -f "$CS_DIR/mini-asi-codesign.pem" ]; then
  CS_ID="$(openssl x509 -in "$CS_DIR/mini-asi-codesign.pem" -noout -fingerprint -sha1 | cut -d= -f2 | tr -d ':')"
  if ! security find-identity -p codesigning | grep -q "$CS_ID"; then
    security import "$CS_DIR/mini-asi-codesign.p12" -P "$(cat "$CS_DIR/mini-asi-codesign-password.txt")" -T /usr/bin/codesign >/dev/null \
      && echo "   Đã nạp chứng chỉ Mini-Asi Code Signing vào Keychain"
  fi
  PROBE="$(mktemp -d)/probe"; cp /usr/bin/true "$PROBE"
  if codesign --force -s "$CS_ID" "$PROBE" 2>/dev/null; then
    export APPLE_SIGNING_IDENTITY="$CS_ID"
    echo "   Ký bằng chứng chỉ Mini-Asi Code Signing"
  else
    echo "   ⚠️  Không ký được bằng chứng chỉ (Keychain hỏi quyền? bấm Luôn cho phép rồi chạy lại) — ký tạm"
  fi
  rm -f "$PROBE"
else
  echo "   Không có $CS_DIR — ký tạm (sau mỗi lần cập nhật macOS sẽ hỏi lại quyền)"
fi

echo "⑥ Đóng gói…"
npx tauri build $TARGET_ARGS

DMG=$(ls $OUT_DIR/*.dmg | head -1)
echo ""
echo "✅ Xong! File cài đặt: $DMG"
echo "   Mở file .dmg → kéo Mini-Asi vào Applications."
echo "   Lần đầu mở: chuột phải vào Mini-Asi → Open (vì app chưa ký số Apple)."
open -R "$DMG"
