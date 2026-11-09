# Mini-Asi — trợ lý sống quanh tai thỏ

Mini-Asi là một hòn đảo nhỏ màu đen ôm lấy tai thỏ MacBook, kiểu Dynamic Island. Máy không có tai thỏ: app tạo tai thỏ ảo,
hoặc chọn **Viên thuốc** (nổi dưới thanh menu) / **Bong bóng** (nút tròn kéo đi đâu cũng được) trong Cài đặt › Tai thỏ.
Bên trong là linh vật hai mắt biết chớp, cười, lo lắng, ngáp ngủ — và **giận đỏ mắt khi bạn có việc trễ**.

**Ba trạng thái:** thu gọn (chỉ hiện điều quan trọng: số việc trễ, lịch sắp tới, đồng hồ tập trung) →
**rê chuột** (một dòng trạng thái, nhạc đang phát, 4 nút nhanh: Micro · Camera · AirDrop · Tập trung) →
**bấm** để mở bảng.

| Luôn có | |
|---|---|
| ✦ **Asi** | Trang đầu: lời chào, 3 việc/lịch gần nhất, gợi ý câu hỏi. Gõ hoặc nói: *"hôm nay có gì"*, *"máy mình thế nào"*, *"nhắc tôi 3h chiều gọi khách"*, *"dọn rác giúp mình"*… Có Apple Intelligence (macOS 26) thì hỏi gì cũng được — viết, dịch, giải thích, lên kế hoạch — và Asi tự thêm việc / ghi chú khi bạn nhờ. Nút mic ở tai thỏ để ra lệnh không cần mở bảng. |
| ✅ **Việc** | Câu tự nhiên Việt/Anh, có dấu hay không: *"mai 9h gọi khách"*, *"thứ 6 tuần sau chốt hợp đồng"*, *"tomorrow 3pm call John"*. Nhóm Trễ hạn / Hôm nay / Sắp tới; lịch hôm nay (Lịch Apple). Đến giờ: Xong · 10 phút · 1 giờ · Mai. |
| ✎ **Ghi chú** | Tự lưu, ghim, tìm, đọc cho Asi ghi. ⌘⇧Space mở nhanh. |
| 📋 **Clipboard** | Nhớ chữ, ảnh, file đã chép. Bấm = chép lại, bấm đúp = dán luôn. Ghim = giữ mãi ở đầu danh sách (đặt tên được). ⌃⇧V. |
| 📦 **Kéo thả file** | Thả vào **nửa trái** tai thỏ = AirDrop ngay; **nửa phải** = Nén ZIP, giải nén, nén/gộp PDF, chép file. |
| 🗓 **Thả → việc / lịch** | Kéo chữ, tin nhắn, link, ảnh, ảnh chụp màn hình, danh bạ vào tai thỏ → Asi đề xuất việc hoặc sự kiện (đọc cả chữ trong ảnh). Sự kiện vào lịch iCloud hoặc Google (tài khoản Google thêm trong Cài đặt hệ thống › Tài khoản Internet). |

| Mô-đun (bật/tắt trong Cài đặt › Tính năng) | |
|---|---|
| 🧹 **Dọn máy** *(mặc định bật)* | Cache, log, Xcode, npm/brew, trình duyệt. Pro: file cài đặt cũ, `node_modules` bỏ lâu, sao lưu iPhone. Cache xoá hẳn, file của bạn vào Thùng rác. |
| ◎ **Tập trung** *(mặc định bật)* | Nút 25 phút trên tai thỏ; Asi tạm im lặng, chỉ hiện đồng hồ tím. |
| ▦ **Social** *(mặc định bật)* | Facebook, Messenger, Instagram, Threads, X, TikTok, YouTube trong cửa sổ dạng điện thoại dưới tai thỏ; Threads nâng cao: thống kê, trả lời, đăng bài (luôn hỏi xác nhận). |
| 📔 **Nhật ký** | Mỗi tối Asi hỏi 3 câu ngắn về ngày của bạn. |
| 📚 **Từ mới** | 2–3 từ tiếng Anh mỗi lần mở, phiên âm, nghe mẫu, chấm phát âm. |

