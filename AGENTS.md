# AGENTS.md — hướng dẫn cho người / AI tiếp tục dự án Mini-Asi

> Đọc file này trước khi sửa code. Kiến trúc chi tiết: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).
> Lịch sử thay đổi: [CHANGELOG.md](CHANGELOG.md). Hướng dẫn người dùng: [README.md](README.md).

## Dự án là gì

Mini-Asi là trợ lý desktop sống quanh **tai thỏ** MacBook (máy không tai thỏ: tai thỏ ảo / viên thuốc / bong bóng).
Chủ dự án: Đỗ Bình Dương (người Việt). **Giao diện, chú thích code và trả lời người dùng đều bằng tiếng Việt.**
Ưu tiên macOS, sau đó Linux & Windows. Có bản Free và Pro (khoá ký ed25519, kiểm tra offline).

Luôn có: Asi (chat & lệnh giọng nói), Việc (hiểu câu tự nhiên Việt/Anh), Ghi chú, Clipboard (mục ghim giữ mãi), kéo thả file
(AirDrop / nén / PDF), thả nội dung → việc / sự kiện lịch (`capture.rs`, `commands/capture.rs`).
Mô-đun bật/tắt (`settings.modules`): Dọn máy, Tập trung, Social (id `threads`, mặc định bật) · Nhật ký, Từ mới (người dùng tự bật).
Nền: nhắc nghỉ mắt/uống nước, đồng bộ Apple (Lời nhắc, Ghi chú, Lịch), sao lưu đám mây, giọng đọc tải thêm, tự cập nhật.

## Công nghệ

- **Tauri v2** (Rust backend + webview). Không framework UI: `ui/` là HTML/CSS/JS thuần, không có bước build.
- Rust workspace: `src-tauri` (app) + `crates/asi-clean`, `crates/notch-native`, `crates/license-tool`.
- macOS API qua `objc2` trong `crates/notch-native` (NSScreen, NSWindow level, NSPasteboard, EventKit, Speech, TIS bộ gõ, quyền).
- Giọng đọc: MỘT bộ đọc Piper (NghiTTS) chạy bằng Python riêng (cài qua `uv`) trong thư mục dữ liệu app — `src-tauri/src/tts.rs`.
  **Không** thêm lại giọng trực tuyến (edge-tts, Google Translate TTS…) hay giọng Linh của macOS cho tiếng Việt.
- "Bộ não" của Asi (`settings.ai_provider`): Apple Intelligence (`apple_ai.rs` + `resources/asi_ai.swift`, biên dịch sẵn trong
  `build.rs`) hoặc ChatGPT / Gemini / Claude (`llm.rs`, API key trong Keychain qua `threads::secret`, tên mục `ai_key_<id>`).
  **Không** để mô hình tự ghi dữ liệu khi người dùng chỉ hỏi — mọi hành động ghi phải qua `assistant::user_asks_for`.
  Luồng hỏi đáp: `commands/assistant.rs::asi_ask` — lệnh ngắn rõ ràng → luật (`assistant.rs`), còn lại → mô hình trả
  `{reply, action, argument}` → `reply_from_ai` thực hiện hành động bằng lệnh có sẵn. Thêm hành động mới: thêm vào
  `apple_ai::ACTIONS`, mô tả trường `action` trong asi_ai.swift, chỉ dẫn trong `ai_instructions` và nhánh trong `reply_from_ai`.

## Chạy, kiểm thử, đóng gói

```bash
cargo test --workspace            # kiểm thử Rust (mọi logic thuần đều có test)
node ui/vnkey.test.js             # bộ gõ Telex/VNI
bash scripts/preview-ui.sh        # xem UI trong trình duyệt với dev-mock.js — xem "Xem thử UI" bên dưới
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
- UI (không framework, không build): `common.js` (cầu nối + định dạng, dùng chung) → `app.js` (vỏ hòn đảo, xuất `window.ASI`)
  → `views.js` / `extra.js` / `vocab.js` đăng ký nội dung tab bằng `A.views.<tab> = fn`. Tab mới: thêm vào mảng `TABS` trong app.js
  (cột thứ 4 = id mô-đun nếu bật/tắt được) + công tắc trong `secFeatures()` của settings.js.
- Cài đặt là cửa sổ riêng `settings.html` + `settings.js`. **Lưu cài đặt bằng `patch_settings` với đúng các khoá vừa đổi**
  (không gửi cả object bằng `save_settings` — hai cửa sổ sẽ ghi đè nhau bằng bản cũ). Backend phát `settings-changed` cho mọi cửa sổ.
- Sự kiện nội bộ UI: `asi-ready`, `asi-status`, `asi-tasks`, `asi-chat` (document) — view tự vẽ lại phần của mình khi cần.
- Không vẽ lại cả khối khi chỉ đổi con số (người dùng ghét nhấp nháy) — xem `updateBar()` / `renderPeek()` so sánh rồi mới đổi.
- **Hệ thiết kế (base.css)**: nền đen `#000`, chữ trắng, chữ phụ `--text-2`, mặt nổi `--surface`/`--surface-2`; màu chỉ để báo
  trạng thái (đỏ = trễ, cam = cảnh báo, xanh lá = xong, tím = tập trung). Nút chính `.btn.primary` (trắng chữ đen), nút phụ `.btn`,
  nút chữ `.link`, nút biểu tượng `.ib`, ô nhập `.field`, hàng danh sách `.row` (nút phụ `.acts` chỉ hiện khi rê chuột).
  Chữ trên nút ≤ 2 từ; không lặp thông tin đã có ở chỗ khác; không thêm emoji vào nút.
