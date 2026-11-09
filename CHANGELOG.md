# Nhật ký thay đổi

## 0.15.0 (11/2026) — ghim clipboard mãi mãi, thả nội dung → việc / lịch, tab Social
**Clipboard ghim**
- Mục ghim nằm riêng ở đầu ("Đã ghim", theo thứ tự ghim), giữ mãi cho tới khi bỏ ghim — "Xoá lịch sử" và giới hạn số mục
  không đụng tới. Đặt tên cho mục ghim (vd. "STK công ty"), tìm được theo tên; hiện số lần đã dùng. Lệnh mới `clip_label`.

**Thả vào tai thỏ → tạo việc / sự kiện lịch**
- Kéo **chữ đang chọn, tin nhắn trong app chat, link, ảnh trên web, ảnh chụp màn hình, danh bạ** vào tai thỏ → bảng
  "Tạo việc · lịch": Asi đề xuất tiêu đề, ngày giờ, địa điểm, ghi chú, chọn Việc hay Lịch; sửa lại rồi bấm tạo.
- File ảnh / .vcf / .txt kéo từ Finder: nửa phải → nút "Tạo việc · lịch". Trong Clipboard cũng có nút này cho chữ & ảnh.
- Chữ trong ảnh đọc bằng Vision ngay trên máy (tiếng Việt + Anh), không gửi ảnh đi đâu.
- Đề xuất tức thì bằng quy tắc trên máy (`capture.rs`, có test), rồi Apple Intelligence / AI đã chọn đọc lại kỹ hơn
  (không ghi đè nếu bạn đã sửa — hiện nút "Dùng gợi ý AI").
- Sự kiện vào lịch đã chọn (nhớ lựa chọn lần sau): iCloud, **Google** (tài khoản Google thêm trong Cài đặt hệ thống ›
  Tài khoản Internet → tự đồng bộ lên Google Calendar), Exchange… Danh bạ có ngày sinh → sự kiện cả ngày "Sinh nhật …".
- Việc có thêm ghi chú (nội dung gốc), hiện biểu tượng nhỏ trong danh sách.
- Lệnh mới: `drop_read`, `capture_supports`, `capture_set_text`, `capture_from_clip`, `capture_analyze`, `capture_create`,
  `calendar_list`; native: `clip::read_drag` (bảng kéo-thả macOS), `ocr::recognize_text`, `calendar::calendars`,
  `calendar::create_event_full`.

**Tab Threads → Social**
- 7 ô: Facebook, Messenger, Instagram, Threads, X, TikTok, YouTube — bấm là mở cửa sổ dạng điện thoại ngay dưới tai thỏ
  (mỗi mạng một cửa sổ, đăng nhập một lần là nhớ, bấm lần nữa để ẩn; chấm xanh = đang mở). Link ra ngoài → trình duyệt.
- Threads API (đăng bài, trả lời, nhắc đến) vẫn ở thẻ "Threads nâng cao" trong tab Social.
- Mã: `social.rs` (danh sách mạng + tên miền được phép, có test), lệnh `social_open`, `social_open_list`;
  `threads_web` dùng chung `open_mobile_window`. Mô-đun vẫn mang id `threads` để giữ cài đặt cũ.

## 0.14.0 (11/2026) — hỏi đáp đúng nghĩa trợ lý, thêm ChatGPT / Gemini / Claude
**Sửa lỗi "hỏi gì cũng bị đẩy vào Việc / Ghi chú"**
- Mô hình chỉ được thêm việc / ghi chú / bật tập trung khi người dùng nhờ rõ ràng (có "nhắc", "thêm việc", "ghi chú", "lưu"…)
  — `assistant::user_asks_for`. Chọn nhầm → hỏi lại ở chế độ chỉ trả lời.
- Câu hỏi (có "?", "không", "gì", "sao", "bao nhiêu", "thế nào"…) không còn bị luật hiểu thành việc chỉ vì có chữ "mai", "thứ 6".
- "Lưu ý…" không còn bị hiểu là lệnh ghi chú.
- AI lỗi / chưa sẵn sàng → báo rõ lý do (kèm nút Cài đặt AI, Lưu thành việc, Lưu ghi chú) thay vì âm thầm tạo việc.
- Chỉ dẫn cho mô hình: mặc định là trả lời; hành động chỉ khi được nhờ.
- Dưới mỗi câu trả lời hiện nhỏ ai đã trả lời: "Apple Intelligence", "Gemini · gemini-flash-latest", "Lệnh cơ bản".