**Nhắc nhở** (Cài đặt › Nhắc nhở): nghỉ mắt, uống nước, giờ đi ngủ, Không làm phiền.
**Đồng bộ**: Việc ⇄ Lời nhắc, Ghi chú ⇄ Ghi chú, Lịch; sao lưu Google Drive / iCloud, đồng bộ nhiều máy.
**Giọng đọc**: một giọng tiếng Việt nhẹ chạy trên máy (Mai Phương / Minh Quang), tải trong Cài đặt › Asi & giọng nói.

Việc, ghi chú, clipboard, nhật ký chỉ lưu trên máy bạn. Những thứ cần mạng: tra từ điển (Từ mới), Threads, kiểm tra cập nhật,
tải giọng đọc lần đầu. Hỏi đáp với Apple Intelligence và đọc to đều chạy trên máy.

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
4. Trong Mini-Asi, tab **Social** › **Threads nâng cao**, chọn một trong hai cách:
   - **Dán token**: dùng **User Token Generator** trong trang Settings của app để tạo token rồi dán vào.
   - **Đăng nhập (OAuth)**: nhập App ID, App Secret và Redirect URI (mặc định `https://localhost/`, nhớ thêm vào **Redirect Callback URLs**). Bấm "Mở trang đăng nhập", chọn Cho phép, rồi chép địa chỉ trang hiện ra dán lại vào Mini-Asi.

Token dài hạn có hạn 60 ngày; Mini-Asi tự gia hạn khi còn dưới 10 ngày. App chưa qua duyệt của Meta chỉ dùng được với tài khoản là Threads Tester (tức là chính bạn).

## Giọng đọc của Asi

- **Một bộ đọc duy nhất, chạy trên máy:** NghiTTS (Piper/VITS) — mỗi giọng ≈ 60 MB, đọc gần như tức thì trên CPU,
  RAM ≈ 100 MB khi đang đọc và tự tắt sau 5 phút rảnh. Hai giọng thường: **Mai Phương** (nữ), **Minh Quang** (nam);
  chỉ tải giọng bạn chọn. Lần đầu tải ≈ 156 MB (Python riêng + piper + 1 giọng), nằm trong
  `~/Library/Application Support/com.miniasi.app/nghitts/`, gỡ là xoá sạch.
- **Không dùng nữa (0.13):** VieNeu-TTS (nặng 210–520 MB), edge-tts và Google Translate TTS (gửi chữ của bạn lên máy chủ
  bên ngoài, điều khoản không cho dùng thương mại), giọng Linh của macOS (kém tự nhiên). Thư mục `vieneu/` cũ được tự xoá.
- Câu trả lời bằng tiếng Anh dùng giọng tiếng Anh có sẵn của macOS (Samantha). Chưa tải giọng → Asi chỉ hiện chữ.
- Giấy phép: thư viện `piper-tts` / eSpeak NG là GPL — được tải về máy người dùng như chương trình riêng, không gói trong app.
  NghiTTS chưa công bố nguồn dữ liệu của từng giọng → nên xin xác nhận của tác giả trước khi bán rộng.

## Bộ não của Asi: Apple Intelligence, ChatGPT, Gemini, Claude

Cài đặt › Asi & giọng nói › **Bộ não của Asi**: chọn Apple Intelligence (chạy trên máy, miễn phí) hoặc ChatGPT / Gemini /
Claude bằng API key của bạn (lưu trong Keychain; phí tính vào tài khoản của bạn; tiếng Việt tốt hơn mô hình trên máy).
Dưới mỗi câu trả lời có ghi nhỏ ai đã trả lời. Có thể tắt "Gửi kèm ngữ cảnh" để không gửi việc / ghi chú / clipboard lên mạng.

Với Apple Intelligence (macOS 26 + chip Apple + đã bật): câu hỏi tự do đi qua mô hình trên máy (FoundationModels).
Asi nhớ 6 lượt gần nhất (30 phút), biết việc, lịch, ghi chú, mục vừa chép và tình trạng máy, và **chỉ khi bạn nhờ rõ ràng**
mới tự thêm việc, ghi chú, bật tập trung; ngoài ra có thể mở lịch hôm nay / việc trễ / tình trạng máy / dọn rác. Lệnh ngắn rõ ràng
("dọn rác", "nhắc tôi 3h…") vẫn chạy bằng luật cho nhanh. Trợ lý Swift được biên dịch sẵn lúc đóng gói (cần Xcode 26 trên
máy build) để máy khách không phải cài công cụ gì.

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
│  ├─ extra.js · vocab.js   Tab Clipboard · Nhật ký · Social/Threads · Từ mới
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
