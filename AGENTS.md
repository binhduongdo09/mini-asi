# AGENTS.md — hướng dẫn cho người / AI tiếp tục dự án Mini-Asi

> Đọc file này trước khi sửa code. Kiến trúc chi tiết: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).
> Lịch sử thay đổi: [CHANGELOG.md](CHANGELOG.md). Hướng dẫn người dùng: [README.md](README.md).

## Dự án là gì

Mini-Asi là trợ lý desktop sống quanh **tai thỏ** MacBook (máy không tai thỏ: tai thỏ ảo / viên thuốc / bong bóng).
Chủ dự án: Đỗ Bình Dương (người Việt). **Giao diện, chú thích code và trả lời người dùng đều bằng tiếng Việt.**
Ưu tiên macOS, sau đó Linux & Windows. Có bản Free và Pro (khoá ký ed25519, kiểm tra offline).

Tính năng chính: chat & lệnh giọng nói với Asi, Việc (hiểu câu tự nhiên Việt/Anh), Ghi chú, Clipboard, Nhật ký,
Học từ mới tiếng Anh, Threads, dọn máy, nén/giải nén/PDF, sức khoẻ (nghỉ mắt, tập trung), đồng bộ Apple
(Lời nhắc, Ghi chú, Lịch), sao lưu Google Drive, giọng đọc tải thêm (VieNeu-TTS, NghiTTS).

## Công nghệ

- **Tauri v2** (Rust backend + webview). Không framework UI: `ui/` là HTML/CSS/JS thuần, không có bước build.
- Rust workspace: `src-tauri` (app) + `crates/asi-clean`, `crates/notch-native`, `crates/license-tool`.
- macOS API qua `objc2` trong `crates/notch-native` (NSScreen, NSWindow level, NSPasteboard, EventKit, Speech, TIS bộ gõ, quyền).
- Giọng đọc tải thêm chạy bằng Python riêng (cài qua `uv`) trong thư mục dữ liệu app — xem `src-tauri/src/tts.rs`.

## Chạy, kiểm thử, đóng gói

```bash
cargo test --workspace            # kiểm thử Rust (mọi logic thuần đều có test)
node ui/vnkey.test.js             # bộ gõ Telex/VNI
bash scripts/preview-ui.sh        # xem UI trong trình duyệt với dev-mock.js (?late=3 ?notch=0 ?disp=bubble ?tts=ready ...)
bash scripts/dev.sh               # chạy app thật (cần Rust + Node)
bash scripts/build-mac.sh         # ra file .dmg (Mac); "Tạo file cài đặt cho Mac.command" = bản nhấp đúp
npx tauri build --bundles deb     # Linux
```

Kiểm tra biên dịch cho macOS từ Linux (không cần Mac): `RUSTC_BOOTSTRAP=1 cargo check -p mini-asi -Zbuild-std=std,panic_abort --target aarch64-apple-darwin`
(cần mã nguồn thư viện chuẩn Rust ở `rustlib/src/rust/library` và đặt `CC_aarch64_apple_darwin` trỏ tới một trình biên dịch C giả chỉ tạo file rỗng).

## Quy ước code

- **Chú thích & chuỗi hiển thị bằng tiếng Việt có dấu.** Mỗi file mở đầu bằng `//!` (Rust) hoặc khối `/* */` (JS) nói file làm gì.
- Logic thuần (phân tích câu, đồng bộ, chấm phát âm, …) đặt ở `src-tauri/src/<tên>.rs` và **phải có `#[cfg(test)]`**.
  Lệnh Tauri chỉ là lớp mỏng ở `src-tauri/src/commands/<tính năng>.rs`.
- Module lệnh dùng `use crate::prelude::*;` — prelude re-export import chung + mọi item của `commands::*`,
  nên các module gọi nhau thoải mái. Item dùng chéo module để `pub(crate)`. Tên hàm/kiểu phải **duy nhất** toàn crate.
