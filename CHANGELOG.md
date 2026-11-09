# Nhật ký thay đổi

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
