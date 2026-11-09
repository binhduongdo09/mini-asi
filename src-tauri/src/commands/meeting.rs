//! Lệnh ghi âm & ghi chú cuộc họp — xem `meeting.rs`.
//!
//! Luồng: `meeting_start` → luồng thu (cpal) cắt đoạn ~30–45 giây → luồng chuyển chữ (Apple Speech, ưu tiên
//! trên máy) ghi dần vào `meetings/<id>/meeting.json` và báo `meeting-update` → `meeting_stop` → chuyển chữ
//! nốt → (tuỳ chọn) tự tóm tắt bằng Apple Intelligence hoặc Gemini → báo `meeting-done`.
use crate::meeting::{self, Line, Meeting, Segment, Summary};
use crate::prelude::*;
use std::sync::atomic::{AtomicU32, Ordering};

enum Job {
    Seg(Segment),
    Done,
}

struct Session {
    id: String,
    title: String,
    rec: Option<meeting::Recorder>,
    tx: mpsc::Sender<Job>,
    pending: Arc<AtomicU32>,
    secs: Arc<Mutex<f32>>,
    system_audio: bool,
    stt: String,
}

fn session() -> &'static Mutex<Option<Session>> {
    static S: OnceLock<Mutex<Option<Session>>> = OnceLock::new();
    S.get_or_init(Default::default)
}

/// Mọi thao tác đọc-sửa-ghi file cuộc họp đi qua khoá này (luồng chuyển chữ và lệnh từ giao diện)
fn file_lock() -> &'static Mutex<()> {
    static L: OnceLock<Mutex<()>> = OnceLock::new();
    L.get_or_init(Default::default)
}

fn update(dir: &std::path::Path, id: &str, f: impl FnOnce(&mut Meeting)) -> Result<Meeting, String> {
    let _g = file_lock().lock().unwrap();
    let mut m = meeting::load(dir, id)?;
    f(&mut m);
    meeting::save(dir, &m)?;
    Ok(m)
}

#[derive(Serialize, Clone)]
pub(crate) struct MeetingLive {
    id: String,
    title: String,
    secs: f32,
    system_audio: bool,
    /// "on-device" | "server"
    stt: String,
    pending: u32,
    warn: Option<String>,
}

fn live_of(s: &Session, warn: Option<String>) -> MeetingLive {
    MeetingLive { id: s.id.clone(), title: s.title.clone(), secs: *s.secs.lock().unwrap(), system_audio: s.system_audio, stt: s.stt.clone(), pending: s.pending.load(Ordering::Relaxed), warn }
}

/// Tên mặc định: sự kiện lịch đang diễn ra (nếu có), không thì "Cuộc họp 14:05"
fn default_title(st: &AppState) -> String {
    let now = now_ms();
    st.events
        .lock()
        .unwrap()
        .iter()
        .find(|e| !e.all_day && e.start - 10 * 60_000 <= now && now <= e.end)
        .map(|e| e.title.clone())
        .unwrap_or_else(|| format!("Cuộc họp {}", Local::now().format("%H:%M %d/%m")))
}

