# Mini-Asi — trợ lý sống quanh tai thỏ

Mini-Asi là một hòn đảo nhỏ màu đen ôm lấy tai thỏ MacBook, kiểu Dynamic Island. Máy không có tai thỏ: app tạo tai thỏ ảo,
hoặc chọn **Viên thuốc** (nổi dưới thanh menu) / **Bong bóng** (nút tròn kéo đi đâu cũng được) trong Cài đặt › Tai thỏ.
Bên trong là linh vật hai mắt biết chớp, cười, lo lắng, ngáp ngủ — và **giận đỏ mắt khi bạn có việc trễ**.

**Ba trạng thái:** thu gọn (chỉ hiện điều quan trọng: số việc trễ, lịch sắp tới, đồng hồ tập trung) →
**rê chuột** (một dòng trạng thái, nhạc đang phát, 4 nút nhanh: Micro · Camera · AirDrop · Tập trung) →
**bấm** để mở bảng.

| Luôn có | |
|---|---|
| ✦ **Asi** | Trang đầu: lời chào, 3 việc/lịch gần nhất, gợi ý câu hỏi. Gõ hoặc nói: *"hôm nay có gì"*, *"máy mình thế nào"*, *"nhắc tôi 3h chiều gọi khách"*, *"dọn rác giúp mình"*… Có Apple Intelligence (macOS 26) thì hỏi đáp tự do ngay trên máy. Nút mic ở tai thỏ để ra lệnh không cần mở bảng. |
| ✅ **Việc** | Câu tự nhiên Việt/Anh, có dấu hay không: *"mai 9h gọi khách"*, *"thứ 6 tuần sau chốt hợp đồng"*, *"tomorrow 3pm call John"*. Nhóm Trễ hạn / Hôm nay / Sắp tới; lịch hôm nay (Lịch Apple). Đến giờ: Xong · 10 phút · 1 giờ · Mai. |
| ✎ **Ghi chú** | Tự lưu, ghim, tìm, đọc cho Asi ghi. ⌘⇧Space mở nhanh. |
| 📋 **Clipboard** | Nhớ chữ, ảnh, file đã chép. Bấm = chép lại, bấm đúp = dán luôn, ghim để giữ mãi. ⌃⇧V. |
| 📦 **Kéo thả file** | Thả vào **nửa trái** tai thỏ = AirDrop ngay; **nửa phải** = Nén ZIP, giải nén, nén/gộp PDF, chép file. |

| Mô-đun (bật/tắt trong Cài đặt › Tính năng) | |
|---|---|
| 🧹 **Dọn máy** *(mặc định bật)* | Cache, log, Xcode, npm/brew, trình duyệt. Pro: file cài đặt cũ, `node_modules` bỏ lâu, sao lưu iPhone. Cache xoá hẳn, file của bạn vào Thùng rác. |
| ◎ **Tập trung** *(mặc định bật)* | Nút 25 phút trên tai thỏ; Asi tạm im lặng, chỉ hiện đồng hồ tím. |
| @ **Threads** *(mặc định bật)* | Mở Threads trong cửa sổ nhỏ riêng; nâng cao: thống kê, trả lời, đăng bài (luôn hỏi xác nhận). |
| 📔 **Nhật ký** | Mỗi tối Asi hỏi 3 câu ngắn về ngày của bạn. |
| 📚 **Từ mới** | 2–3 từ tiếng Anh mỗi lần mở, phiên âm, nghe mẫu, chấm phát âm. |

**Nhắc nhở** (Cài đặt › Nhắc nhở): nghỉ mắt, uống nước, giờ đi ngủ, Không làm phiền.
**Đồng bộ**: Việc ⇄ Lời nhắc, Ghi chú ⇄ Ghi chú, Lịch; sao lưu Google Drive / iCloud, đồng bộ nhiều máy.
**Giọng đọc**: giọng có sẵn của máy (offline) hoặc gói giọng tải thêm trong Cài đặt › Asi & giọng nói.

