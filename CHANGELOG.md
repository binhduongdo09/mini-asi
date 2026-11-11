# Nhật ký thay đổi

## 0.21.0 (11/2026) — giao diện Liquid Glass (macOS 27), chat lưu việc / lịch / ghi chú, ghi chú họp theo Gemini hoặc Apple
**Giao diện "kính trên nền đen" theo Liquid Glass của macOS 27 Golden Gate**
- Hòn đảo vẫn đen đặc để liền tai thỏ; lớp điều khiển là kính (viền trên sáng, viền dưới tối, đục hơn bản macOS 26 cho dễ đọc):
  thanh tab thành dải kính nổi, ô thêm việc / ô tìm / thanh chat dạng viên thuốc kính, nút viên thuốc, ô nhanh ở peek.
- Thẻ và danh sách là lớp nội dung (mặt phẳng, bo góc đồng tâm 28 → 16 → 12); nội dung mờ dần ở mép cuộn thay cho vạch kẻ.
- Công tắc, nhóm chọn, thanh trượt, nút bật danh sách kiểu macOS 27. Cài đặt (luôn tối): thanh bên sát mép với ô biểu tượng màu,
  mục đang chọn tô màu nhấn, vạch ngăn thụt vào.
- Theo cài đặt Trợ năng của máy: Giảm độ trong suốt, Tăng tương phản, Giảm chuyển động. Kính vẽ bằng gradient (không làm mờ
  nền trên từng nút) nên không nặng máy.

**Chat → việc / lịch / ghi chú**
- **Thêm lịch từ chat**: "thêm lịch họp team 3h chiều mai", "đặt lịch hẹn nha sĩ thứ 6 10h", "thêm họp 9h mai vào lịch",
  "schedule a call with John tomorrow 3pm" → tạo sự kiện trong Lịch (có giờ = 1 tiếng, chỉ có ngày = cả ngày), có Hoàn tác.
  Lịch chưa dùng được → lưu thành việc có hẹn giờ và nói rõ. Thiếu ngày giờ → Asi hỏi lại, câu kế tiếp ("mai 9h") được ghép vào.
- Nội dung đứng trước nơi lưu: "lưu số anh Nam 0912… vào ghi chú", "thêm gọi khách 3h chiều vào danh sách việc".
- Trỏ về câu trước: "lưu câu trả lời trên vào ghi chú" lưu đúng câu trả lời vừa rồi; "thêm các việc trên vào danh sách" thêm
  từng gạch đầu dòng thành một việc (trước đây lưu nguyên mấy chữ "câu trả lời trên").
- "Ghi chú" trống → Asi hỏi "ghi lại điều gì?", câu kế tiếp là nội dung; trả lời "thôi" để bỏ.
- Mô hình AI có thêm hành động `add_event`; vẫn chỉ ghi khi người dùng nhờ (`user_asks_for`).

**Ghi cuộc họp**
- Một lựa chọn cho cả ghi chú trong lúc họp lẫn biên bản: chọn **Gemini** (có key) thì ghi chú trong lúc họp cũng bằng Gemini,
  không phụ thuộc sức máy; chọn **Apple Intelligence** thì mọi thứ trên máy như cũ.
- Gemini: tên mô hình đã cũ (404) → tự chọn mô hình mới nhất còn dùng được và thử lại. Gemini lỗi cuối buổi → Apple Intelligence;
  Apple cũng không chạy được nhưng đã có ghi chú trong lúc họp → gộp các ghi chú đó thành biên bản.
- Ghi chú trong lúc họp lỗi 2 lần liền mới ngừng (trước: lỗi 1 lần là ngừng, hoặc hỏi lại mãi một khúc).
- Cuộc họp nói tiếng Anh → biên bản, nhãn và ghi chú bằng tiếng Anh.
- Thoát app khi đang ghi → dừng thu đàng hoàng, không mất phần ghi âm cuối. Khởi động lại sau khi bị tắt giữa chừng → dọn
  trạng thái "đang chuyển chữ" treo, nén nốt file ghi âm. Tải lại cửa sổ khi đang ghi vẫn thấy chữ và ghi chú gần nhất.

