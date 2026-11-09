//! Lệnh "thả vào tai thỏ → việc / lịch": đọc nội dung vừa thả (chữ, link, ảnh, danh bạ, file ảnh/vcf/txt),
//! đọc chữ trong ảnh (Vision, trên máy), đề xuất việc/sự kiện (quy tắc + AI nếu có), rồi tạo theo lựa chọn
//! của người dùng — sự kiện vào lịch đã chọn (iCloud / Google / … qua app Lịch của Mac).
use crate::capture::{Contact, Draft};
use crate::prelude::*;
use serde::Deserialize;
use base64::Engine as _;

/// Nội dung của lần thả gần nhất (giữ lại để phân tích / đọc ảnh sau khi bảng đã hiện)
#[derive(Default)]
struct Captured {
    text: String,
    url: Option<String>,
    image: Option<Vec<u8>>,
    contacts: Vec<Contact>,
    /// Chữ đọc được từ ảnh (đọc 1 lần)
    ocr: Option<String>,
}

fn last() -> &'static Mutex<Captured> {
    static L: OnceLock<Mutex<Captured>> = OnceLock::new();
    L.get_or_init(Default::default)
}

const IMG_EXT: &[&str] = &["png", "jpg", "jpeg", "heic", "heif", "tif", "tiff", "gif", "webp", "bmp"];
const TXT_EXT: &[&str] = &["txt", "md", "markdown", "text", "rtf"];
const MAX_IMG: u64 = 30 * 1024 * 1024;

#[derive(Serialize, Default)]
pub(crate) struct DropInfo {
    /// "text" | "url" | "image" | "contact" | "none"
    kind: String,
    /// Chữ / link (rút gọn) để xem trước
    preview: String,
    /// Ảnh xem trước (data URL) — chỉ khi ảnh không quá lớn
    image: Option<String>,
    contacts: Vec<String>,
}

fn ext_of(p: &str) -> String {
    std::path::Path::new(p).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

/// File thả vào có tạo việc/lịch được không (ảnh, danh bạ .vcf, chữ .txt/.md) — để bảng file hiện thêm nút
#[tauri::command]
pub(crate) fn capture_supports(paths: Vec<String>) -> bool {
    paths.len() == 1 && {
        let e = ext_of(&paths[0]);
        IMG_EXT.contains(&e.as_str()) || TXT_EXT.contains(&e.as_str()) || e == "vcf"
    }
}

fn from_paths(paths: &[String]) -> Captured {
    let mut c = Captured::default();
    for p in paths.iter().take(5) {
        let e = ext_of(p);
        let size = std::fs::metadata(p).map(|m| m.len()).unwrap_or(u64::MAX);
        if IMG_EXT.contains(&e.as_str()) && c.image.is_none() && size <= MAX_IMG {
            c.image = std::fs::read(p).ok();
        } else if e == "vcf" && size <= 2 * 1024 * 1024 {
            if let Ok(s) = std::fs::read_to_string(p) {
                c.contacts.extend(capture::parse_vcard(&s));
            }
        } else if TXT_EXT.contains(&e.as_str()) && size <= 256 * 1024 {
            if let Ok(s) = std::fs::read_to_string(p) {
                c.text.push_str(&s);
            }
        }
    }
    c
}

fn from_drag(d: notch_native::clip::DragData) -> Captured {
    let mut c = if d.files.is_empty() { Captured::default() } else { from_paths(&d.files) };
    if let Some(v) = d.vcard.as_deref() {
        c.contacts.extend(capture::parse_vcard(v));
    }
    if let Some(t) = d.text {
        if c.text.is_empty() {
            c.text = t;
        }
    }
    if let Some(u) = d.url {
        // kéo link từ trình duyệt: chữ thường chính là link → để trống phần chữ
        if c.text.trim() == u.trim() {
            c.text.clear();
        }
        c.url = Some(u);
    }
    if c.image.is_none() {
        c.image = d.image_png;
    }
    c
}

fn info(c: &Captured) -> DropInfo {
    let kind = if !c.contacts.is_empty() {
        "contact"
    } else if c.image.is_some() {
        "image"
    } else if !c.text.trim().is_empty() {
        "text"
    } else if c.url.is_some() {
        "url"
    } else {
        "none"
    };
    let preview: String = if !c.text.trim().is_empty() { c.text.trim().chars().take(400).collect() } else { c.url.clone().unwrap_or_default() };
    let image = c.image.as_ref().filter(|b| b.len() <= 6 * 1024 * 1024).map(|b| {
        let mime = if b.starts_with(&[0x89, b'P', b'N', b'G']) {
            "image/png"
        } else if b.starts_with(&[0xFF, 0xD8]) {
            "image/jpeg"
        } else if b.starts_with(b"GIF8") {
            "image/gif"
        } else {
            "image/*"
        };
        format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(b))
    });
    DropInfo { kind: kind.into(), preview, image, contacts: c.contacts.iter().map(|x| x.name.clone()).collect() }
}