Việc, ghi chú, clipboard, nhật ký chỉ lưu trên máy bạn. Những thứ cần mạng: tra từ điển (Từ mới), Threads, kiểm tra cập nhật,
và một số giọng đọc trực tuyến.

## Cài lên Mac

### Cách 0 (dễ nhất): nhấp đúp

Giải nén mã nguồn → nhấp đúp **“Tạo file cài đặt cho Mac.command”** (lần đầu: chuột phải → Open → Open). Cửa sổ Terminal tự chạy, xong sẽ mở sẵn file `.dmg` — kéo Mini-Asi vào Applications.

### Cách 1: tự đóng gói trên Mac (khoảng 10 phút lần đầu)

```bash
cd đường-dẫn/mini-asi
bash scripts/build-mac.sh
```

Script tự kiểm tra và cài những gì còn thiếu (Xcode CLT, Rust, gói npm), chạy kiểm thử, rồi tạo **file `.dmg` chạy được trên cả chip Apple lẫn Intel**.
Mở file `.dmg` và kéo Mini-Asi vào Applications.

> Vì app chưa ký số Apple, lần đầu mở bạn cần **chuột phải → Open**, hoặc vào *System Settings → Privacy & Security → Open Anyway*.

### Cách 2: để GitHub đóng gói hộ

1. Tạo repo riêng (private) trên GitHub và đẩy thư mục này lên.
2. Vào tab **Actions → build → Run workflow**.
3. Sau khoảng 15 phút, trong mục **Releases** sẽ có bản nháp chứa `.dmg` (Mac), `.msi` (Windows), `.deb` và `.AppImage` (Linux).

### Các quyền nên cấp

Khi macOS hiện hộp thoại xin quyền, Mini-Asi tự thu gọn và hạ xuống dưới để bạn bấm **Cho phép** được.

- **Thông báo**: macOS sẽ tự hỏi ở lần nhắc đầu tiên.
- **Micro & Nhận dạng giọng nói**: macOS hỏi ở lần đầu bấm nút mic.
- **Lịch**, **Tự động hoá (Lời nhắc, Ghi chú)**: macOS hỏi khi bạn bật đồng bộ trong Cài đặt.
- **Trợ năng** (không bắt buộc): để "chọn là dán luôn" trong Clipboard. Không bật thì Asi chép, bạn tự ⌘V.
- **Full Disk Access** (không bắt buộc): để quét sâu hơn. Vào tab Dọn máy, bấm "Cấp quyền Full Disk Access".
- **Nén PDF đẹp hơn** (không bắt buộc): chạy `brew install ghostscript`. Nếu không cài, app dùng bộ lọc có sẵn của macOS.
- **Giải nén RAR/7z** (không bắt buộc): chạy `brew install unar`.

---

## Sao lưu lên Google Drive

1. Cài **Google Drive cho máy tính** (google.com/drive/download) và đăng nhập — thư mục “My Drive” sẽ có trong Finder.
2. Mini-Asi tự tìm thấy thư mục đó. Vào **Cài đặt › Đồng bộ & sao lưu** → **Sao lưu ngay**.
3. Dữ liệu nằm ở `My Drive/Mini-Asi/` (bản theo ngày trong `backups/`, trạng thái từng máy trong `devices/`).
4. Máy thứ hai: cài Mini-Asi + Google Drive cùng tài khoản, bật **Đồng bộ giữa các máy** → dữ liệu hai máy tự gộp mỗi 15 phút.
5. Khôi phục: chọn một bản trong danh sách → bấm **Khôi phục** 2 lần. Bản đang dùng được cất riêng trước khi ghi đè.

## Kết nối Threads (nâng cao, không bắt buộc)

Chỉ cần nếu bạn muốn Asi hiện thống kê bài viết và báo trả lời mới ngay trên tai thỏ. Để lướt Threads thì không cần bước này.

