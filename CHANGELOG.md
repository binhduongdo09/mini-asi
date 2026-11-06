# Nhật ký thay đổi

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
