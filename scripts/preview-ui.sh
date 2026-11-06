#!/bin/bash
# Xem thử giao diện trong trình duyệt (dữ liệu giả lập, không cần build Rust)
cd "$(dirname "$0")/../ui"
echo "Mở http://localhost:8765  — thêm ?late=3 để xem tai thỏ đỏ, ?notch=0 để xem tai thỏ ảo"
python3 -m http.server 8765
