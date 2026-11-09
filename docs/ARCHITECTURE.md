# Kiến trúc Mini-Asi

## Tổng quan

```
┌──── Cửa sổ "notch" (index.html, trong suốt, luôn ở trên) ────┐  ┌── Cửa sổ "settings" (settings.html) ──┐
│ common.js · app.js: collapsed/peek/expanded/alert/listen/     │  │ common.js · settings.js               │
│   reply/drop/sheet · views.js (Asi, Việc, Ghi chú, Dọn máy)   │  │ 10 mục, lưu bằng patch_settings       │
│ extra.js (Clipboard, Nhật ký, Threads) · vocab.js · vnkey.js  │  └───────────────────────────────────────┘
└───────────────┬───────────────────────────────────────────────▲────────────────────┘
                │ invoke("lệnh", args)                           │ emit("sự kiện", dữ liệu)
┌───────────────▼───────────────────────────────────────────────┴────────────────────┐
│ src-tauri/src/lib.rs  run(): plugin, phím tắt, khay, cửa sổ, luồng nền, đăng ký lệnh │
│ commands/*.rs  lớp lệnh mỏng theo tính năng ── gọi logic thuần ↓                     │
│ store · nlp · sync · apple · clipboard · voice · journal · threads · backup · files  │
│ health · assistant · tts · vocab · license                                           │
│ crates/notch-native (API macOS) · crates/asi-clean (dọn máy)                         │
└─────────────────────────────────────────────────────────────────────────────────────┘
```

## Luồng nền (commands/status.rs)

- `background_loop` (10 giây): nhắc việc đến hạn (thông báo + sự kiện `reminder`), điểm trễ → màu đỏ,
  nghỉ mắt / uống nước, hỏi nhật ký buổi tối, đồng bộ định kỳ, đẩy `status` lên UI.
- `hover_loop` (65–350 ms tuỳ khoảng cách chuột): con trỏ vào/ra cửa sổ → sự kiện `hover` (để "ló ra" kể cả khi app không focus).
- Luồng đồng bộ Apple (`apple_sync::run_sync`) chạy khi có `kick_sync` hoặc mỗi 5 phút.

## Trạng thái & dữ liệu

`commands/state.rs::AppState` (được `app.manage`): `store` (việc, ghi chú, cài đặt — `store.rs`, JSON trong thư mục dữ liệu),
`clips`, `journal`, `vocab`, `threads` (cache), `health`, `geo` (hình học màn hình), `tts` / `nghi` (bộ đọc giọng), …
Mọi file JSON ghi kiểu "ghi file tạm rồi đổi tên" để không hỏng khi tắt máy đột ngột.

## Giao diện (ui/)

- Trạng thái hòn đảo: `collapsed` (huy hiệu theo `idle_items`) → `peek` (dòng trạng thái, nhạc, Micro/Camera/AirDrop/Tập trung)
  → `expanded` (thanh tab + `#view`). Lớp phụ: `alert`, `listen`/`reply` (lệnh giọng nói), `drop` → `sheet` (công cụ file).
- Mỗi trạng thái có kích thước riêng (`SIZES` / `sizeOf` trong app.js); `setMode` nở cửa sổ trước khi chạy hoạt cảnh, thu sau.
- Tab = `TABS` trong app.js; tab có cột mô-đun chỉ hiện khi `settings.modules` chứa id đó.

## Cửa sổ & kiểu hiển thị (commands/window.rs)

- `display_mode`: `notch` (tầng 27 – trên menu bar) · `pill` / `bubble` (tầng 3 – nổi trên cửa sổ thường).
- `place()` tính vị trí theo kiểu; UI gọi `set_frame(w,h)` mỗi khi đổi trạng thái; bong bóng kéo được (`move_bubble`).
- `set_interactive(on)`: mở bảng → focus + (nếu cần) tạm chuyển bộ gõ sang ABC để `vnkey.js` tự gõ dấu; đóng → trả bộ gõ cũ.
- `window_low()` hạ tầng cửa sổ khi macOS hiện hộp thoại xin quyền.

## Trợ lý (commands/assistant.rs + assistant.rs + apple_ai.rs)

`asi_ask(câu)`:
1. Lệnh rõ ràng ở đầu câu (`assistant::explicit_command`: "nhắc tôi…", "ghi chú…") hoặc lệnh ngắn ≤ 5 từ mà luật nhận ra
   (`assistant::classify`, khớp theo từ trọn vẹn) → `rule_reply` (offline, tức thì).
2. Còn lại, nếu bật Apple Intelligence → `apple_ai::ask(chỉ dẫn + ngữ cảnh máy, lịch sử 6 lượt + câu hỏi)` → trợ lý Swift
   trả `{reply, action, argument}` (guided generation) → `reply_from_ai`: chữ của mô hình + thẻ/nút thật của app;
   hành động ghi dữ liệu (thêm việc/ghi chú/tập trung) chạy qua đúng lệnh có sẵn, dọn rác vẫn cần bấm xác nhận.