## 0.20.0 (11/2026) — cập nhật không phải xin lại quyền, việc trễ khi rê chuột, rê lên micro để nói
**Không phải cấp lại quyền sau mỗi lần cập nhật**
- Nguyên nhân: mọi bản trước được "ký tạm" (ad-hoc) — chữ ký gắn với dấu vân tay của từng bản build, nên macOS coi mỗi
  bản mới là một app khác và hỏi lại Trợ năng, Micro, Ghi âm thanh hệ thống, Tự động hoá…
- Từ bản này mọi bản phát hành ký bằng **cùng một chứng chỉ tự ký "Mini-Asi Code Signing"** (miễn phí, hạn 20 năm).
  macOS nhận ra vẫn là app cũ → giữ nguyên quyền. Lần chuyển sang chữ ký mới vẫn hỏi quyền **một lần cuối**.
- Bộ tự cập nhật không còn ký lại app sau khi tải (việc đó làm mất chữ ký cố định); chỉ ký tạm nếu chữ ký bị hỏng.
- GitHub Actions: 2 secret `MINIASI_CODESIGN_P12`, `MINIASI_CODESIGN_PASSWORD`; tự đóng gói trên Mac: `scripts/build-mac.sh`
  tự nạp chứng chỉ từ `secrets/codesign/` vào Keychain. Thiếu chứng chỉ → ký tạm như cũ.
- Tắt hardened runtime (chỉ cần khi notarize bằng Apple Developer ID).

**Rê chuột lên tai thỏ: hiện việc trễ** — tối đa 3 việc trễ lâu nhất, mỗi việc có nút ✓ (xong) và +1h (hoãn);
bấm vào việc → mở tab Việc. Tắt ở Cài đặt › Tai thỏ › Khi rê chuột.

**Rê chuột lên nút micro trên tai thỏ = bấm micro**: giữ chuột trên nút ~0,45 giây (vòng xanh chạy quanh nút) là Asi
bắt đầu nghe; lướt qua thì không. Backend gửi toạ độ chuột (`hover-at`) vì cửa sổ không focus thì trang không nhận mousemove.
Tắt ở Cài đặt › Tai thỏ › Khi rê chuột.

## 0.19.0 (11/2026) — chat thêm việc / ghi chú chắc chắn, giao diện tiếng Anh
**Sửa lỗi: bảo Asi thêm việc / ghi chú nhưng không vào tab Việc / Ghi chú**
- Nguyên nhân: chỉ câu *mở đầu* bằng lệnh ("nhắc tôi…", "ghi chú…") mới chạy bằng quy tắc. Câu nói tự nhiên như
  "bạn thêm giúp mình task gửi báo giá…", "thêm vào danh sách việc: …", "lưu vào ghi chú: …", "note lại ý này: …" rơi xuống
  mô hình AI; mô hình trả lời "Đã thêm…" nhưng không chọn hành động (hoặc bị chặn vì không nhận ra người dùng có yêu cầu).
- `assistant::write_intent`: nhận yêu cầu thêm việc / ghi chú ở bất kỳ đâu trong câu (tiếng Việt & Anh, bỏ từ đệm
  "giúp mình", "nhé", "được không"…), lấy đúng nội dung → chạy thẳng bằng quy tắc, không qua mô hình. Câu hỏi
  ("hôm nay có việc gì", "làm sao để ghi chú…") không bị nhầm.
- Lưới an toàn: mô hình nói "đã thêm / đã lưu" mà không kèm hành động → Asi tự thêm việc / ghi chú theo yêu cầu.
- `user_asks_for` nhận thêm các cách nói tự nhiên ở trên (trước đây chặn nhầm).