Threads chỉ cho app bên ngoài truy cập qua **Threads API** của Meta, nên bạn cần một app (miễn phí) trên Meta for Developers:

1. Vào [developers.facebook.com/apps](https://developers.facebook.com/apps), bấm **Create app**, chọn use case **Access the Threads API**.
2. Trong **Use cases › Threads API › Settings**: bật các quyền `threads_basic`, `threads_content_publish`, `threads_read_replies`, `threads_manage_replies`, `threads_manage_insights`, `threads_manage_mentions`.
3. Thêm tài khoản Threads của bạn vào **App roles › Roles › Threads Testers**, rồi chấp nhận lời mời trong app Threads (Cài đặt › Tài khoản › Quyền truy cập website).
4. Trong Mini-Asi, tab **Threads**, chọn một trong hai cách:
   - **Dán token**: dùng **User Token Generator** trong trang Settings của app để tạo token rồi dán vào.
   - **Đăng nhập (OAuth)**: nhập App ID, App Secret và Redirect URI (mặc định `https://localhost/`, nhớ thêm vào **Redirect Callback URLs**). Bấm "Mở trang đăng nhập", chọn Cho phép, rồi chép địa chỉ trang hiện ra dán lại vào Mini-Asi.

Token dài hạn có hạn 60 ngày; Mini-Asi tự gia hạn khi còn dưới 10 ngày. App chưa qua duyệt của Meta chỉ dùng được với tài khoản là Threads Tester (tức là chính bạn).

## Giọng đọc VieNeu-TTS — cách đóng gói

- Mô hình **VieNeu-TTS v3 Turbo** (Apache-2.0, cho phép dùng thương mại; tác giả Phạm Nguyễn Ngọc Bảo) không nằm trong file cài để file `.dmg` vẫn nhẹ.
- Bấm **Tải giọng VieNeu**: Mini-Asi tải `uv`, tạo Python 3.12 riêng (đã bỏ bộ test, IDLE, Tk) và gói `vieneu` bản tối giản (ONNX Runtime, không torch, không gradio), rồi tải mô hình.
- Chỉ tải phần cần để **đọc bằng giọng dựng sẵn**: bỏ bộ khử nhiễu (43 MB) và bộ mã hoá codec (45 MB) vì chúng chỉ dùng để nhân bản giọng.
  - Bản **Gọn**: backbone int8 165 MB + bộ giải mã codec 45 MB ≈ 210 MB. Mặc định trên chip Apple.
  - Bản **Chất lượng cao**: backbone fp32 475 MB + codec 45 MB ≈ 520 MB. Mặc định trên máy Intel (int8 dễ méo tiếng trên CPU Intel không có VNNI).
  - Đã so với VieNeu v3 Nano: Nano cần ≈ 280 MB, chất lượng thấp hơn và không có giọng Hải Đăng → bản Gọn của Turbo vừa nhỏ hơn vừa hay hơn. Tất cả nằm trong `~/Library/Application Support/com.miniasi.app/vieneu/`, không động vào Python của máy.
- Khi đọc, một tiến trình nền giữ mô hình trong RAM và đọc từng câu; câu đầu xong là phát ngay. Rảnh 20 phút thì tự tắt để trả RAM. Lần đầu mở tai thỏ cần ≈ 15 giây để nạp, trong lúc đó Asi dùng giọng của máy.
- Ghi công: VieNeu-TTS © Phạm Nguyễn Ngọc Bảo, github.com/pnnbao97/VieNeu-TTS (Apache-2.0); bộ mã âm thanh MOSS-Audio-Tokenizer-Nano (Apache-2.0).

## Các bộ đọc tiếng Việt đã đánh giá (11/2026)

| Bộ đọc | Dung lượng | Vì sao chưa dùng |
|---|---|---|
| **NghiTTS** (Piper/VITS) | ≈ 60 MB mỗi giọng, rất nhanh | **Đã thêm (0.10)** như bộ đọc tuỳ chọn, chỉ với 2 giọng thường (Mai Phương, Minh Quang). Các giọng mang tên người nổi tiếng / giọng thương mại (Mỹ Tâm, Trấn Thành, Ngọc Ngạn, Ban Mai, Viettel…) bị loại vì không có giấy phép. Lưu ý: dự án không công bố nguồn dữ liệu của từng giọng; thư viện chạy `piper-tts`/eSpeak NG là GPL — được tải về máy người dùng như một chương trình riêng, không gói trong app. |
| **ZeroTTS** (GGUF, qua wyoming-vietnamese) | q4 ≈ 130 MB / q8 ≈ 216 MB + codec 45 MB | MIT, nhưng giọng mẫu ghi "chỉ để thử nghiệm", chưa có cách chạy gọn trên Mac ngoài tự biên dịch C++; dung lượng ngang bản Gọn của VieNeu. |
| **VieNeu-TTS v3 Turbo – bản Gọn** (đang dùng) | ≈ 210 MB | Apache-2.0, cho phép thương mại, giọng có đồng ý theo thẻ mô hình. |

## Free và Pro

Khoá bản quyền được kiểm tra **offline** bằng chữ ký ed25519. App chỉ chứa khoá công khai, nên không ai tự tạo được khoá hợp lệ.

```bash
# Tạo cặp khoá cho bản thương mại (làm 1 lần; cất khoá bí mật thật kỹ)
cargo run -p license-tool -- keygen
#   → dán KHOÁ CÔNG KHAI vào src-tauri/src/license.rs (PUBLIC_KEY)

# Cấp khoá Pro cho khách (vĩnh viễn, hoặc thêm số ngày để giới hạn)
MASI_SECRET_KEY=<khoá bí mật> cargo run -p license-tool -- sign khach@email.com
MASI_SECRET_KEY=<khoá bí mật> cargo run -p license-tool -- sign khach@email.com 365
```

> ⚠️ Thư mục `secrets/` chứa **khoá thử nghiệm** (`DEV-pro-key.txt` dùng để thử kích hoạt Pro).
> Trước khi bán: tạo cặp khoá mới, thay PUBLIC_KEY, và **không** đưa `secrets/` lên GitHub (đã có trong .gitignore).

---

## Dành cho lập trình viên

Tài liệu kỹ thuật: **[AGENTS.md](AGENTS.md)** (đọc trước khi sửa) và **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)**.