- Cửa sổ chính luôn ở tầng cao; khi macOS hiện hộp thoại xin quyền phải gọi `window_low(app, true)` rồi trả lại.

## Ràng buộc quan trọng (đừng phá)

- **Không commit/gửi thư mục `secrets/`** (khoá bí mật ký bản quyền, key Pro). Đã có trong `.gitignore`.
  Khoá công khai nằm ở `src-tauri/src/license.rs`. Trước khi bán thật: tạo cặp khoá mới bằng `license-tool keygen`.
- Đăng bài Threads chỉ khi người dùng bấm Đăng **và** xác nhận. Không tự động đăng.
- Tab Social: chỉ dùng ô chữ viết tắt trung tính (Fb, Ms, Ig…), **không** vẽ/nhúng logo thương hiệu. Cửa sổ mạng xã hội
  không được cấp capabilities (chỉ `notch`, `settings` có quyền gọi lệnh); thêm mạng mới ở `social.rs` kèm tên miền được phép.
- Thông tin trực tiếp (thời tiết, tin tức, tỷ giá, giá vàng, chứng khoán…) **không bao giờ** hỏi mô hình — đi qua `live.rs`
  (nguồn thật hoặc nút "Xem trên web"). Apple Intelligence chỉ nhận lịch sử khi `assistant::is_follow_up`; câu trả lời lặp
  bị chặn bằng `assistant::is_repeat`.
- Ghi cuộc họp: chỉ ghi khi người dùng bấm (không tự ghi); biên bản/âm thanh chỉ lưu trên máy; chỉ gửi toàn văn lên Gemini
  khi người dùng chọn Gemini. Việc cần làm trong biên bản chỉ vào tab Việc khi người dùng tick & bấm thêm.
- Thả nội dung → việc/lịch: luôn để người dùng xem & bấm tạo, không tự tạo. Ảnh chỉ đọc bằng Vision trên máy; chỉ chữ đọc được
  (không phải ảnh) mới gửi cho AI đám mây nếu người dùng chọn bộ não đám mây.
- Giọng đọc: **không** dùng các giọng NghiTTS mang tên/giọng người nổi tiếng hoặc giọng thương mại
  (Mỹ Tâm, Trấn Thành, Ngọc Ngạn, Việt Thảo, Ban Mai, Thanh Phương Viettel, Minh Khang, Tài An, Lạc Phi, Adam…).
  Chỉ dùng giọng thường (hiện tại: Mai Phương, Minh Quang). VieNeu-TTS: Apache-2.0, giữ ghi công.
- Dọn máy: cache có thể xoá hẳn; file của người dùng phải vào Thùng rác. Luật dọn ở `crates/asi-clean/rules/`.
- Đồng bộ Lời nhắc: chỉ xoá việc ở Mini-Asi khi chắc chắn bên Apple đã xoá (đã từng thấy ở lần kéo trước) — xem `sync::plan` và test.
- Dữ liệu người dùng chỉ lưu trên máy (thư mục dữ liệu Tauri: `~/Library/Application Support/com.miniasi.app/` trên Mac).

## Xem thử UI (trình duyệt, dữ liệu giả)

`ui/index.html?open=peek|home|tasks|notes|meet|clips|journal|words|threads|clean|listen|reply|sheet|drop|dropc|capture|alert-reminder|alert-break|alert-journal`
· `drop=text|image|contact|url` (với open=capture) · `meet=rec` (giả lập đang ghi họp) · `late=0|3` · `notch=0` · `disp=pill|bubble` · `mods=journal,words,clean,focus,threads` · `media=0` · `threads=1` · `update=1` · `journal=1`
· Cửa sổ Cài đặt: `ui/settings.html?sec=general|look|features|reminders|assistant|ime|clips|sync|perms|pro`.

## Những gì chưa kiểm chứng trên Mac thật (ưu tiên test khi có máy)

- Giọng NghiTTS (cài qua `uv`, tải giọng) — mới test bằng mô hình giả.
- Trợ lý Apple Intelligence (`asi_ai.swift`) — chưa biên dịch/chạy trên macOS 26 thật.
- Tự chuyển bộ gõ khi dùng bộ gõ bên thứ ba (VietTelex, OpenKey…), tầng cửa sổ khi hiện hộp thoại quyền.
- Cửa sổ Threads ở chế độ bong bóng (đã sửa lỗi bị che & ẩn nhầm, cần xác nhận).
- Học từ mới: tra từ online (dictionaryapi.dev) và chấm phát âm bằng nhận dạng giọng nói tiếng Anh.

## Lộ trình gợi ý

0. Rủi ro cần xử lý trước khi bán: updater đọc GitHub Releases của kho private bằng token máy dev và chưa kiểm chữ ký gói
   cập nhật; NghiTTS chưa công bố nguồn dữ liệu giọng (xin xác nhận tác giả); trợ lý Apple Intelligence chưa chạy thử trên
   máy macOS 26 thật (kiểm tra cú pháp `@Generable`/`@Guide` khi build bằng Xcode 26).
1. Ký số + notarize Apple; ký gói cập nhật.
2. Chạy VieNeu trực tiếp trong Rust (crate `ort`) để bỏ Python (~150 MB).
3. Nhận dạng giọng nói trên Windows/Linux (Whisper on-device).
4. Đồng bộ Google Calendar / Tasks.
5. Peek: thay 4 nút cố định bằng nút người dùng tự chọn (giới hạn 4).