**Giao diện tiếng Anh** — Cài đặt › Chung › Ngôn ngữ · Language: Tiếng Việt / English
- `ui/i18n.js` dịch giao diện khi chạy (theo dõi DOM), bản dịch ở `ui/i18n-en.js` (câu cố định + mẫu có chỗ trống "{}").
  Nội dung của người dùng (việc, ghi chú, chat, clipboard, biên bản) đánh dấu `translate="no"` nên không bị đổi.
- Menu thanh trạng thái và thông báo hệ thống (nhắc việc, nghỉ mắt, giờ họp, cảnh báo giá, Threads…) cũng theo ngôn ngữ đã chọn.
- Asi trả lời theo ngôn ngữ bạn gõ; chọn English thì Asi mặc định trả lời tiếng Anh.

## 0.18.1 (11/2026) — thanh tab kiểu thanh công cụ macOS
- Mọi tab luôn hiện đủ **biểu tượng + tên** (biểu tượng trên, tên dưới, như thanh công cụ Finder / Cài đặt hệ thống),
  mục đang chọn có nền bo góc, chia đều chiều ngang; hẹp quá mới cuộn ngang. Rê chuột hiện phím tắt ⌘1…⌘9.
- Tab Họp: các nút Sửa · Ghi chú · Chép · Finder · Xoá có cả biểu tượng và chữ.

## 0.18.0 (11/2026) — đổi định dạng file bằng kéo thả (kể cả thư mục)
Kéo file **hoặc cả thư mục** vào nửa phải tai thỏ → tấm công cụ file có thêm:
- **Đổi định dạng ảnh**: JPG · PNG · HEIC · TIFF · PDF (mỗi ảnh 1 file) · **1 PDF** (gộp mọi ảnh thành một file).
  Đọc được PNG, JPG, HEIC, TIFF, GIF, BMP, WebP. Dùng `sips` có sẵn trên macOS (giữ màu, chất lượng cao).
- **Sang PDF**: .txt .md .csv .log .json .html .docx .rtf .doc .odt → PDF khổ A4, font Unicode của máy (Arial) nên tiếng Việt
  hiển thị đúng, chọn & tìm chữ được; Markdown có tiêu đề to/đậm, gạch đầu dòng. .rtf/.doc/.odt đọc bằng `textutil` của macOS,
  .docx đọc trực tiếp. Bộ ghi PDF tự viết (`convert.rs`: nhúng font TrueType, Identity-H + ToUnicode), có test bằng qpdf/pdftotext.
- **Thư mục**: lấy mọi ảnh / file chữ bên trong (kể cả thư mục con, bỏ file ẩn, tối đa 2000 file); kết quả vào thư mục mới
  cạnh thư mục gốc, vd. "Ảnh chuyến đi - JPG", giữ nguyên cấu trúc. File lẻ → kết quả ngay cạnh file gốc, không ghi đè.
- Lệnh mới: `file_kinds`, `file_convert_images`, `file_text_pdf`.

## 0.17.0 (11/2026) — hết hỏi Keychain, nghe lại & sửa biên bản họp, cổ phiếu & cảnh báo giá, popup luôn nằm trên
**Không còn bị hỏi quyền Keychain**
- API key / token (ChatGPT, Gemini, Claude, Threads…) lưu trong file riêng của app (`secrets/`, quyền 600 — chỉ tài khoản
  của bạn đọc được, không vào bản sao lưu). Key cũ trong Keychain được chuyển ra file **một lần duy nhất** (có thể bị hỏi lần
  cuối), sau đó không bao giờ đụng Keychain nữa. Màn Cài đặt chỉ kiểm tra "đã có key" mà không đọc key.
- Nguyên nhân cũ: app tự đóng gói chưa có chữ ký cố định → mỗi bản build macOS coi là app khác, hỏi lại Keychain.

**Họp**
- **Nghe lại**: trình phát (−15 / +15 giây, 1× / 1.5× / 2×, Space), bấm một dòng toàn văn để nghe đúng đoạn đó, dòng đang
  phát được tô sáng. Sau khi họp ghi âm được nén sang m4a (~14 MB/giờ, `afconvert`). Phát qua asset protocol của Tauri.