/// Đọc thứ vừa thả vào tai thỏ. `paths` rỗng → đọc bảng kéo-thả của macOS (chữ chọn, link, ảnh web, danh bạ…).
#[tauri::command]
pub(crate) fn drop_read(paths: Vec<String>) -> DropInfo {
    let c = if paths.is_empty() {
        match notch_native::clip::read_drag() {
            Some(d) => from_drag(d),
            None => Captured::default(),
        }
    } else {
        from_paths(&paths)
    };
    let i = info(&c);
    *last().lock().unwrap() = c;
    i
}

/// Dùng chữ người dùng dán/nhập (hoặc mục trong lịch sử chép) làm nội dung để tạo việc/lịch
#[tauri::command]
pub(crate) fn capture_set_text(text: String) -> DropInfo {
    let c = Captured { text, ..Default::default() };
    let i = info(&c);
    *last().lock().unwrap() = c;
    i
}

/// Dùng một mục trong lịch sử chép (chữ hoặc ảnh) làm nội dung để tạo việc/lịch
#[tauri::command]
pub(crate) fn capture_from_clip(st: S, id: u64) -> Result<DropInfo, String> {
    let c = {
        let clips = st.clips.lock().unwrap();
        let clip = clips.get(id).ok_or("Không tìm thấy mục đã chép")?;
        match clip.kind {
            clipboard::ClipKind::Image => Captured { image: clips.image_bytes(id), ..Default::default() },
            clipboard::ClipKind::Files => from_paths(&clip.files),
            _ => Captured { text: clip.text.clone(), ..Default::default() },
        }
    };
    let i = info(&c);
    *last().lock().unwrap() = c;
    Ok(i)
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub(crate) struct DraftView {
    kind: String,
    title: String,
    /// unix ms
    start: Option<i64>,
    end: Option<i64>,
    all_day: bool,
    location: String,
    notes: String,
    #[serde(default)]
    engine: String,
    /// Chữ đọc được trong ảnh (để người dùng xem)
    #[serde(default)]
    ocr: String,
    #[serde(default)]
    calendar_id: String,
}

fn view(d: Draft, off: i64, ocr: String) -> DraftView {
    DraftView {
        kind: d.kind,
        title: d.title,
        start: d.start.map(|x| to_ms(x, off)),
        end: d.end.map(|x| to_ms(x, off)),
        all_day: d.all_day,
        location: d.location,
        notes: d.notes,
        engine: d.engine,
        ocr,
        calendar_id: String::new(),
    }
}

/// Hỏi bộ não đã chọn để trích thông tin (trả JSON). Lỗi → None (giữ đề xuất theo quy tắc).
fn ai_extract(st: &AppState, text: &str, now: chrono::NaiveDateTime) -> Option<Draft> {
    let system = capture::ai_system(now);
    let body: String = text.chars().take(4000).collect();
    let prompt = format!("Nội dung:\n{body}");
    let (prov, label) = brain(st);
    let raw = match prov {
        None => {
            if !st.store.lock().unwrap().settings.apple_ai_enabled {
                return None;
            }
            let t = apple_ai::ask(&st.data_dir, &system, &prompt, false).ok()?;
            if t.reply.contains('{') { t.reply } else { t.argument }
        }
        Some(p) => {
            let model = model_of(&st.store.lock().unwrap().settings, p);
            let key = threads::secret::get(&p.key_name(), &st.data_dir).filter(|k| !k.trim().is_empty())?;
            llm::ask(p, key.trim(), &model, &system, &prompt).ok()?
        }
    };
    let notes: String = text.trim().chars().take(capture::MAX_NOTES).collect();
    capture::parse_ai(&raw, &notes, &label)
}

/// Phân tích nội dung vừa thả → đề xuất việc/sự kiện. `ai = false` → chỉ quy tắc (tức thì);
/// `ai = true` → đọc chữ trong ảnh (nếu có) + nhờ AI, chậm hơn vài giây.
#[tauri::command]
pub(crate) async fn capture_analyze(st: S<'_>, ai: bool, offset: Option<i32>) -> Result<DraftView, String> {
    let st2 = st.inner().clone();
    blocking(move || {
        let (now, off) = local_now(offset);
        // Đọc chữ trong ảnh (1 lần) — chỉ khi được yêu cầu vì mất ~1 giây
        let need_ocr = { let l = last().lock().unwrap(); l.image.is_some() && l.ocr.is_none() };
        if need_ocr && ai {
            let img = last().lock().unwrap().image.clone().unwrap_or_default();
            let t = notch_native::ocr::recognize_text(&img).unwrap_or_default();
            last().lock().unwrap().ocr = Some(t);
        }
        let (text, url, contact, ocr) = {
            let l = last().lock().unwrap();
            let ocr = l.ocr.clone().unwrap_or_default();
            let text = [l.text.trim(), ocr.trim()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join("\n");
            (text, l.url.clone(), l.contacts.first().cloned(), ocr)
        };
        if let Some(c) = contact.filter(|_| text.trim().is_empty()) {
            return Ok(view(capture::from_contact(&c, now), off, ocr));
        }
        if text.trim().is_empty() && url.is_none() {
            let has_img = last().lock().unwrap().image.is_some();
            return Err(if has_img && ai { "Không thấy chữ nào trong ảnh — hãy tự nhập tiêu đề".into() } else { "Không có nội dung để tạo".into() });
        }
        let mut d = capture::from_text(&text, url.as_deref(), now);
        if ai && !text.trim().is_empty() {
            if let Some(a) = ai_extract(&st2, &text, now) {
                d = a;
                if let Some(u) = url.as_deref() {
                    if !d.notes.contains(u) {
                        d.notes = format!("{}\n{u}", d.notes).trim().to_string();
                    }
                }
            }
        }
        Ok(view(d, off, ocr))
    })
    .await
}

#[derive(Serialize)]
pub(crate) struct CalendarView {
    id: String,
    title: String,
    source: String,
    default: bool,
}

/// Các lịch ghi được trên máy (iCloud, Google, Exchange…) + lịch đã chọn lần trước
#[tauri::command]
pub(crate) async fn calendar_list() -> Result<Vec<CalendarView>, String> {
    blocking(|| {
        Ok(notch_native::calendar::calendars()?
            .into_iter()
            .map(|c| CalendarView { id: c.id, title: c.title, source: c.source, default: c.default })
            .collect())
    })
    .await
}

#[derive(Serialize)]
pub(crate) struct CaptureResult {
    task_id: Option<u64>,
    event: bool,
    message: String,
}

/// Tạo việc hoặc sự kiện từ đề xuất (người dùng đã xem & sửa). Sự kiện vào lịch `calendar_id`
/// (lưu lại làm lựa chọn mặc định lần sau).
#[tauri::command]
pub(crate) async fn capture_create(app: AppHandle, st: S<'_>, d: DraftView) -> Result<CaptureResult, String> {
    let title = d.title.trim().to_string();
    if title.is_empty() {
        return Err("Chưa có tiêu đề".into());
    }
    if d.kind == "event" {
        let start = d.start.ok_or("Sự kiện cần ngày giờ bắt đầu")?;
        let end = d.end.filter(|e| *e > start).unwrap_or(start + if d.all_day { 86_400_000 } else { 3_600_000 });
        {
            let mut s = st.store.lock().unwrap();
            if s.settings.capture_calendar != d.calendar_id {
                s.settings.capture_calendar = d.calendar_id.clone();
                let _ = s.save_settings();
            }
        }
        let st2 = st.inner().clone();
        let ev = notch_native::calendar::NewEvent {
            title: title.clone(),
            start_ms: start,
            end_ms: end,
            all_day: d.all_day,
            location: d.location.trim().to_string(),
            notes: d.notes.trim().to_string(),
            calendar_id: d.calendar_id.clone(),
        };
        blocking(move || {
            notch_native::calendar::create_event_full(&ev)?;
            let _ = refresh_events(&st2);
            Ok(())
        })
        .await?;
        push_status(&app);
        return Ok(CaptureResult { task_id: None, event: true, message: format!("Đã thêm vào lịch: {title}") });
    }
    let t = {
        let mut s = st.store.lock().unwrap();
        let mut t = s.add_task(title.clone(), d.start);
        let notes = d.notes.trim();
        if !notes.is_empty() {
            if let Some(x) = s.task_mut(t.id) {
                x.notes = notes.chars().take(capture::MAX_NOTES).collect();
                t = x.clone();
            }
            let _ = s.save_tasks();
        }
        t
    };
    push_status(&app);
    kick_sync(&app);
    Ok(CaptureResult { task_id: Some(t.id), event: false, message: format!("Đã thêm việc: {title}") })
}