#[tauri::command]
pub(crate) async fn meeting_start(app: AppHandle, st: S<'_>, system: Option<bool>) -> Result<MeetingLive, String> {
    if session().lock().unwrap().is_some() {
        return Err("Đang ghi một cuộc họp rồi".into());
    }
    // Xin quyền Micro + Nhận dạng giọng nói (hạ tai thỏ để hộp thoại của macOS không bị che)
    if needs_prompt(notch_native::perm::microphone()) {
        window_low(&app, true);
        let ok = tauri::async_runtime::spawn_blocking(notch_native::perm::request_microphone).await.unwrap_or(false);
        window_low(&app, false);
        if !ok {
            return Err("Chưa có quyền Micro — vào Cài đặt › Quyền cho Mini-Asi".into());
        }
    }
    if needs_prompt(notch_native::perm::speech()) {
        window_low(&app, true);
        let _ = tauri::async_runtime::spawn_blocking(notch_native::speech::ensure_access).await;
        window_low(&app, false);
    }
    let (locale, sys_default) = {
        let s = st.store.lock().unwrap();
        (s.settings.voice_lang.clone(), s.settings.meeting_system_audio)
    };
    let system = system.unwrap_or(sys_default);
    let stt = match notch_native::speech::on_device_support(&locale) {
        Some(true) => "on-device",
        Some(false) => "server",
        None if cfg!(target_os = "macos") => return Err(format!("Máy chưa hỗ trợ nhận dạng giọng nói {locale}")),
        None => "server",
    }
    .to_string();
    let id = Local::now().format("%Y%m%d-%H%M%S").to_string();
    let title = default_title(&st);
    let dir = st.data_dir.clone();
    let mut m = Meeting { id: id.clone(), title: title.clone(), started: now_ms(), locale: locale.clone(), stt: stt.clone(), ..Default::default() };
    m.system_audio = system;
    meeting::save(&dir, &m)?;

    let (tx, rx) = mpsc::channel::<Job>();
    let pending = Arc::new(AtomicU32::new(0));
    let secs = Arc::new(Mutex::new(0f32));
    // Luồng chuyển chữ: lần lượt từng đoạn
    {
        let (app, dir, id, pending, locale) = (app.clone(), dir.clone(), id.clone(), pending.clone(), locale.clone());
        let st2 = st.inner().clone();
        let on_device = stt == "on-device";
        std::thread::spawn(move || {
            // Máy đủ sức (Apple Intelligence sẵn sàng, chip Apple, đủ pin) → ghi chú ngay trong lúc họp
            let live = live_capable(&st2).is_ok();
            transcribe_loop(app, st2, dir, id, rx, pending, locale, on_device, live)
        });
    }
    // Lần đầu thu âm thanh máy: macOS hỏi quyền "Ghi âm thanh hệ thống" (bất đồng bộ) → hạ tai thỏ một lúc
    let sys_mark = dir.join(".system-audio-asked");
    if system && !sys_mark.exists() {
        let _ = std::fs::write(&sys_mark, b"");
        window_low_for(&app, 25);
    }
    let audio = meeting::dir_of(&dir, &id).join("audio.wav");
    let (a1, s1) = (app.clone(), secs.clone());
    let (tx2, p2) = (tx.clone(), pending.clone());
    let (rec, warn) = blocking(move || {
        meeting::start(
            system,
            audio,
            move |lv| {
                *s1.lock().unwrap() = lv.secs;
                let _ = a1.emit("meeting-level", (lv.secs, lv.mic, lv.sys));
            },
            move |seg| {
                p2.fetch_add(1, Ordering::Relaxed);
                let _ = tx2.send(Job::Seg(seg));
            },
        )
    })
    .await
    .inspect_err(|_| {
        let _ = std::fs::remove_dir_all(meeting::dir_of(&st.data_dir, &id));
    })?;
    if !rec.system_audio && system {
        let _ = update(&dir, &id, |m| m.system_audio = false);
    }
    let s = Session { id, title, system_audio: rec.system_audio, rec: Some(rec), tx, pending, secs, stt };
    let live = live_of(&s, warn);
    *session().lock().unwrap() = Some(s);
    let _ = app.emit("meeting-state", Some(live.clone()));
    Ok(live)
}