- **Ghi chú trong lúc họp, tuỳ sức máy** (Cài đặt › Ghi cuộc họp: Tự động / Luôn / Sau khi họp): máy có Apple Intelligence,
  chip Apple, đủ pin → cứ ~5 phút lời nói Asi ghi chú phần đó ngay (hiện trong thẻ đang ghi); hết họp biên bản gộp từ các ghi
  chú nên xong gần như ngay. Máy yếu / pin yếu / Apple Intelligence lỗi / chuyển chữ bị dồn → chỉ tóm tắt sau khi họp.
- **Thư mục "Biên bản họp"** trong tab Ghi chú: biên bản tự lưu (1 ghi chú / cuộc họp, tự cập nhật khi tóm tắt lại hoặc sửa),
  có nút **Nghe lại** mở cuộc họp; trong cuộc họp có nút mở ghi chú biên bản.
- **Sửa** biên bản (tóm tắt / quyết định / việc cần làm, ⌘S để lưu) và từng dòng toàn văn (để trống = xoá).
- **Chuyển ý quan trọng** (mỗi ý trong biên bản, ghi chú trong lúc họp, mỗi dòng toàn văn) thành **Việc**, **Lịch** (bảng tạo
  sự kiện: sửa giờ, chọn lịch iCloud / Google) hoặc **Ghi chú** (thư mục Biên bản họp).

**Cổ phiếu & cảnh báo giá** (`stocks.rs`, `commands/stocks.rs`)
- Hỏi "giá FPT", "HPG với VNM thế nào", "VN-Index hôm nay" → giá khớp mới nhất, thay đổi, cao / thấp / tham chiếu / khối lượng.
  Nguồn: DNSE Entrade (miễn phí, không cần key, nến 1 phút trong phiên). (Thư viện quantvn cần API key nên không dùng.)
- "Báo cho tôi khi FPT vượt 100k hoặc giảm dưới 50k" → 2 cảnh báo; Asi kiểm tra mỗi phút trong giờ giao dịch (9:00–15:00,
  thứ 2–6), chạm ngưỡng thì báo (thông báo macOS + tai thỏ, nút Xem giá), mỗi cảnh báo báo 1 lần. "Xem cảnh báo giá",
  "Huỷ cảnh báo giá FPT".

**Popup luôn nằm trên tai thỏ**
- Quản lý tầng cửa sổ tập trung (`window.rs`): hộp thoại quyền, chọn file / thư mục, AirDrop, quyền ghi âm thanh hệ thống,
  quyền Lịch… → tai thỏ thu gọn & hạ tầng trong lúc popup hiện (đếm lồng nhau; popup không báo lúc đóng thì hạ có thời hạn, chạm
  lại tai thỏ là nâng ngay). Cửa sổ Cài đặt / Social khi đang được chọn thì nổi trên tai thỏ.

## 0.16.0 (11/2026) — ghi âm & biên bản cuộc họp
**Tab Họp mới** (luôn có, sau Ghi chú)
- Bấm "Ghi cuộc họp": thu **micro** (họp trực tiếp) + tuỳ chọn **âm thanh của máy** (giọng người khác trong Zoom / Meet /
  Teams, kể cả khi đeo tai nghe — macOS 14.6+, quyền "Ghi âm thanh hệ thống"; cpal process tap).
- Đang ghi: đồng hồ đỏ trên tai thỏ (bấm để mở), thanh âm lượng micro / máy, chữ hiện dần sau mỗi ~30–45 giây.
- Chuyển chữ bằng nhận dạng giọng nói của Apple, **ngay trên máy** nếu máy hỗ trợ tiếng Việt (không cần mạng), không thì
  qua máy chủ Apple. Cắt đoạn ở chỗ lặng nhất để không cắt giữa câu. Có 2 nguồn → biên bản chia "Bạn" / "Người khác";
  tiếng loa lọt vào micro tự bỏ.