```
mini-asi/
├─ ui/                      HTML/CSS/JS thuần (không cần build)
│  ├─ index.html · app.js   Cửa sổ tai thỏ: hình học, linh vật, peek, báo động, giọng nói, kéo thả file, thanh tab
│  ├─ views.js              Tab Asi · Việc · Ghi chú · Dọn máy
│  ├─ extra.js · vocab.js   Tab Clipboard · Nhật ký · Threads · Từ mới
│  ├─ settings.html/.js     Cửa sổ Cài đặt riêng
│  ├─ common.js             Cầu nối Tauri, định dạng ngày giờ/dung lượng (dùng chung 2 cửa sổ)
│  ├─ base.css · style.css · settings.css
│  ├─ vnkey.js              Bộ gõ Telex/VNI riêng (+ vnkey.test.js)
│  └─ dev-mock.js           Giả lập backend để xem UI trong trình duyệt
├─ src-tauri/src/           lib.rs · prelude.rs · commands/* (lệnh mỏng) · *.rs (logic thuần có test)
├─ crates/                  asi-clean (dọn máy) · notch-native (API macOS) · license-tool (khoá Pro)
└─ scripts/                 build-mac.sh · dev.sh · preview-ui.sh
```

```bash
cargo test --workspace     # kiểm thử Rust
node ui/vnkey.test.js      # bộ gõ
bash scripts/dev.sh        # chạy app thật
bash scripts/preview-ui.sh # xem UI trong trình duyệt: ?open=peek|home|tasks|… ?late=3 ?disp=bubble ?mods=journal,words
```