#[allow(clippy::too_many_arguments)]
fn transcribe_loop(app: AppHandle, st: Arc<AppState>, dir: PathBuf, id: String, rx: mpsc::Receiver<Job>, pending: Arc<AtomicU32>, locale: String, on_device: bool, mut live: bool) {
    let mdir = meeting::dir_of(&dir, &id);
    let _ = app.emit("meeting-live-mode", serde_json::json!({ "id": id, "live": live }));
    while let Ok(job) = rx.recv() {
        match job {
            Job::Seg(seg) => {
                let mut new_lines: Vec<Line> = vec![];
                let mut err = String::new();
                let mut run = |samples: &[f32], tag: &str| -> Option<String> {
                    if !meeting::has_speech(samples, meeting::RATE) {
                        return None;
                    }
                    let p = mdir.join(format!("seg-{}-{tag}.wav", seg.index));
                    meeting::write_wav16(&p, samples).ok()?;
                    let r = notch_native::speech::transcribe_file_opts(&p.to_string_lossy(), &locale, on_device);
                    let _ = std::fs::remove_file(&p);
                    match r {
                        Ok(t) if !t.trim().is_empty() => Some(t.trim().to_string()),
                        Ok(_) => None,
                        Err(e) => {
                            if !e.contains("Không nghe thấy") {
                                err = e;
                            }
                            None
                        }
                    }
                };
                let sys_text = seg.sys.as_ref().and_then(|s| run(s, "sys"));
                let mic_text = run(&seg.mic, "mic");
                let two = seg.sys.is_some();
                if let Some(t) = &sys_text {
                    new_lines.push(Line { at: seg.at, who: "others".into(), text: t.clone() });
                }
                if let Some(t) = mic_text {
                    // tiếng loa lọt vào micro → bỏ, đã có ở dòng "Người khác"
                    if !sys_text.as_deref().is_some_and(|s| meeting::is_echo(&t, s)) {
                        new_lines.insert(0, Line { at: seg.at, who: if two { "me".into() } else { String::new() }, text: t });
                    }
                }
                let left = pending.fetch_sub(1, Ordering::Relaxed).saturating_sub(1);
                let failed = !err.is_empty();
                let _ = update(&dir, &id, |m| {
                    m.lines.extend(new_lines.iter().cloned());
                    m.pending = left;
                    m.duration = m.duration.max((seg.at + seg.mic.len() as f32 / meeting::RATE as f32) as u32);
                    if failed {
                        m.failed += 1;
                        m.last_error = err.clone();
                    }
                });
                let _ = app.emit("meeting-update", serde_json::json!({ "id": id, "lines": new_lines, "pending": left, "error": if failed { Some(err) } else { None } }));
                // Ghi chú trong lúc họp: đủ ~5 phút lời nói chưa ghi chú + không bị dồn việc chuyển chữ
                if live && left <= 1 {
                    live = live_note_step(&app, &st, &dir, &id);
                }
            }
            Job::Done => {
                let _ = update(&dir, &id, |m| m.pending = 0);
                // Nén file ghi âm (song song với tóm tắt) để nghe lại nhẹ hơn
                {
                    let (dir, id, app) = (dir.clone(), id.clone(), app.clone());
                    std::thread::spawn(move || {
                        if meeting::compress_audio(&dir, &id).is_ok() {
                            let _ = app.emit("meeting-audio", &id);
                        }
                    });
                }
                let (auto, has_lines) = (st.store.lock().unwrap().settings.meeting_auto_summary, meeting::load(&dir, &id).map(|m| !m.lines.is_empty()).unwrap_or(false));
                if auto && has_lines {
                    let _ = app.emit("meeting-progress", serde_json::json!({ "id": id, "text": "Đang tóm tắt…" }));
                    let a2 = app.clone();
                    let id2 = id.clone();
                    let r = summarize(&app, &st, &id, None, true, move |t| {
                        let _ = a2.emit("meeting-progress", serde_json::json!({ "id": id2, "text": t }));
                    });
                    if let Err(e) = r {
                        let _ = app.emit("meeting-progress", serde_json::json!({ "id": id, "text": "", "error": e }));
                    }
                }
                let _ = app.emit("meeting-done", &id);
                break;
            }
        }
    }
}

