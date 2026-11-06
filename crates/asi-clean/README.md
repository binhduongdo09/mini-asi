# asi-clean — bộ máy dọn rác của Mini-Asi

Thư viện Rust kèm CLI: quét và dọn rác **an toàn** cho macOS (Linux và Windows có khung sườn).
Sau này app Tauri gọi trực tiếp vào `asi_clean::Engine`; CLI dùng để thử và debug.

> Bộ luật được tự soạn từ kiến thức công khai về cấu trúc thư mục macOS. Cách làm học theo các dự án
> Mole (GPL-3.0) và Pearcleaner (Commons Clause), nhưng **không sao chép code** của chúng, nên được
> phép dùng trong bản thương mại.

## 5 nguyên tắc an toàn

1. **Quét không bao giờ thay đổi ổ đĩa.** Lệnh `clean` mặc định chỉ xem trước; phải thêm `--yes` mới xoá thật.
2. **Cache tự tạo lại được thì xoá hẳn**, vì xoá vào Thùng rác sẽ không giải phóng dung lượng.
   **File của người dùng thì luôn vào Thùng rác.** Luật nhóm `files` khai báo xoá hẳn sẽ bị từ chối ngay khi nạp.
3. **Guard kiểm tra 2 lần**, lúc quét và ngay trước khi xoá:
   - Chặn mọi đường dẫn ngoài thư mục home, có `..`, nằm trong `.git`, hoặc có thư mục cha là symlink.
   - Chặn vùng cấm: Keychain, iCloud Drive, CloudStorage, Photos, Messages, Mail, `.ssh`...
   - Chặn whitelist do người dùng tự đặt.
4. **Bỏ qua mục đang được app sử dụng** (có file thay đổi trong N phút gần đây) và **dự án có code mới sửa trong 7 ngày**.
   Thư mục build đã commit vào git cũng được giữ lại.
5. **Ghi nhật ký mọi thao tác** (JSON Lines) để xem lịch sử và làm nền cho tính năng hoàn tác.

## Free và Pro

| | Free | Pro |
|---|---|---|
| Cache app, trình duyệt (có Cốc Cốc), log, Xcode DerivedData, cache npm/pip/brew | ✅ | ✅ |
| Dọn Thùng rác | ✅ | ✅ |
| File cài đặt .dmg/.pkg, ảnh chụp màn hình cũ, Downloads cũ, file > 1 GB | 🔒 | ✅ |
| `node_modules`/`target`... của dự án cũ, Xcode Archives, iOS DeviceSupport | 🔒 | ✅ |
| Bản sao lưu iPhone, file đính kèm Mail | 🔒 | ✅ |

Bản Free **vẫn quét cả luật Pro** để hiện dòng *"✨ Bản Pro dọn thêm được 13.9 GB"*. Con số dung lượng cụ thể
của chính máy người dùng là lý do thuyết phục nhất để họ nâng cấp.

## Chạy thử

```bash
cargo build --release
./target/release/asi-clean status          # tình trạng ổ đĩa + biểu cảm
./target/release/asi-clean rules           # danh sách luật
./target/release/asi-clean scan -v         # quét, xem chi tiết
./target/release/asi-clean clean           # xem trước các luật an toàn
./target/release/asi-clean clean --yes     # dọn thật
./target/release/asi-clean clean --rule files-installers --yes   # dọn 1 luật (cần Pro)
./target/release/asi-clean history
MINIASI_TIER=pro ./target/release/asi-clean scan   # giả lập giấy phép Pro
```

Mọi lệnh đều có `--json` để UI đọc. `MINIASI_HOME=/thu-muc-gia` chạy trên một home giả lập, còn
`--os macos` nạp bộ luật macOS khi đang ở máy khác (dùng để kiểm thử).

## Cấu hình người dùng: `~/.config/mini-asi/clean.toml`

```toml
whitelist = ["~/Library/Caches/com.spotify.client"]   # không bao giờ dọn
disabled_rules = ["system-saved-state"]
project_roots = ["~/Work"]                            # nơi tìm node_modules...
# rules_file = "~/.config/mini-asi/rules.toml"        # luật cập nhật qua mạng
```

## Cấu trúc

```
rules/macos.toml  linux.toml  windows.toml   ← bộ luật (thêm luật không cần sửa code)
src/guard.rs     vùng cấm, kiểm tra đường dẫn
src/scan.rs      quét, tính dung lượng thực (block), chống đếm trùng giữa các luật
src/clean.rs     thực thi: xoá hẳn hoặc vào Thùng rác
src/disk.rs      dung lượng trống → mức ok / warn (<15%) / critical (<5%) cho linh vật
src/oplog.rs     nhật ký
tests/engine.rs  10 bài test (vùng cấm, symlink, git, whitelist, Free/Pro, dry-run)
```

## Việc tiếp theo

- [ ] Chạy thử chế độ chỉ quét (`scan`) trên Mac thật để chỉnh lại luật.
- [ ] Gỡ app kèm file sót lại (đọc `CFBundleIdentifier`) và tìm file mồ côi của app đã gỡ (Pro).
- [ ] Snapshot cục bộ của Time Machine (`tmutil`) và dung lượng "purgeable".
- [ ] Cache Zalo, Telegram, Messenger, phổ biến với người dùng Việt (cần khảo sát đường dẫn trên máy thật).
- [ ] Tìm file trùng lặp bằng hash (Pro).
- [ ] Hoàn tác: khôi phục từ Thùng rác dựa vào nhật ký.
- [ ] Xác thực giấy phép Pro bằng key ký số (hiện chỉ giả lập bằng biến môi trường).
- [ ] Lấy dung lượng trống trên Windows.