**Bộ não tuỳ chọn** (Cài đặt › Asi & giọng nói › Bộ não của Asi): Apple Intelligence (trên máy) · ChatGPT · Gemini · Claude.
- API key của người dùng lưu trong Keychain (không nằm trong file cài đặt / bản sao lưu); nút Lưu tự kiểm tra kết nối.
- Ô mô hình + nút Kiểm tra: lấy danh sách mô hình của tài khoản, tên cũ thì tự chọn bản nhanh mới nhất.
- Công tắc "Gửi kèm ngữ cảnh" (việc, lịch, ghi chú, mục vừa chép, tình trạng máy) cho AI đám mây.
- Mã: `src-tauri/src/llm.rs` (dựng yêu cầu / đọc kết quả, có test), lệnh `ai_info`, `ai_set_key`, `ai_test`.

## 0.13.0 (11/2026) — giọng đọc gọn nhẹ, trợ lý Apple Intelligence thông minh hơn
**Giọng đọc**
- Chỉ còn **1 bộ đọc chạy trên máy**: NghiTTS (Piper) với 2 giọng thường Mai Phương / Minh Quang; chỉ tải giọng được chọn
  (≈ 156 MB lần đầu, ≈ 61 MB mỗi giọng thêm); tiến trình đọc tự tắt sau 5 phút rảnh (trước: 20 phút).
- Bỏ: VieNeu-TTS, edge-tts ("Asi Neural Studio" — thực chất là giọng Microsoft trực tuyến), Google Translate TTS,
  giọng Linh của macOS cho tiếng Việt; 6 tên giọng "ảo" (cùng 2 giọng Microsoft đổi cao độ). Tự xoá thư mục `vieneu/` cũ.
- Chưa tải giọng → Asi chỉ hiện chữ; tải xong tự bật "Đọc to câu trả lời". Câu tiếng Anh dùng giọng tiếng Anh của macOS.

**Trợ lý (Apple Intelligence)**
- Câu hỏi tự do (không chỉ khi luật "không hiểu") → Apple Intelligence; lệnh ngắn rõ ràng vẫn chạy bằng luật cho nhanh.
- Nhớ mạch trò chuyện (6 lượt / 30 phút) để hiểu câu nối tiếp; nút "Cuộc trò chuyện mới" xoá trí nhớ (`asi_reset`).
- Ngữ cảnh đầy đủ hơn: việc kèm hạn theo giờ máy, lịch hôm nay, ghi chú, mục vừa chép, ổ đĩa / pin / RAM.
- "Guided generation": mô hình vừa trả lời vừa chọn hành động (thêm việc, ghi chú, tập trung, xem lịch / việc trễ /
  tình trạng máy, dọn rác, mở tab) → app thực hiện bằng đúng lệnh có sẵn (có Hoàn tác), kèm thẻ số liệu thật.
- Trợ lý Swift tách ra `resources/asi_ai.swift`, **biên dịch sẵn lúc build** (build.rs, cần Xcode 26) và nhúng vào app;
  máy khách không cần Command Line Tools. Không còn gọi `/usr/bin/swiftc` trên máy chưa có công cụ (tránh bật hộp thoại cài Xcode).
  Có hẹn giờ 45 giây; lỗi → quay về luật.
- Luật nhận lệnh khớp theo **từ trọn vẹn** ("tre" không còn khớp "trên"); lệnh "nhắc tôi…" chỉ tính khi ở đầu câu.

## 0.12.0 (11/2026) — làm lại giao diện, cắt bớt tính năng thừa
**Giao diện mới kiểu Dynamic Island tối giản**: nền đen, chữ trắng, màu chỉ dùng để báo trạng thái; một màn hình một việc chính.
- **Peek** (rê chuột) gọn lại: 1 dòng trạng thái (bấm để mở đúng tab) · thẻ nhạc chỉ hiện khi có nhạc · 4 nút Micro / Camera / AirDrop / Tập trung.
  Bỏ hàng 7 nút chia màn hình, bỏ thanh chỉnh cỡ tai thỏ (đã có trong Cài đặt và kéo cạnh tai thỏ).