#[tauri::command]
pub(crate) async fn meeting_stop(app: AppHandle, st: S<'_>) -> Result<String, String> {
    let mut s = session().lock().unwrap().take().ok_or("Không có cuộc họp nào đang ghi")?;
    let _ = app.emit("meeting-state", None::<MeetingLive>);
    let rec = s.rec.take();
    let (id, tx) = (s.id.clone(), s.tx.clone());
    let secs = *s.secs.lock().unwrap();
    let dir = st.data_dir.clone();
    let id2 = id.clone();
    blocking(move || {
        if let Some(r) = rec {
            r.finish()?; // gửi nốt đoạn cuối vào hàng đợi
        }
        let _ = update(&dir, &id2, |m| m.duration = m.duration.max(secs as u32));
        let _ = tx.send(Job::Done);
        Ok(())
    })
    .await?;
    Ok(id)
}

#[tauri::command]
pub(crate) fn meeting_status() -> Option<MeetingLive> {
    session().lock().unwrap().as_ref().map(|s| live_of(s, None))
}

#[tauri::command]
pub(crate) fn meeting_list(st: S) -> Vec<meeting::MeetingBrief> {
    meeting::list(&st.data_dir)
}

#[derive(Serialize)]
pub(crate) struct MeetingFull {
    #[serde(flatten)]
    m: Meeting,
    has_audio: bool,
    /// đường dẫn file ghi âm để giao diện phát lại (qua asset protocol); trống khi đang ghi
    audio_path: Option<String>,
    recording: bool,
    /// ghi chú trong lúc họp: có chạy được trên máy này không, và vì sao
    live_ok: bool,
    live_reason: String,
}

#[tauri::command]
pub(crate) async fn meeting_get(st: S<'_>, id: String) -> Result<MeetingFull, String> {
    let st2 = st.inner().clone();
    blocking(move || {
        let m = meeting::load(&st2.data_dir, &id)?;
        let recording = session().lock().unwrap().as_ref().is_some_and(|s| s.id == id);
        let audio = if recording { None } else { meeting::audio_file(&st2.data_dir, &id) };
        let (live_ok, live_reason) = if recording { match live_capable(&st2) { Ok(()) => (true, String::new()), Err(e) => (false, e) } } else { (!m.live.is_empty(), String::new()) };
        Ok(MeetingFull { has_audio: audio.is_some(), audio_path: audio.map(|p| p.to_string_lossy().to_string()), m, recording, live_ok, live_reason })
    })
    .await
}