3. Lỗi / không có Apple Intelligence → luật. Mọi lượt được nhớ trong `AppState.chat_log` (xoá bằng `asi_reset`).
Nút trên thẻ gọi `asi_action(cmd, arg)` (add_task, undo_task, add_note, clean_safe, clip_copy, copy_text).

Trợ lý Swift (`resources/asi_ai.swift`): biên dịch sẵn trong `build.rs` (Xcode 26, `-D ASI_STRUCTURED`, không được thì bản
chỉ trả lời chữ), nhúng bằng `include_bytes!`, giải nén vào thư mục dữ liệu khi chạy. Không có bản sẵn → chỉ tự biên dịch
khi `xcode-select -p` báo có công cụ (không bật hộp thoại cài Xcode trên máy khách).

## Giọng đọc (commands/speech.rs + tts.rs + resources/tts_worker.py)

- Một bộ đọc: NghiTTS (Piper) — `Tts::install(giọng)` → `uv` → Python 3.12 riêng → `piper-tts` → tải đúng giọng đã chọn
  (kiểm SHA-256) → đọc thử. Tiến trình nền `tts_worker.py` (JSON qua stdin/stdout) đọc từng câu ra WAV → `afplay`;
  rảnh 5 phút thì tắt. Chưa tải giọng → không đọc (không dùng giọng Linh). Câu tiếng Anh → `say -v Samantha`.

## Học từ mới (commands/vocab.rs + vocab.rs + ui/vocab.js)

Danh sách từ `resources/words_en.tsv` (từ, loại từ, nghĩa Việt) → `Vocab::batch(n, fresh)` xoay vòng, bỏ từ đã thuộc.
Phiên âm/audio/ví dụ: Free Dictionary API (cache trong `vocab.json`). Chấm phát âm: `dictate(..., {lang:"en-US", keep:true})`
→ `vocab_check(word, heard)` → `vocab::score` (khoảng cách sửa chữ). Nghe lại giọng mình: `vocab_play_mine`.

## Đồng bộ

- Apple: `sync::plan` (thuần, có test) quyết định PushNew/PushUpdate/PullNew/PullUpdate/DeleteLocal/DeleteRemote/Relink;
  `apple.rs` chạy JXA (`osascript -l JavaScript`). An toàn: không xoá khi Lời nhắc trả về rỗng bất thường, chỉ xoá mục đã từng thấy.
- Nhiều máy: `backup.rs` ghi zip vào thư mục đám mây (Google Drive/iCloud…), gộp theo thời gian sửa + tombstone.

## Danh sách lệnh theo module

| Module | Lệnh |
|---|---|
| window | set_display, move_bubble, get_geometry, set_frame, set_interactive, ime_info, open_settings_window |
| tasks | parse_task, add_task, list_tasks, toggle_task, snooze_task, rename_task, delete_task, clear_done |
| notes | list_notes, save_note, pin_note, delete_note |
| files | file_info, pick_files, file_zip, file_extract, file_compress_pdf, file_merge_pdf, file_airdrop, cursor_side, reveal |
| clean | clean_scan, clean_run, clean_history, open_disk_settings |
| health | health_break_done, health_snooze, focus_start, focus_stop |
| settings | get_settings, save_settings, patch_settings, license_info, activate_license, deactivate_license, quit |
| clipboard | clip_list, clip_copy, clip_merge, clip_copy_files, clip_pin, clip_delete, clip_clear, clip_image, clip_to_note, clip_to_task, accessibility_status, open_privacy |
| voice_input | voice_start, voice_stop(lang, keep), voice_cancel |
| apple_sync | sync_now, sync_info, get_events, add_to_calendar |
| journal | journal_get, journal_save, journal_delete, journal_snooze, journal_skip_today, journal_make_tasks, journal_export |
| threads | threads_get, threads_save_app, threads_login, threads_finish, threads_set_token, threads_logout, threads_replies, threads_post, threads_mark_read, threads_web, threads_embed, threads_close, threads_embed_ime, threads_embed_nav, open_link |
| permissions | permissions_status, request_permissions |
| backup | backup_info, backup_pick_dir, backup_set, backup_now, device_sync_now, backup_restore |
| assistant | asi_ask, asi_action, asi_reset, apple_ai_status |
| speech | asi_speak, asi_stop_speaking, tts_info, tts_install, tts_uninstall, tts_use |
| vocab | vocab_batch, vocab_learned, vocab_check, vocab_play, vocab_play_mine |
| status | get_status |

## Sự kiện backend → UI

`status`, `reminder`, `break`, `water`, `event-soon`, `focus-done`, `journal-ask`, `hover`, `toggle`, `open-tab`, `window-blur`,
`perm-prompt`, `ime-warn`, `voice-level`, `voice-ended`, `clip`, `synced`, `threads`, `tts-progress`, `vocab-updated`,
`settings-changed`, `settings-section`, `update-available`, `update-progress`.
