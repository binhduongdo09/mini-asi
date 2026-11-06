#!/bin/bash
# Nhấp đúp file này để tạo file cài đặt Mini-Asi (.dmg) trên máy Mac của bạn.
# Lần đầu macOS có thể chặn: chuột phải vào file → Open → Open.
cd "$(dirname "$0")"
echo "🐰 Mini-Asi — đang tạo file cài đặt .dmg (lần đầu mất khoảng 5–10 phút)…"
echo ""
bash scripts/build-mac.sh
echo ""
echo "Có thể đóng cửa sổ này."
read -n 1 -s -r -p "Nhấn phím bất kỳ để đóng…"