#[tauri::command]
pub(crate) fn meeting_rename(app: AppHandle, st: S, id: String, title: String) -> Result<(), String> {
    let t = title.trim().chars().take(120).collect::<String>();
    if t.is_empty() {
        return Err("Tên trống".into());
    }
    update(&st.data_dir, &id, |m| m.title = t.clone())?;
    if let Some(s) = session().lock().unwrap().as_mut().filter(|s| s.id == id) {
        s.title = t;
        let _ = app.emit("meeting-state", Some(live_of(s, None)));
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn meeting_delete(st: S, id: String, audio_only: Option<bool>) -> Result<(), String> {
    if session().lock().unwrap().as_ref().is_some_and(|s| s.id == id) {
        return Err("Cuộc họp đang ghi — dừng trước đã".into());
    }
    if !meeting::valid_id(&id) {
        return Err("Mã cuộc họp không hợp lệ".into());
    }
    let d = meeting::dir_of(&st.data_dir, &id);
    let _g = file_lock().lock().unwrap();
    if audio_only.unwrap_or(false) {
        let _ = std::fs::remove_file(d.join("audio.m4a"));
        let _ = std::fs::remove_file(d.join("audio.wav"));
        Ok(())
    } else {
        std::fs::remove_dir_all(d).map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub(crate) fn meeting_reveal(st: S, id: String) -> Result<(), String> {
    if !meeting::valid_id(&id) {
        return Err("Mã cuộc họp không hợp lệ".into());
    }
    let p = meeting::audio_file(&st.data_dir, &id).ok_or("Không còn file ghi âm")?;
    files::reveal(&p);
    Ok(())
}

// ───────────────────────── tóm tắt ─────────────────────────

const APPLE_SYSTEM: &str = "Bạn là thư ký cuộc họp. Người dùng gửi một đoạn ghi chép cuộc họp (chữ chuyển từ giọng nói, có thể sai chính tả, \
mỗi dòng có mốc thời gian; \"Bạn\" là người dùng, \"Người khác\" là người dự họp khác). Viết biên bản NGẮN GỌN bằng tiếng Việt có dấu, \
đúng theo mẫu sau và không thêm gì khác:\n\
TÓM TẮT:\n- ý chính (tối đa 4 dòng)\n\
QUYẾT ĐỊNH:\n- điều đã thống nhất (nếu không có thì ghi - Không có)\n\
VIỆC CẦN LÀM:\n- việc cần làm | người phụ trách | hạn\n\
Chỉ ghi những gì có trong đoạn ghi chép. Không bịa tên người, ngày tháng, con số. Không rõ người phụ trách hoặc hạn thì ghi ?.";

const OVERVIEW_SYSTEM: &str = "Gộp các ý dưới đây (tóm tắt từng phần của một cuộc họp) thành 3–6 ý chính không trùng nhau, tiếng Việt có dấu. \
Trả lời đúng mẫu:\nTÓM TẮT:\n- ý chính\nKhông thêm ý mới.";

const GEMINI_SYSTEM: &str = "Bạn là thư ký cuộc họp. Đầu vào là toàn văn một cuộc họp (chuyển từ giọng nói, có thể sai chính tả; \
\"Bạn\" là người dùng, \"Người khác\" là người dự họp khác). Trả về DUY NHẤT một đối tượng JSON, tiếng Việt có dấu: \
{\"overview\": [3–8 ý chính], \"decisions\": [các điều đã thống nhất], \"actions\": [{\"task\": \"việc cần làm\", \"owner\": \"người phụ trách hoặc rỗng\", \"due\": \"hạn hoặc rỗng\"}]}. \
Chỉ dựa vào nội dung cuộc họp, không bịa tên người, ngày tháng, con số.";

/// Tóm tắt cuộc họp. `engine`: "apple" | "gemini" (None = theo Cài đặt). Gemini lỗi / chưa có key → tự dùng Apple Intelligence.
pub(crate) fn summarize(app: &AppHandle, st: &AppState, id: &str, engine: Option<&str>, use_live: bool, progress: impl Fn(String)) -> Result<Summary, String> {
    let m = meeting::load(&st.data_dir, id)?;
    if m.lines.is_empty() {
        return Err("Chưa có nội dung để tóm tắt".into());
    }
    let pref = engine.map(str::to_string).unwrap_or_else(|| st.store.lock().unwrap().settings.meeting_ai.clone());
    let mut note = String::new();
    let mut result: Option<Summary> = None;
    if pref == "gemini" {
        let p = llm::Provider::Gemini;
        match threads::secret::get(&p.key_name(), &st.data_dir).filter(|k| !k.trim().is_empty()) {
            None => note = "Chưa có API key Gemini — đã tóm tắt bằng Apple Intelligence.".into(),
            Some(key) => {
                progress("Gemini đang đọc toàn bộ cuộc họp…".into());
                let model = model_of(&st.store.lock().unwrap().settings, p);
                let text: String = meeting::transcript_text(&m.lines).chars().take(400_000).collect();
                let prompt = format!("Cuộc họp: {}\n\n{text}", m.title);
                match llm::ask_with_timeout(p, key.trim(), &model, GEMINI_SYSTEM, &prompt, Duration::from_secs(150)) {
                    Ok(raw) => match meeting::parse_summary_json(&raw) {
                        Some(mut s) => {
                            s.engine = format!("Gemini · {model}");
                            result = Some(s);
                        }
                        None => note = "Gemini trả về sai định dạng — đã tóm tắt bằng Apple Intelligence.".into(),
                    },
                    Err(e) => note = format!("Gemini lỗi: {e} — đã tóm tắt bằng Apple Intelligence."),
                }
            }
        }
    }
    let mut s = match result {
        Some(s) => s,
        None => {
            // Đã ghi chú trong lúc họp → chỉ tóm tắt phần còn lại rồi gộp (xong gần như ngay)
            let (done, from): (Vec<Summary>, usize) = if use_live && m.noted <= m.lines.len() { (m.live.iter().map(|l| l.summary.clone()).collect(), m.noted) } else { (vec![], 0) };
            summarize_apple(st, &m.lines[from..], done, &progress).map_err(|e| if note.is_empty() { e } else { format!("{note} Apple Intelligence cũng lỗi: {e}") })?
        }
    };
    s.note = note;
    s.at = now_ms();
    let out = s.clone();
    let m = update(&st.data_dir, id, move |m| m.summary = Some(s))?;
    save_meeting_note(app, st, &m);
    Ok(out)
}

/// Lưu / cập nhật biên bản vào thư mục "Biên bản họp" của tab Ghi chú
pub(crate) fn save_meeting_note(app: &AppHandle, st: &AppState, m: &Meeting) {
    if m.summary.is_none() {
        return;
    }
    let text = meeting::to_note_text(m, false);
    let n = st.store.lock().unwrap().upsert_meeting_note(&m.id, m.note_id, text);
    if m.note_id != Some(n.id) {
        let _ = update(&st.data_dir, &m.id, |x| x.note_id = Some(n.id));
    }
    kick_sync(app);
    let _ = app.emit("notes-changed", ());
}

/// Máy có đủ sức ghi chú trong lúc họp không (Ok) hoặc lý do (Err)
pub(crate) fn live_capable(st: &AppState) -> Result<(), String> {
    let mode = st.store.lock().unwrap().settings.meeting_live.clone();
    if mode == "after" {
        return Err("Đã chọn chỉ tóm tắt sau khi họp".into());
    }
    if !apple_ai::status(&st.data_dir).available {
        return Err("Máy chưa dùng được Apple Intelligence".into());
    }
    if mode == "live" {
        return Ok(());
    }
    if !cfg!(target_arch = "aarch64") {
        return Err("Máy chip Intel — ghi chú sau khi họp cho nhẹ máy".into());
    }
    let sys = assistant::sysinfo();
    if sys.battery_pct.is_some_and(|b| b < 25) && sys.charging != Some(true) {
        return Err("Pin yếu — ghi chú sau khi họp".into());
    }
    Ok(())
}

/// Một bước ghi chú trong lúc họp. Trả về false nếu nên ngừng ghi chú trực tiếp (máy yếu / Apple Intelligence lỗi).
fn live_note_step(app: &AppHandle, st: &AppState, dir: &std::path::Path, id: &str) -> bool {
    let Ok(m) = meeting::load(dir, id) else { return true };
    let Some((a, b)) = meeting::next_live_chunk(&m.lines, m.noted, 2200, 2600) else { return true };
    let sys = assistant::sysinfo();
    if st.store.lock().unwrap().settings.meeting_live == "auto" && sys.battery_pct.is_some_and(|p| p < 20) && sys.charging != Some(true) {
        let _ = app.emit("meeting-live-mode", serde_json::json!({ "id": id, "live": false, "reason": "Pin yếu — phần còn lại sẽ tóm tắt sau khi họp" }));
        return false;
    }
    let text = meeting::transcript_text(&m.lines[a..b]);
    match apple_ai::ask(&st.data_dir, APPLE_SYSTEM, &format!("Đoạn ghi chép:\n{text}"), false) {
        Ok(t) => {
            let s = meeting::parse_summary_text(&t.reply);
            if s.overview.is_empty() && s.actions.is_empty() && s.decisions.is_empty() {
                return true; // lần sau thử lại cùng khúc (dài hơn)
            }
            let note = meeting::LiveNote { from: m.lines[a].at, to: m.lines[b - 1].at, summary: s };
            let n2 = note.clone();
            let _ = update(dir, id, move |m| {
                m.live.push(n2);
                m.noted = b;
            });
            let _ = app.emit("meeting-live", serde_json::json!({ "id": id, "note": note }));
            true
        }
        Err(e) => {
            let _ = app.emit("meeting-live-mode", serde_json::json!({ "id": id, "live": false, "reason": format!("Apple Intelligence: {e} — sẽ tóm tắt sau khi họp") }));
            false
        }
    }
}

/// Apple Intelligence chỉ đọc được ~4096 token mỗi lần → tóm tắt từng khúc ~2200 ký tự rồi gộp
fn summarize_apple(st: &AppState, lines: &[Line], done: Vec<Summary>, progress: &impl Fn(String)) -> Result<Summary, String> {
    let parts = meeting::chunks(lines, 2200);
    let n = parts.len() + done.len();
    let mut got = done;
    let mut last_err = String::new();
    for (i, c) in parts.iter().enumerate() {
        progress(if parts.len() > 1 { format!("Apple Intelligence đang tóm tắt phần {}/{}…", i + 1, parts.len()) } else { "Apple Intelligence đang tóm tắt…".into() });
        match apple_ai::ask(&st.data_dir, APPLE_SYSTEM, &format!("Đoạn ghi chép:\n{c}"), false) {
            Ok(t) => {
                let s = meeting::parse_summary_text(&t.reply);
                if !s.overview.is_empty() || !s.actions.is_empty() || !s.decisions.is_empty() {
                    got.push(s);
                }
            }
            Err(e) => last_err = e,
        }
    }
    if got.is_empty() {
        return Err(if last_err.is_empty() { "Apple Intelligence không trả về biên bản".into() } else { last_err });
    }
    let mut s = meeting::merge(got);
    if n > 1 && s.overview.len() > 6 {
        progress("Đang gộp các phần…".into());
        let bullets: String = s.overview.iter().map(|x| format!("- {x}")).collect::<Vec<_>>().join("\n").chars().take(2600).collect();
        if let Ok(t) = apple_ai::ask(&st.data_dir, OVERVIEW_SYSTEM, &bullets, false) {
            let o = meeting::parse_summary_text(&t.reply).overview;
            if o.len() >= 2 {
                s.overview = o;
            }
        }
    }
    s.engine = "Apple Intelligence".into();
    Ok(s)
}

#[tauri::command]
pub(crate) async fn meeting_summarize(app: AppHandle, st: S<'_>, id: String, engine: Option<String>) -> Result<Summary, String> {
    let st2 = st.inner().clone();
    blocking(move || {
        let (a, i) = (app.clone(), id.clone());
        let app = app.clone();
        summarize(&app, &st2, &id, engine.as_deref(), false, move |t| {
            let _ = a.emit("meeting-progress", serde_json::json!({ "id": i, "text": t }));
        })
    })
    .await
}

/// Thêm các việc đã chọn trong biên bản vào tab Việc (hạn đọc bằng nlp: "thứ 6", "mai 9h"…)
#[tauri::command]
pub(crate) fn meeting_add_tasks(app: AppHandle, st: S, id: String, items: Vec<usize>, offset: Option<i32>) -> Result<usize, String> {
    let m = meeting::load(&st.data_dir, &id)?;
    let s = m.summary.ok_or("Chưa có biên bản")?;
    let (now, off) = local_now(offset);
    let mut n = 0;
    {
        let mut store = st.store.lock().unwrap();
        for i in items {
            let Some(a) = s.actions.get(i) else { continue };
            let p = nlp::parse(&format!("{} {}", a.task, a.due).trim().to_string(), now);
            let title = if p.title.trim().is_empty() { a.task.clone() } else { p.title };
            let t = store.add_task(title, p.due.map(|d| to_ms(d, off)));
            let mut notes = format!("Từ cuộc họp: {}", m.title);
            if !a.owner.is_empty() {
                notes.push_str(&format!("\nPhụ trách: {}", a.owner));
            }
            if !a.due.is_empty() {
                notes.push_str(&format!("\nHạn: {}", a.due));
            }
            if let Some(x) = store.task_mut(t.id) {
                x.notes = notes;
            }
            n += 1;
        }
        let _ = store.save_tasks();
    }
    push_status(&app);
    kick_sync(&app);
    Ok(n)
}

/// Mở ghi chú biên bản của cuộc họp (tạo nếu chưa có) — trả về id ghi chú để tab Ghi chú mở ra
#[tauri::command]
pub(crate) fn meeting_to_note(app: AppHandle, st: S, id: String) -> Result<u64, String> {
    let m = meeting::load(&st.data_dir, &id)?;
    if m.summary.is_none() {
        // chưa có biên bản → lưu toàn văn
        let text = meeting::to_note_text(&m, true);
        let n = st.store.lock().unwrap().upsert_meeting_note(&m.id, m.note_id, text);
        let _ = update(&st.data_dir, &id, |x| x.note_id = Some(n.id));
        kick_sync(&app);
        return Ok(n.id);
    }
    save_meeting_note(&app, &st, &m);
    meeting::load(&st.data_dir, &id)?.note_id.ok_or_else(|| "Không tạo được ghi chú".into())
}

/// Sửa biên bản (tóm tắt / quyết định / việc cần làm) — cập nhật luôn ghi chú biên bản
#[tauri::command]
pub(crate) fn meeting_update_summary(app: AppHandle, st: S, id: String, summary: Summary) -> Result<Summary, String> {
    let clean = |v: Vec<String>| v.into_iter().map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect::<Vec<_>>();
    let m = update(&st.data_dir, &id, |m| {
        let old = m.summary.take().unwrap_or_default();
        m.summary = Some(Summary {
            overview: clean(summary.overview.clone()),
            decisions: clean(summary.decisions.clone()),
            actions: summary.actions.iter().filter(|a| !a.task.trim().is_empty()).map(|a| meeting::ActionItem { task: a.task.trim().into(), owner: a.owner.trim().into(), due: a.due.trim().into() }).collect(),
            engine: if old.engine.is_empty() { "Bạn tự viết".into() } else if old.engine.ends_with("· đã sửa") { old.engine } else { format!("{} · đã sửa", old.engine) },
            at: now_ms(),
            note: String::new(),
        });
    })?;
    save_meeting_note(&app, &st, &m);
    Ok(m.summary.unwrap_or_default())
}

/// Sửa một dòng toàn văn (nhận dạng sai chính tả…); để trống = xoá dòng
#[tauri::command]
pub(crate) fn meeting_edit_line(st: S, id: String, index: usize, text: String) -> Result<(), String> {
    update(&st.data_dir, &id, |m| {
        if index < m.lines.len() {
            if text.trim().is_empty() {
                m.lines.remove(index);
                if m.noted > index {
                    m.noted -= 1;
                }
            } else {
                m.lines[index].text = text.trim().to_string();
            }
        }
    })
    .map(|_| ())
}

/// Lưu một ý quan trọng của cuộc họp thành ghi chú riêng (thư mục Biên bản họp)
#[tauri::command]
pub(crate) fn meeting_item_note(app: AppHandle, st: S, id: String, text: String) -> Result<u64, String> {
    let t = text.trim();
    if t.is_empty() {
        return Err("Nội dung trống".into());
    }
    let m = meeting::load(&st.data_dir, &id)?;
    let n = st.store.lock().unwrap().add_meeting_snippet(&id, format!("{t}\n\nTừ cuộc họp: {}", m.title));
    kick_sync(&app);
    let _ = app.emit("notes-changed", ());
    Ok(n.id)
}

/// Chép biên bản vào clipboard (Markdown)
#[tauri::command]
pub(crate) fn meeting_copy(st: S, id: String, transcript: Option<bool>) -> Result<(), String> {
    let m = meeting::load(&st.data_dir, &id)?;
    notch_native::clip::write(&notch_native::clip::ClipData::Text(meeting::to_markdown(&m, transcript.unwrap_or(false))))
}