- Dừng ghi → tự tóm tắt (tắt được): **Apple Intelligence** (mặc định, chạy trên máy; chia khúc ~2200 ký tự vì mô hình chỉ
  đọc ~4096 token rồi gộp, bỏ trùng) hoặc **Gemini** (đọc trọn cuộc họp; chưa có key / lỗi → tự dùng Apple và ghi chú lý do).
- Biên bản: Tóm tắt · Quyết định · Việc cần làm (người phụ trách, hạn) — tick chọn rồi "Thêm vào tab Việc" (hạn đọc bằng
  nlp, ghi chú "Từ cuộc họp…"); Lưu vào Ghi chú, Chép (Markdown), mở file ghi âm, đổi tên, xoá. Tên mặc định = sự kiện lịch
  đang diễn ra.
- Cài đặt › Asi & giọng nói › Ghi cuộc họp: bộ não tóm tắt (Apple / Gemini, ô API key Gemini + nút Kiểm tra ngay tại đây),
  tự tóm tắt, thu âm thanh máy.
- Dữ liệu: `meetings/<id>/meeting.json` + `audio.wav` (16 kHz mono) trong thư mục dữ liệu app, chỉ lưu trên máy.
- Mã: `meeting.rs` (thu âm, cắt đoạn, đổi tần số, đọc biên bản, có test), `commands/meeting.rs`, `ui/meeting.js`;
  `speech::transcribe_file_opts` / `on_device_support`; `llm::ask_with_timeout`. Info.plist: `NSAudioCaptureUsageDescription`.
- Giới hạn đã biết: thiết bị phát có kèm micro (tai nghe Bluetooth đang ở chế độ gọi) chưa thu được âm thanh máy → chỉ ghi
  micro và báo rõ.

## 0.15.1 (11/2026) — hết lặp câu trả lời, hết bịa thời tiết / giá vàng
**Lỗi**: Apple Intelligence (mô hình nhỏ trên máy) chép lại câu trả lời trước cho câu hỏi khác ("thị trường hôm nay"
→ trả lời thời tiết / giá vàng), nhại lại câu hỏi ("Tin công nghệ hôm nay có gì không?"), và bịa số liệu trực tiếp
(thời tiết, "giá vàng 1.650.000 VND/ounce") vì không có Internet.
- **Thông tin trực tiếp lấy từ nguồn thật, không qua mô hình** (`live.rs`, `commands/live.rs`, có test):
  thời tiết & ngày mai (Open-Meteo, nơi xem đặt trong Cài đặt › Asi & giọng nói hoặc đoán theo mạng; hỏi
  "thời tiết Đà Nẵng" để xem nơi khác), tin tức mới nhất / công nghệ / kinh doanh / thể thao / thế giới / giải trí
  (VnExpress, bấm để đọc), tỷ giá (Vietcombank). Giá vàng, chứng khoán, giá xăng, tiền số: nói thật là chưa có
  dữ liệu trực tiếp + nút "Xem trên web" — không đoán số. "Thị trường hôm nay" → hỏi lại muốn xem thị trường nào.
- **Chống lặp**: Apple Intelligence chỉ nhận lịch sử khi câu hỏi là câu nối tiếp ("còn mai thì sao", "ngắn hơn",
  "dịch…"), tối đa 2 lượt. Câu trả lời trùng câu trả lời cũ / nhại câu hỏi → hỏi lại không kèm lịch sử; vẫn lặp →
  nói thật "chưa trả lời chắc được" + nút tìm trên web (`assistant::is_repeat`, `is_follow_up`, có test).
- Chỉ dẫn cho mọi bộ não: không có Internet → không nêu thời tiết / giá / tin tức; không chép câu trả lời cũ.
- Thẻ link mới trong câu trả lời (`Card::Links`), nút `open_url`. Cài đặt mới `weather_place`.

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