- Thêm lệnh mới: viết `#[tauri::command] pub(crate) fn ten_lenh(...)` trong module phù hợp → thêm `ten_lenh`
  vào `tauri::generate_handler![...]` trong `src-tauri/src/lib.rs` → thêm bản giả trong `ui/dev-mock.js`.
- Thêm cài đặt mới: trường trong `store::Settings` (struct có `#[serde(default)]`, nhớ đặt giá trị mặc định trong `impl Default`)
  + giá trị trong `dev-mock.js`. UI đọc qua `get_settings`, lưu bằng `save_settings` (gửi cả object).
- UI: tiện ích chung xuất qua `window.ASI` (cuối `ui/app.js`); file phụ (`extra.js`, `vocab.js`) đăng ký view bằng `A.views.<tab> = fn`.
  Tab mới: thêm vào mảng `TABS` trong `app.js`.
- Không vẽ lại cả khối khi chỉ đổi con số (người dùng ghét nhấp nháy) — xem `updateBar()` cập nhật tại chỗ.
- Cửa sổ chính luôn ở tầng cao; khi macOS hiện hộp thoại xin quyền phải gọi `window_low(app, true)` rồi trả lại.

## Ràng buộc quan trọng (đừng phá)

- **Không commit/gửi thư mục `secrets/`** (khoá bí mật ký bản quyền, key Pro). Đã có trong `.gitignore`.
  Khoá công khai nằm ở `src-tauri/src/license.rs`. Trước khi bán thật: tạo cặp khoá mới bằng `license-tool keygen`.
- Đăng bài Threads chỉ khi người dùng bấm Đăng **và** xác nhận. Không tự động đăng.
- Giọng đọc: **không** dùng các giọng NghiTTS mang tên/giọng người nổi tiếng hoặc giọng thương mại
  (Mỹ Tâm, Trấn Thành, Ngọc Ngạn, Việt Thảo, Ban Mai, Thanh Phương Viettel, Minh Khang, Tài An, Lạc Phi, Adam…).
  Chỉ dùng giọng thường (hiện tại: Mai Phương, Minh Quang). VieNeu-TTS: Apache-2.0, giữ ghi công.
- Dọn máy: cache có thể xoá hẳn; file của người dùng phải vào Thùng rác. Luật dọn ở `crates/asi-clean/rules/`.
- Đồng bộ Lời nhắc: chỉ xoá việc ở Mini-Asi khi chắc chắn bên Apple đã xoá (đã từng thấy ở lần kéo trước) — xem `sync::plan` và test.
- Dữ liệu người dùng chỉ lưu trên máy (thư mục dữ liệu Tauri: `~/Library/Application Support/com.miniasi.app/` trên Mac).

## Những gì chưa kiểm chứng trên Mac thật (ưu tiên test khi có máy)

- Giọng VieNeu / NghiTTS (cài qua `uv`, tải mô hình) — mới test bằng mô hình giả.
- Tự chuyển bộ gõ khi dùng bộ gõ bên thứ ba (VietTelex, OpenKey…), tầng cửa sổ khi hiện hộp thoại quyền.
- Cửa sổ Threads ở chế độ bong bóng (đã sửa lỗi bị che & ẩn nhầm, cần xác nhận).
- Học từ mới: tra từ online (dictionaryapi.dev) và chấm phát âm bằng nhận dạng giọng nói tiếng Anh.

## Lộ trình gợi ý

1. Ký số + notarize Apple, Tauri updater.
2. Chạy VieNeu trực tiếp trong Rust (crate `ort`) để bỏ Python (~150 MB).
3. Nhận dạng giọng nói trên Windows/Linux (Whisper on-device).
4. Đồng bộ Google Calendar / Tasks.
5. Tách `ui/app.js` (≈1800 dòng) thành các file theo tab như `extra.js` / `vocab.js`.