- **Tab Asi** thay trang Widget: lời chào + 3 việc/lịch gần nhất + 4 gợi ý theo ngữ cảnh; bỏ chip trạng thái trùng, bỏ 8 gợi ý cố định, bỏ dải từ mới.
- **Thanh trên** khi mở bảng chỉ còn nút VI/EN và Cài đặt (bỏ tiêu đề "Mini-Asi", ghim, đóng — bấm thanh hoặc Esc để thu gọn). ⌘1…⌘9 chuyển tab.
- **Kéo thả file**: nửa phải mở *tấm công cụ file* (Nén ZIP, Giải nén, Nén/Gộp PDF, AirDrop, Chép file) ngay trên tai thỏ — thay tab File.
- Việc / Ghi chú / Clipboard / Nhật ký / Từ mới / Threads / Dọn máy vẽ lại theo hệ thiết kế mới; nút phụ chỉ hiện khi rê chuột;
  bỏ nút chọn ngôn ngữ giọng nói lặp ở mọi ô nhập (chỉnh 1 lần trong Cài đặt); Clipboard bỏ chế độ gộp & tạo việc ít dùng.
- **Cửa sổ Cài đặt** viết lại (settings.html riêng, nhẹ): Chung · Tai thỏ · Tính năng · Nhắc nhở · Asi & giọng nói · Bộ gõ · Clipboard · Đồng bộ & sao lưu · Quyền · Pro.
- Sửa: "gân giận" bị cắt mép trên; bong bóng chat nhật ký bị bóp; chữ bị cắt cụt trong thẻ widget; thông báo chồng nhau.

**Cắt tính năng**: bảng Widget, chia màn hình (Window Snap — cũng bỏ vòng theo dõi app phía trước mỗi 65 ms), tab File, tab Sức khoẻ
(cài đặt chuyển sang Cài đặt › Nhắc nhở), cài đặt nhúng trong bảng, mục khay "Viết nhật ký", "Dọn máy", "Đóng Threads".

**Mô-đun bật/tắt** (`settings.modules`): mặc định Threads · Dọn máy · Tập trung; Nhật ký & Từ mới do người dùng tự bật
(Nhật ký tắt → không hỏi chuyện buổi tối).

**Kỹ thuật**: lệnh `patch_settings` (chỉ gửi khoá đổi → 2 cửa sổ không ghi đè nhau; có test); app.js 3.600 → ~1.100 dòng, tách
views.js / settings.js / common.js; bỏ `window_snap`, `active_widgets`, `widget_images`, `visible_tabs`.

## 0.11.0 (11/2026, làm trên Antigravity)
- Tự cập nhật từ GitHub Releases; Apple Intelligence (FoundationModels, macOS 26); điều khiển nhạc, micro, camera; AirDrop;
  chia màn hình; cửa sổ Cài đặt riêng; chỉnh cỡ tai thỏ; bảng Widget.

## 0.10.0 (11/2026)
- Cửa sổ Threads: không còn bị app khác che / tự ẩn nhầm (nhất là chế độ bong bóng); mở cạnh bong bóng; bong bóng luôn dùng cửa sổ riêng.
- Giọng đọc NghiTTS (Piper) tuỳ chọn, chỉ giọng thường: Mai Phương (nữ), Minh Quang (nam); một tiến trình đọc chung `tts_worker.py`.
- Học từ mới tiếng Anh: 2–3 từ mỗi lần mở, phiên âm US/UK, ví dụ (Free Dictionary API), nghe mẫu, ghi âm chấm phát âm.
- Tái cấu trúc backend: `lib.rs` (3.100 dòng) tách thành `commands/*` theo tính năng + `prelude.rs`; thêm AGENTS.md, docs/ARCHITECTURE.md.
- Sửa cảnh báo clippy (vòng lặp đọc dòng có thể chạy mãi khi lỗi đọc, đọc từng byte không đệm).

## 0.9.0
- Sửa thêm việc bằng giọng nói (danh sách không tải lại; đồng bộ Lời nhắc có thể xoá nhầm việc mới); nút Hoàn tác / Thêm thành việc.
- Chế độ chờ tuỳ chọn (đồng hồ, mic, số việc, ghi chú, clipboard, dung lượng…) + độ rộng tai thỏ.

## 0.8.x
- Giọng VieNeu-TTS (bản Gọn 210 MB / Chất lượng cao 520 MB); nhật ký 3 câu cảm xúc; bỏ % dung lượng; chế độ tập trung không nhấp nháy.

## 0.7.0
- 3 kiểu hiển thị (tai thỏ / viên thuốc / bong bóng); nhận diện bộ gõ bên thứ ba; hạ cửa sổ khi xin quyền; Threads cửa sổ riêng.

## ≤ 0.6
- Nền tảng: việc, ghi chú, clipboard, nhật ký, Threads, dọn máy, file, sức khoẻ, đồng bộ Apple, sao lưu Drive, lệnh giọng nói, chat với Asi.
