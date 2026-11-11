//! Lệnh ghi âm & ghi chú cuộc họp — xem `meeting.rs`.
//!
//! Luồng: `meeting_start` → luồng thu (cpal) cắt đoạn ~30–45 giây → luồng chuyển chữ (Apple Speech, ưu tiên
//! trên máy) ghi dần vào `meetings/<id>/meeting.json` và báo `meeting-update` → `meeting_stop` → chuyển chữ
//! nốt → (tuỳ chọn) tự tóm tắt bằng Apple Intelligence hoặc Gemini → báo `meeting-done`.
//!
//! "Bộ ghi chú" theo một lựa chọn duy nhất (Cài đặt › Ghi cuộc họp › Tóm tắt biên bản bằng):
//! - **Apple Intelligence**: mọi thứ trên máy. Ghi chú trong lúc họp nếu máy đủ sức; hết họp gộp các ghi chú + phần còn lại.
//! - **Gemini** (có API key): ghi chú trong lúc họp cũng bằng Gemini (không phụ thuộc sức máy); hết họp Gemini đọc lại
//!   toàn văn một lượt. Gemini lỗi → Apple Intelligence (nếu máy có) và ghi rõ lý do trong biên bản.
//! Biên bản viết theo ngôn ngữ của cuộc họp (ngôn ngữ nhận dạng lúc ghi).
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
        .unwrap_or_else(|| format!("{} {}", if st.store.lock().unwrap().settings.ui_lang == "en" { "Meeting" } else { "Cuộc họp" }, Local::now().format("%H:%M %d/%m")))
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
            // Ghi chú ngay trong lúc họp: Gemini (đã chọn + có key) hoặc Apple Intelligence (máy đủ sức)
            let live = live_engine(&st2);
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
fn transcribe_loop(app: AppHandle, st: Arc<AppState>, dir: PathBuf, id: String, rx: mpsc::Receiver<Job>, pending: Arc<AtomicU32>, locale: String, on_device: bool, live: Result<MeetAi, String>) {
    let mdir = meeting::dir_of(&dir, &id);
    let _ = app.emit("meeting-live-mode", live_mode_json(&id, &live));
    let mut live = live.ok();
    let mut live_fails = 0u8;
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
                // Ghi chú trong lúc họp: đủ ~3 phút lời nói chưa ghi chú + không bị dồn việc chuyển chữ
                if let Some(engine) = live.as_ref().filter(|_| left <= 1) {
                    if !live_note_step(&app, &st, &dir, &id, engine, &mut live_fails) {
                        live = None;
                    }
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
    /// "Apple Intelligence" | "Gemini" — ai đang ghi chú trong lúc họp (trống nếu không)
    live_engine: String,
}

#[tauri::command]
pub(crate) async fn meeting_get(st: S<'_>, id: String) -> Result<MeetingFull, String> {
    let st2 = st.inner().clone();
    blocking(move || {
        let m = meeting::load(&st2.data_dir, &id)?;
        let recording = session().lock().unwrap().as_ref().is_some_and(|s| s.id == id);
        let audio = if recording { None } else { meeting::audio_file(&st2.data_dir, &id) };
        let (live_ok, live_reason, live_name) = if recording {
            match live_engine(&st2) {
                Ok(e) => (true, String::new(), e.name()),
                Err(e) => (false, e, String::new()),
            }
        } else {
            (!m.live.is_empty(), String::new(), String::new())
        };
        Ok(MeetingFull { has_audio: audio.is_some(), audio_path: audio.map(|p| p.to_string_lossy().to_string()), m, recording, live_ok, live_reason, live_engine: live_name })
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

// ───────────────────────── AI cho cuộc họp ─────────────────────────
//
// Một "bộ ghi chú" (Cài đặt › Asi & giọng nói › Ghi cuộc họp, hoặc nút chọn trong từng cuộc họp) làm cả 4 việc: ghi chú
// trong lúc họp, tóm tắt, Take note (ghi chép đầy đủ), dịch. Chỉ có Apple Intelligence → không cần chọn.
// - Apple Intelligence (trên máy, ~4096 token mỗi lần): đọc từng khúc ≤ 3 phút / ≤ 2000 ký tự, cắt ở chỗ đổi người nói;
//   báo "quá dài" thì tự chia đôi khúc và thử lại. Trả lời theo mẫu chữ (mô hình nhỏ viết JSON hay hỏng).
// - ChatGPT / Gemini / Claude (API key của bạn): đọc cả cuộc họp một lượt (Take note: từng khúc ~10 phút), trả JSON.
//   Lỗi → tự dùng Apple Intelligence nếu máy có, và ghi rõ trong biên bản.

/// Ai làm việc với nội dung cuộc họp
#[derive(Clone, Debug)]
pub(crate) enum MeetAi {
    Apple,
    Cloud(llm::Provider, String),
}

impl MeetAi {
    fn name(&self) -> String {
        match self {
            MeetAi::Apple => "Apple Intelligence".into(),
            MeetAi::Cloud(p, _) => p.name().into(),
        }
    }
    fn id(&self) -> &'static str {
        match self {
            MeetAi::Apple => "apple",
            MeetAi::Cloud(p, _) => p.id(),
        }
    }
}

fn cloud_key(st: &AppState, p: llm::Provider) -> Option<String> {
    threads::secret::get(&p.key_name(), &st.data_dir).map(|k| k.trim().to_string()).filter(|k| !k.is_empty())
}

/// Key Gemini (nếu đã nhập) — Gemini là AI duy nhất nghe được file ghi âm
fn gemini_key(st: &AppState) -> Option<String> {
    cloud_key(st, llm::Provider::Gemini)
}

/// Các AI dùng được cho cuộc họp trên máy này: Apple Intelligence (nếu máy hỗ trợ) + AI đám mây đã có key
fn meet_ais(st: &AppState) -> Vec<MeetAi> {
    let mut v = vec![];
    if apple_ai::status(&st.data_dir).available {
        v.push(MeetAi::Apple);
    }
    for p in llm::ALL {
        if let Some(k) = cloud_key(st, p) {
            v.push(MeetAi::Cloud(p, k));
        }
    }
    v
}

/// Chọn AI: `engine` (nút trong cuộc họp) hoặc Cài đặt; không dùng được → Apple Intelligence → AI đầu tiên có key
fn pick_ai(st: &AppState, engine: Option<&str>) -> Result<MeetAi, String> {
    let want = engine.map(str::to_string).unwrap_or_else(|| st.store.lock().unwrap().settings.meeting_ai.clone());
    let all = meet_ais(st);
    all.iter()
        .find(|a| a.id() == want)
        .or_else(|| all.iter().find(|a| matches!(a, MeetAi::Apple)))
        .or_else(|| all.first())
        .cloned()
        .ok_or_else(|| "Máy chưa dùng được Apple Intelligence và chưa có API key AI nào — vào Cài đặt › Asi & giọng nói, hoặc bấm “Tự viết”".into())
}

#[derive(Serialize)]
pub(crate) struct MeetEngine {
    id: String,
    name: String,
    /// nghe được file ghi âm (nhận dạng lại)
    audio: bool,
}

/// AI dùng được cho cuộc họp (giao diện chỉ hiện nút chọn khi có từ 2 AI)
#[tauri::command]
pub(crate) async fn meeting_engines(st: S<'_>) -> Result<Vec<MeetEngine>, String> {
    let st2 = st.inner().clone();
    blocking(move || Ok(meet_ais(&st2).into_iter().map(|a| MeetEngine { audio: matches!(a, MeetAi::Cloud(llm::Provider::Gemini, _)), id: a.id().into(), name: a.name() }).collect())).await
}

/// Hỏi AI đám mây. Tên mô hình đang lưu đã cũ (404) → tự chọn mô hình mới nhất còn dùng được, lưu lại và hỏi lại một lần.
/// Trả về (chữ trả lời, tên mô hình).
fn cloud_ask(app: &AppHandle, st: &AppState, p: llm::Provider, key: &str, system: &str, prompt: &str, timeout: Duration) -> Result<(String, String), String> {
    let model = model_of(&st.store.lock().unwrap().settings, p);
    match llm::ask_with_timeout(p, key, &model, system, prompt, timeout) {
        Ok(raw) => Ok((raw, model)),
        Err(e) if e.contains("Không tìm thấy mô hình") => {
            let newer = refresh_model(app, st, p, key, &model).ok_or(e)?;
            Ok((llm::ask_with_timeout(p, key, &newer, system, prompt, timeout)?, newer))
        }
        Err(e) => Err(e),
    }
}

/// Đổi sang mô hình mới nhất còn dùng được với key (lưu vào Cài đặt). None nếu không có mô hình nào khác.
fn refresh_model(app: &AppHandle, st: &AppState, p: llm::Provider, key: &str, current: &str) -> Option<String> {
    let list = llm::list_models(p, key).ok()?;
    let newer = llm::pick_model(p, current, &list).filter(|m| m != current)?;
    let snap = {
        let mut s = st.store.lock().unwrap();
        match p {
            llm::Provider::OpenAi => s.settings.ai_model_openai = newer.clone(),
            llm::Provider::Gemini => s.settings.ai_model_gemini = newer.clone(),
            llm::Provider::Claude => s.settings.ai_model_claude = newer.clone(),
        }
        let _ = s.save_settings();
        s.settings.clone()
    };
    let _ = app.emit("settings-changed", &snap);
    Some(newer)
}

/// Hỏi Apple Intelligence về một đoạn toàn văn; báo "quá dài" → chia đôi (tối đa 2 lần) rồi hỏi từng nửa.
/// Trả về các (dòng đầu, dòng cuối + 1, kết quả) theo thứ tự — chỉ số tính trong `lines`.
fn apple_lines(st: &AppState, system: &str, lines: &[Line], en: bool) -> Vec<(usize, usize, Result<String, String>)> {
    fn go(st: &AppState, system: &str, lines: &[Line], en: bool, a: usize, b: usize, depth: u8, out: &mut Vec<(usize, usize, Result<String, String>)>) {
        if a >= b {
            return;
        }
        let prompt = format!("{}:\n{}", if en { "Transcript" } else { "Đoạn ghi chép" }, meeting::transcript_text_in(&lines[a..b], en));
        match apple_ai::ask(&st.data_dir, system, &prompt, false) {
            Ok(t) => out.push((a, b, Ok(t.reply))),
            Err(e) if meeting::is_too_long(&e) && b - a > 1 && depth < 2 => {
                let mid = a + (b - a) / 2;
                go(st, system, lines, en, a, mid, depth + 1, out);
                go(st, system, lines, en, mid, b, depth + 1, out);
            }
            Err(e) => out.push((a, b, Err(e))),
        }
    }
    let mut out = vec![];
    go(st, system, lines, en, 0, lines.len(), 0, &mut out);
    out
}

// ───────────────────────── lời nhắc ─────────────────────────

/// Biên bản theo mẫu chữ (Apple Intelligence). Mỗi ý kèm mốc [mm:ss] để nghe lại.
fn apple_system(en: bool) -> &'static str {
    if en {
        "You are a meeting secretary. The user sends part of a meeting transcript (speech-to-text, may contain spelling mistakes; \
each line starts with a [mm:ss] timestamp; \"You\" is the user, \"Others\" are the other attendees). Write SHORT minutes in English, exactly in this \
format and nothing else:\n\
SUMMARY:\n- [mm:ss] key point (at most 4 lines)\n\
DECISIONS:\n- [mm:ss] what was agreed (write - None if there are none)\n\
ACTION ITEMS:\n- [mm:ss] task | owner | due\n\
[mm:ss] is the timestamp of the transcript line where it was said. Only use what is in the transcript. Never invent names, dates or numbers. \
If the owner or due date is unclear write ?."
    } else {
        "Bạn là thư ký cuộc họp. Người dùng gửi một đoạn ghi chép cuộc họp (chữ chuyển từ giọng nói, có thể sai chính tả, \
mỗi dòng bắt đầu bằng mốc [mm:ss]; \"Bạn\" là người dùng, \"Người khác\" là người dự họp khác). Viết biên bản NGẮN GỌN bằng tiếng Việt có dấu, \
đúng theo mẫu sau và không thêm gì khác:\n\
TÓM TẮT:\n- [mm:ss] ý chính (tối đa 4 dòng)\n\
QUYẾT ĐỊNH:\n- [mm:ss] điều đã thống nhất (nếu không có thì ghi - Không có)\n\
VIỆC CẦN LÀM:\n- [mm:ss] việc cần làm | người phụ trách | hạn\n\
[mm:ss] là mốc của dòng ghi chép nơi ý đó được nói. Chỉ ghi những gì có trong đoạn ghi chép. Không bịa tên người, ngày tháng, con số. \
Không rõ người phụ trách hoặc hạn thì ghi ?."
    }
}

fn overview_system(en: bool) -> &'static str {
    if en {
        "Merge the points below (summaries of parts of one meeting, each with a [mm:ss] timestamp) into 3–6 key points without duplicates, in English. \
Keep the timestamp of the first related point. Answer exactly in this format:\nSUMMARY:\n- [mm:ss] key point\nDo not add new points."
    } else {
        "Gộp các ý dưới đây (tóm tắt từng phần của một cuộc họp, mỗi ý có mốc [mm:ss]) thành 3–6 ý chính không trùng nhau, tiếng Việt có dấu. \
Giữ mốc của ý liên quan xuất hiện sớm nhất. Trả lời đúng mẫu:\nTÓM TẮT:\n- [mm:ss] ý chính\nKhông thêm ý mới."
    }
}

/// Biên bản dạng JSON (AI đám mây). `part = true`: chỉ một khúc (ghi chú trong lúc họp).
fn cloud_summary_system(en: bool, part: bool) -> String {
    let (what, n) = if part { (if en { "one part of a meeting transcript" } else { "một khúc toàn văn của cuộc họp đang diễn ra" }, "1–4") } else { (if en { "the full transcript of a meeting" } else { "toàn văn một cuộc họp" }, "3–8") };
    if en {
        format!("You are a meeting secretary. The input is {what} (speech-to-text, may contain spelling mistakes; each line starts with a [mm:ss] timestamp; \"You\" is the user, \"Others\" are the other attendees). \
Return ONLY one JSON object, in English: {{\"overview\": [{n} items of {{\"text\": \"key point\", \"at\": \"mm:ss\"}}], \"decisions\": [{{\"text\": \"what was agreed\", \"at\": \"mm:ss\"}}], \"actions\": [{{\"task\": \"what to do\", \"owner\": \"who, or empty\", \"due\": \"deadline, or empty\", \"at\": \"mm:ss\"}}]}}. \
\"at\" is the timestamp of the transcript line where it was said. Use only the meeting content; never invent names, dates or numbers.")
    } else {
        format!("Bạn là thư ký cuộc họp. Đầu vào là {what} (chuyển từ giọng nói, có thể sai chính tả; mỗi dòng bắt đầu bằng mốc [mm:ss]; \"Bạn\" là người dùng, \"Người khác\" là người dự họp khác). \
Trả về DUY NHẤT một đối tượng JSON, tiếng Việt có dấu: {{\"overview\": [{n} mục {{\"text\": \"ý chính\", \"at\": \"mm:ss\"}}], \"decisions\": [{{\"text\": \"điều đã thống nhất\", \"at\": \"mm:ss\"}}], \"actions\": [{{\"task\": \"việc cần làm\", \"owner\": \"người phụ trách hoặc rỗng\", \"due\": \"hạn hoặc rỗng\", \"at\": \"mm:ss\"}}]}}. \
\"at\" là mốc của dòng ghi chép nơi ý đó được nói. Chỉ dựa vào nội dung cuộc họp, không bịa tên người, ngày tháng, con số.")
    }
}

/// Take note: viết lại toàn bộ lời nói thành ghi chép đầy đủ, KHÔNG tóm tắt
fn full_note_system(en: bool, json: bool) -> String {
    let body = if en {
        "You are a meeting note-taker. The input is a meeting transcript (speech-to-text, may contain recognition mistakes; each line starts with a [mm:ss] timestamp; \"You\" is the user, \"Others\" are the other attendees). \
Rewrite it as complete, readable notes in English: fix recognition and spelling mistakes, remove filler words (uh, um, you know), join broken sentences, \
split into paragraphs by topic or speaker and start a paragraph with the speaker (\"You:\", \"Others:\") when known. \
Do NOT summarize, do NOT drop any information, numbers or names, do NOT add anything that was not said."
    } else {
        "Bạn là thư ký ghi chép cuộc họp. Đầu vào là toàn văn cuộc họp (chuyển từ giọng nói, có thể nhận dạng sai; mỗi dòng bắt đầu bằng mốc [mm:ss]; \"Bạn\" là người dùng, \"Người khác\" là người dự họp khác). \
Viết lại thành bản ghi chép ĐẦY ĐỦ, dễ đọc, tiếng Việt có dấu: sửa lỗi nhận dạng và chính tả, bỏ từ đệm (ờ, à, ừm, kiểu như là), nối câu bị ngắt, \
chia đoạn theo ý hoặc theo người nói và ghi người nói ở đầu đoạn (\"Bạn:\", \"Người khác:\") nếu biết. \
KHÔNG tóm tắt, KHÔNG bỏ ý, con số hay tên riêng nào, KHÔNG thêm điều không được nói."
    };
    if json {
        format!("{body} Return ONLY one JSON object: {{\"paragraphs\": [{{\"at\": \"mm:ss of the first line of the paragraph\", \"text\": \"paragraph\"}}]}}.")
    } else if en {
        format!("{body} Start every paragraph with the [mm:ss] timestamp of its first transcript line, and separate paragraphs with a blank line. Output only the notes.")
    } else {
        format!("{body} Mỗi đoạn bắt đầu bằng mốc [mm:ss] của dòng ghi chép đầu tiên của đoạn, các đoạn cách nhau một dòng trống. Chỉ trả về bản ghi chép.")
    }
}

fn translate_system(to_en: bool, json: bool) -> String {
    let (from, to) = if to_en { ("Vietnamese", "English") } else { ("English", "Vietnamese (with full diacritics)") };
    if json {
        format!("Translate each item of the input JSON array from {from} into natural {to}. Keep names, numbers and dates. \
Return ONLY one JSON object: {{\"items\": [translations in the same order, same count]}}.")
    } else {
        format!("Translate each numbered line from {from} into natural {to}. Keep names, numbers and dates. \
Answer with exactly the same numbered lines (1., 2., …), one translation per line, and nothing else.")
    }
}

// ───────────────────────── tóm tắt ─────────────────────────

/// Tóm tắt cuộc họp. `engine`: id AI (None = theo Cài đặt). AI đám mây lỗi / chưa có key → tự dùng Apple Intelligence.
pub(crate) fn summarize(app: &AppHandle, st: &AppState, id: &str, engine: Option<&str>, use_live: bool, progress: impl Fn(String)) -> Result<Summary, String> {
    let m = meeting::load(&st.data_dir, id)?;
    if m.lines.is_empty() {
        return Err("Chưa có nội dung để tóm tắt".into());
    }
    let en = meeting::is_en(&m.locale);
    let ai = pick_ai(st, engine)?;
    let mut note = String::new();
    let mut result: Option<Summary> = None;
    if let MeetAi::Cloud(p, key) = &ai {
        progress(format!("{} đang đọc toàn bộ cuộc họp…", p.name()));
        let text: String = meeting::transcript_text_in(&m.lines, en).chars().take(400_000).collect();
        let prompt = format!("{}: {}\n\n{text}", if en { "Meeting" } else { "Cuộc họp" }, m.title);
        match cloud_ask(app, st, *p, key, &cloud_summary_system(en, false), &prompt, Duration::from_secs(150)) {
            Ok((raw, model)) => match meeting::parse_summary_json(&raw) {
                Some(mut s) => {
                    s.engine = format!("{} · {model}", p.name());
                    result = Some(s.fill_stamps(m.lines.first().map(|l| l.at)));
                }
                None => note = format!("{} trả về sai định dạng — đã tóm tắt bằng Apple Intelligence.", p.name()),
            },
            Err(e) => note = format!("{} lỗi: {e} — đã tóm tắt bằng Apple Intelligence.", p.name()),
        }
    } else if let Some(e) = engine.filter(|e| *e != "apple") {
        note = format!("Chưa có API key {} — đã tóm tắt bằng Apple Intelligence.", llm::Provider::from_id(e).map(|p| p.name()).unwrap_or(e));
    }
    let mut s = match result {
        Some(s) => s,
        None => {
            // Đã ghi chú trong lúc họp → chỉ tóm tắt phần còn lại rồi gộp (xong gần như ngay)
            let (done, from): (Vec<Summary>, usize) = if use_live && m.noted <= m.lines.len() { (m.live.iter().map(|l| l.summary.clone().fill_stamps(Some(l.from))).collect(), m.noted) } else { (vec![], 0) };
            let live_only = !done.is_empty();
            match summarize_apple(st, &m.lines[from..], done.clone(), en, &progress) {
                Ok(s) => s,
                // Apple Intelligence không chạy được nhưng đã có ghi chú trong lúc họp (vd. do AI đám mây ghi) → gộp các ghi chú đó
                Err(e) if live_only => {
                    let mut s = meeting::merge(done);
                    s.engine = if en { "Notes taken during the meeting".into() } else { "Ghi chú trong lúc họp".into() };
                    note = format!("{note} Phần cuối chưa được tóm tắt ({e}).").trim().to_string();
                    s
                }
                Err(e) => {
                    return Err(if !note.is_empty() {
                        format!("{note} Apple Intelligence cũng lỗi: {e}")
                    } else if !apple_ai::status(&st.data_dir).available {
                        format!("{e} — thêm API key ChatGPT / Gemini / Claude (Cài đặt › Asi & giọng nói) hoặc bấm “Tự viết” để ghi biên bản")
                    } else {
                        e
                    })
                }
            }
        }
    };
    s.note = note;
    s.at = now_ms();
    let out = s.clone();
    // bản dịch cũ không còn khớp biên bản mới
    let m = update(&st.data_dir, id, move |m| {
        m.summary = Some(s);
        m.translation = None;
    })?;
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

/// Apple Intelligence: tóm tắt từng khúc (≤ 3 phút / ≤ 2000 ký tự, cắt ở chỗ đổi người nói; quá dài thì tự chia đôi) rồi gộp
fn summarize_apple(st: &AppState, lines: &[Line], done: Vec<Summary>, en: bool, progress: &impl Fn(String)) -> Result<Summary, String> {
    let ranges = meeting::chunk_ranges(lines, 180.0, 2000);
    let n = ranges.len() + done.len();
    let mut got = done;
    let mut last_err = String::new();
    for (i, (a, b)) in ranges.iter().enumerate() {
        progress(if ranges.len() > 1 { format!("Apple Intelligence đang tóm tắt phần {}/{}…", i + 1, ranges.len()) } else { "Apple Intelligence đang tóm tắt…".into() });
        let chunk = &lines[*a..*b];
        for (i0, _, r) in apple_lines(st, apple_system(en), chunk, en) {
            let at = chunk[i0].at;
            match r {
                Ok(reply) => {
                    let s = meeting::parse_summary_text(&reply);
                    if !s.is_empty() {
                        got.push(s.fill_stamps(Some(at)));
                    }
                }
                Err(e) => last_err = e,
            }
        }
        // Apple Intelligence không chạy được (máy không hỗ trợ / chưa bật) → không thử tiếp từng khúc
        if got.is_empty() && i == 0 && ranges.len() > 1 && !last_err.is_empty() && !apple_ai::status(&st.data_dir).available {
            break;
        }
    }
    if got.is_empty() {
        return Err(if last_err.is_empty() { "Apple Intelligence không trả về biên bản".into() } else { last_err });
    }
    let mut s = meeting::merge(got);
    if n > 1 && s.overview.len() > 6 {
        progress("Đang gộp các phần…".into());
        let bullets: String = s
            .overview
            .iter()
            .zip(&s.overview_at)
            .map(|(x, at)| match at {
                Some(t) => format!("- [{}] {x}", meeting::clock(*t)),
                None => format!("- {x}"),
            })
            .collect::<Vec<_>>()
            .join("\n")
            .chars()
            .take(2600)
            .collect();
        if let Ok(t) = apple_ai::ask(&st.data_dir, overview_system(en), &bullets, false) {
            let o = meeting::parse_summary_text(&t.reply);
            if o.overview.len() >= 2 {
                s.overview = o.overview;
                s.overview_at = o.overview_at;
            }
        }
    }
    s.engine = "Apple Intelligence".into();
    Ok(s.fill_stamps(lines.first().map(|l| l.at)))
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

// ───────────────────────── Take note: ghi chép đầy đủ ─────────────────────────

/// Đọc kết quả ghi chép đầy đủ: JSON {"paragraphs":[{at,text}]} hoặc chữ "[mm:ss] đoạn…" cách nhau dòng trống
fn parse_full_parts(raw: &str, fallback: f32) -> Vec<meeting::NotePart> {
    if let (Some(a), Some(b)) = (raw.find('{'), raw.rfind('}')) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw[a..=b]) {
            if let Some(ps) = v["paragraphs"].as_array() {
                return ps
                    .iter()
                    .filter_map(|p| {
                        let (at1, text) = meeting::split_stamp(p["text"].as_str()?.trim());
                        let at = p["at"].as_str().and_then(|x| meeting::parse_clock(x.trim().trim_matches(['[', ']']))).or_else(|| p["at"].as_f64().map(|x| x as f32)).or(at1).unwrap_or(fallback);
                        (!text.trim().is_empty()).then(|| meeting::NotePart { at, text: text.trim().to_string() })
                    })
                    .collect();
            }
        }
    }
    let mut out: Vec<meeting::NotePart> = vec![];
    for block in raw.split("\n\n") {
        for line in block.lines() {
            let l = line.trim();
            if l.is_empty() {
                continue;
            }
            let (at, text) = meeting::split_stamp(l);
            match (at, out.last_mut()) {
                // dòng không có mốc trong cùng khối → nối vào đoạn trước
                (None, Some(last)) if !block.trim_start().starts_with(l) => {
                    last.text.push('\n');
                    last.text.push_str(text);
                }
                _ => out.push(meeting::NotePart { at: at.unwrap_or_else(|| out.last().map(|x| x.at).unwrap_or(fallback)), text: text.to_string() }),
            }
        }
    }
    out
}

/// Take note: viết lại toàn bộ lời nói thành ghi chép đầy đủ (không lược ý) bằng AI đã chọn, lưu vào Ghi chú › Biên bản họp.
/// Khúc nào AI không viết được → giữ nguyên văn khúc đó (không mất nội dung).
pub(crate) fn take_note(app: &AppHandle, st: &AppState, id: &str, engine: Option<&str>, progress: impl Fn(String)) -> Result<meeting::FullNote, String> {
    let m = meeting::load(&st.data_dir, id)?;
    if m.lines.is_empty() {
        return Err("Chưa có nội dung để ghi chép".into());
    }
    let en = meeting::is_en(&m.locale);
    let ai = pick_ai(st, engine)?;
    // Apple: khúc ≤ 2 phút / 1300 ký tự (vì kết quả dài gần bằng đầu vào); AI đám mây: khúc ~10 phút
    let ranges = match ai {
        MeetAi::Apple => meeting::chunk_ranges(&m.lines, 120.0, 1300),
        MeetAi::Cloud(..) => meeting::chunk_ranges(&m.lines, 600.0, 12_000),
    };
    let mut parts: Vec<meeting::NotePart> = vec![];
    let mut raw_parts = 0u32;
    let mut total = 0u32;
    let mut engine_label = ai.name();
    let mut last_err = String::new();
    for (i, (a, b)) in ranges.iter().enumerate() {
        progress(format!("{} đang viết ghi chép {}/{}…", ai.name(), i + 1, ranges.len()));
        let lines = &m.lines[*a..*b];
        let start = lines[0].at;
        // (đoạn toàn văn, kết quả AI) — Apple có thể chia đôi khúc khi quá dài
        let mut got: Vec<(&[Line], Option<Vec<meeting::NotePart>>)> = vec![];
        match &ai {
            MeetAi::Cloud(p, key) => {
                let prompt = format!("{}:\n{}", if en { "Transcript" } else { "Toàn văn" }, meeting::transcript_text_in(lines, en));
                match cloud_ask(app, st, *p, key, &full_note_system(en, true), &prompt, Duration::from_secs(180)) {
                    Ok((raw, model)) => {
                        engine_label = format!("{} · {model}", p.name());
                        got.push((lines, Some(parse_full_parts(&raw, start))));
                    }
                    Err(e) => {
                        last_err = e;
                        got.push((lines, None));
                    }
                }
            }
            MeetAi::Apple => {
                for (x, y, r) in apple_lines(st, &full_note_system(en, false), lines, en) {
                    let seg = &lines[x..y];
                    match r {
                        Ok(reply) => got.push((seg, Some(parse_full_parts(&reply, seg[0].at)))),
                        Err(e) => {
                            last_err = e;
                            got.push((seg, None));
                        }
                    }
                }
            }
        }
        for (seg, ps) in got {
            let out_chars: usize = ps.as_ref().map(|v| v.iter().map(|p| p.text.chars().count()).sum()).unwrap_or(0);
            let seg_chars = seg.iter().map(|l| l.text.chars().count()).sum::<usize>().max(1);
            total += 1;
            // không viết được hoặc ngắn bất thường (bị tóm tắt mất ý) → giữ nguyên văn đoạn đó
            match ps.filter(|v| !v.is_empty() && out_chars * 4 >= seg_chars) {
                Some(v) => parts.extend(v),
                None => {
                    raw_parts += 1;
                    let text = seg.iter().map(|l| l.text.trim()).filter(|t| !t.is_empty()).collect::<Vec<_>>().join(" ");
                    if !text.is_empty() {
                        parts.push(meeting::NotePart { at: seg[0].at, text });
                    }
                }
            }
        }
    }
    // AI không viết được đoạn nào → báo lỗi, không lưu bản "ghi chép" chỉ là nguyên văn
    if raw_parts == total && !last_err.is_empty() {
        return Err(last_err);
    }
    parts.sort_by(|a, b| a.at.partial_cmp(&b.at).unwrap_or(std::cmp::Ordering::Equal));
    let full = meeting::FullNote { parts, engine: engine_label, at: now_ms(), raw_parts };
    let f2 = full.clone();
    let m = update(&st.data_dir, id, move |m| m.full = Some(f2))?;
    let n = st.store.lock().unwrap().upsert_meeting_full_note(&m.id, m.full_note_id, meeting::full_note_text(&m));
    if m.full_note_id != Some(n.id) {
        let _ = update(&st.data_dir, &m.id, |x| x.full_note_id = Some(n.id));
    }
    kick_sync(app);
    let _ = app.emit("notes-changed", ());
    Ok(full)
}

#[tauri::command]
pub(crate) async fn meeting_take_note(app: AppHandle, st: S<'_>, id: String, engine: Option<String>) -> Result<meeting::FullNote, String> {
    if session().lock().unwrap().as_ref().is_some_and(|s| s.id == id) {
        return Err("Cuộc họp đang ghi — dừng ghi rồi bấm Take note".into());
    }
    let st2 = st.inner().clone();
    blocking(move || {
        let (a, i) = (app.clone(), id.clone());
        take_note(&app, &st2, &id, engine.as_deref(), move |t| {
            let _ = a.emit("meeting-progress", serde_json::json!({ "id": i, "text": t }));
        })
    })
    .await
}

/// Mở ghi chú "Ghi chép đầy đủ" của cuộc họp — trả về id ghi chú
#[tauri::command]
pub(crate) fn meeting_full_to_note(st: S, id: String) -> Result<u64, String> {
    let m = meeting::load(&st.data_dir, &id)?;
    if m.full.is_none() {
        return Err("Chưa có ghi chép đầy đủ — bấm Take note trước".into());
    }
    let n = st.store.lock().unwrap().upsert_meeting_full_note(&m.id, m.full_note_id, meeting::full_note_text(&m));
    if m.full_note_id != Some(n.id) {
        let _ = update(&st.data_dir, &m.id, |x| x.full_note_id = Some(n.id));
    }
    Ok(n.id)
}

// ───────────────────────── dịch biên bản ─────────────────────────

/// Dịch một danh sách câu. Apple: từng lô ≤ ~1200 ký tự theo dòng đánh số; lô lỗi → dịch từng câu; câu lỗi → giữ nguyên.
fn translate_items(app: &AppHandle, st: &AppState, ai: &MeetAi, items: &[String], to_en: bool, progress: &impl Fn(String)) -> Result<(Vec<String>, String), String> {
    if items.is_empty() {
        return Ok((vec![], ai.name()));
    }
    if let MeetAi::Cloud(p, key) = ai {
        progress(format!("{} đang dịch…", p.name()));
        let input = serde_json::to_string(items).unwrap_or_default();
        let (raw, model) = cloud_ask(app, st, *p, key, &translate_system(to_en, true), &input, Duration::from_secs(90))?;
        let v: Option<Vec<String>> = raw.find('{').and_then(|a| raw.rfind('}').map(|b| (a, b))).and_then(|(a, b)| serde_json::from_str::<serde_json::Value>(&raw[a..=b]).ok()).and_then(|v| v["items"].as_array().map(|x| x.iter().filter_map(|s| s.as_str().map(str::to_string)).collect()));
        return match v.filter(|v| v.len() == items.len()) {
            Some(v) => Ok((v, format!("{} · {model}", p.name()))),
            None => Err(format!("{} trả về bản dịch sai định dạng", p.name())),
        };
    }
    // Apple Intelligence
    let sys = translate_system(to_en, false);
    let mut out: Vec<String> = vec![];
    let mut batch: Vec<&String> = vec![];
    let mut fails = 0;
    let mut flush = |batch: &mut Vec<&String>, out: &mut Vec<String>| {
        if batch.is_empty() {
            return;
        }
        progress(format!("Apple Intelligence đang dịch {}/{}…", (out.len() + batch.len()).min(items.len()), items.len()));
        let prompt = batch.iter().enumerate().map(|(i, x)| format!("{}. {}", i + 1, x.replace('\n', " "))).collect::<Vec<_>>().join("\n");
        let ok = apple_ai::ask(&st.data_dir, &sys, &prompt, false).ok().and_then(|t| meeting::parse_numbered(&t.reply, batch.len()));
        match ok {
            Some(v) => out.extend(v),
            None => {
                // từng câu một
                for x in batch.iter() {
                    match apple_ai::ask(&st.data_dir, &sys, &format!("1. {}", x.replace('\n', " ")), false).ok().and_then(|t| meeting::parse_numbered(&t.reply, 1)) {
                        Some(mut v) => out.push(v.remove(0)),
                        None => {
                            fails += 1;
                            out.push((*x).clone());
                        }
                    }
                }
            }
        }
        batch.clear();
    };
    let mut chars = 0;
    for x in items {
        if chars + x.chars().count() > 1200 && !batch.is_empty() {
            flush(&mut batch, &mut out);
            chars = 0;
        }
        chars += x.chars().count();
        batch.push(x);
    }
    flush(&mut batch, &mut out);
    if fails == items.len() {
        return Err("Apple Intelligence chưa dịch được — thử lại hoặc chọn AI khác".into());
    }
    Ok((out, "Apple Intelligence".into()))
}

/// Dịch biên bản sang ngôn ngữ kia (họp tiếng Việt → English, họp tiếng Anh → tiếng Việt). Mốc nghe lại giữ nguyên.
#[tauri::command]
pub(crate) async fn meeting_translate(app: AppHandle, st: S<'_>, id: String, engine: Option<String>) -> Result<meeting::Translation, String> {
    let st2 = st.inner().clone();
    blocking(move || {
        let m = meeting::load(&st2.data_dir, &id)?;
        let s = m.summary.clone().ok_or("Chưa có biên bản — tóm tắt trước rồi dịch")?;
        let to_en = !meeting::is_en(&m.locale);
        let ai = pick_ai(&st2, engine.as_deref())?;
        let (a, i) = (app.clone(), id.clone());
        let progress = move |t: String| {
            let _ = a.emit("meeting-progress", serde_json::json!({ "id": i, "text": t }));
        };
        let mut items: Vec<String> = s.overview.clone();
        items.extend(s.decisions.iter().cloned());
        items.extend(s.actions.iter().map(|x| x.task.clone()));
        let (tr, label) = match translate_items(&app, &st2, &ai, &items, to_en, &progress) {
            Ok(x) => x,
            // AI đám mây lỗi → Apple Intelligence (nếu có)
            Err(e) if !matches!(ai, MeetAi::Apple) && apple_ai::status(&st2.data_dir).available => translate_items(&app, &st2, &MeetAi::Apple, &items, to_en, &progress).map_err(|e2| format!("{e} · {e2}"))?,
            Err(e) => return Err(e),
        };
        let (no, nd) = (s.overview.len(), s.decisions.len());
        let t = meeting::Translation {
            lang: if to_en { "en".into() } else { "vi".into() },
            summary: Summary {
                overview: tr[..no].to_vec(),
                overview_at: s.overview_at.clone(),
                decisions: tr[no..no + nd].to_vec(),
                decisions_at: s.decisions_at.clone(),
                actions: s.actions.iter().zip(&tr[no + nd..]).map(|(a, t)| meeting::ActionItem { task: t.clone(), ..a.clone() }).collect(),
                engine: label.clone(),
                at: now_ms(),
                note: String::new(),
            },
            engine: label,
            at: now_ms(),
        };
        let t2 = t.clone();
        let m = update(&st2.data_dir, &id, move |m| m.translation = Some(t2))?;
        save_meeting_note(&app, &st2, &m);
        Ok(t)
    })
    .await
}

// ───────────────────────── nhận dạng lại từ file ghi âm ─────────────────────────

const GEMINI_HEAR: &str = "You transcribe meeting audio verbatim. Return ONLY a JSON array: \
[{\"at\": \"mm:ss from the start of this audio\", \"who\": \"speaker label (Người 1, Người 2… or Speaker 1… in the language spoken)\", \"text\": \"exactly what was said\"}]. \
One item per sentence or short turn. Write in the language actually spoken, with correct diacritics. Do not summarize, translate or skip anything; skip silence.";

/// Nhận dạng lại toàn văn từ file ghi âm: "apple" = nhận dạng giọng nói của Apple (trên máy nếu hỗ trợ, theo ngôn ngữ đã chọn),
/// "gemini" = Gemini nghe file (phân biệt người nói, thường chính xác hơn; gửi âm thanh tới Google). Toàn văn cũ được cất
/// trong `lines.bak.json` cạnh file cuộc họp.
#[tauri::command]
pub(crate) async fn meeting_retranscribe(app: AppHandle, st: S<'_>, id: String, engine: String, locale: Option<String>) -> Result<usize, String> {
    if session().lock().unwrap().as_ref().is_some_and(|s| s.id == id) {
        return Err("Cuộc họp đang ghi — dừng ghi trước đã".into());
    }
    let st2 = st.inner().clone();
    blocking(move || {
        let m = meeting::load(&st2.data_dir, &id)?;
        let locale = locale.filter(|l| !l.trim().is_empty()).unwrap_or_else(|| m.locale.clone());
        let emit = |t: String| {
            let _ = app.emit("meeting-progress", serde_json::json!({ "id": id, "text": t }));
        };
        emit("Đang đọc file ghi âm…".into());
        let samples = meeting::load_audio_16k(&st2.data_dir, &id)?;
        let mdir = meeting::dir_of(&st2.data_dir, &id);
        let mut lines: Vec<Line> = vec![];
        let mut failed = 0u32;
        let mut last_err = String::new();
        let label;
        if engine == "gemini" {
            let key = gemini_key(&st2).ok_or("Chưa có API key Gemini — vào Cài đặt › Asi & giọng nói")?;
            let mut model = model_of(&st2.store.lock().unwrap().settings, llm::Provider::Gemini);
            // ~5 phút mỗi lần (WAV 16 kHz ≈ 9,6 MB, dưới giới hạn gửi kèm của Gemini)
            let parts = meeting::split_audio(&samples, 270.0, 300.0);
            for (k, (at, seg)) in parts.iter().enumerate() {
                emit(format!("Gemini đang nghe phần {}/{}…", k + 1, parts.len()));
                if !meeting::has_speech(seg, meeting::RATE) {
                    continue;
                }
                let p = mdir.join("re-part.wav");
                meeting::write_wav16(&p, seg)?;
                let wav = std::fs::read(&p).map_err(|e| e.to_string())?;
                let _ = std::fs::remove_file(&p);
                let prompt = format!("Language: {locale}. Meeting: {}.", m.title);
                let mut r = llm::gemini_audio(&key, &model, GEMINI_HEAR, &prompt, &wav, Duration::from_secs(240));
                if matches!(&r, Err(e) if e.contains("Không tìm thấy mô hình")) {
                    if let Some(newer) = refresh_model(&app, &st2, llm::Provider::Gemini, &key, &model) {
                        model = newer;
                        r = llm::gemini_audio(&key, &model, GEMINI_HEAR, &prompt, &wav, Duration::from_secs(240));
                    }
                }
                match r.map(|raw| meeting::parse_transcript_json(&raw, *at)) {
                    Ok(v) if !v.is_empty() => lines.extend(v),
                    Ok(_) => failed += 1,
                    Err(e) => {
                        failed += 1;
                        last_err = e;
                    }
                }
            }
            label = format!("Gemini · {model}");
        } else {
            let on_device = notch_native::speech::on_device_support(&locale) == Some(true);
            let parts = meeting::split_audio(&samples, meeting::SEG_MIN, meeting::SEG_MAX);
            for (k, (at, seg)) in parts.iter().enumerate() {
                emit(format!("Đang nhận dạng lại {}/{}…", k + 1, parts.len()));
                if !meeting::has_speech(seg, meeting::RATE) {
                    continue;
                }
                let p = mdir.join(format!("seg-re-{k}.wav"));
                meeting::write_wav16(&p, seg)?;
                let r = notch_native::speech::transcribe_file_opts(&p.to_string_lossy(), &locale, on_device);
                let _ = std::fs::remove_file(&p);
                match r {
                    Ok(t) if !t.trim().is_empty() => lines.push(Line { at: *at, who: String::new(), text: t.trim().to_string() }),
                    Ok(_) => {}
                    Err(e) if e.contains("Không nghe thấy") => {}
                    Err(e) => {
                        failed += 1;
                        last_err = e;
                    }
                }
            }
            label = format!("Apple · {locale}");
        }
        if lines.is_empty() {
            return Err(if last_err.is_empty() { "Không nhận dạng được lời nói nào trong file ghi âm".into() } else { last_err });
        }
        // cất toàn văn cũ, thay bằng toàn văn mới
        let _ = std::fs::write(mdir.join("lines.bak.json"), serde_json::to_vec_pretty(&m.lines).unwrap_or_default());
        let n = lines.len();
        update(&st2.data_dir, &id, move |m| {
            m.lines = lines;
            m.noted = 0;
            m.failed = failed;
            m.last_error = last_err;
            m.locale = locale;
            m.retranscribed = label;
        })?;
        Ok(n)
    })
    .await
}

fn live_mode_json(id: &str, live: &Result<MeetAi, String>) -> serde_json::Value {
    match live {
        Ok(e) => serde_json::json!({ "id": id, "live": true, "engine": e.name() }),
        Err(r) => serde_json::json!({ "id": id, "live": false, "why": r }),
    }
}

/// Có ghi chú được ngay trong lúc họp không, và bằng AI nào (Ok) — hoặc lý do không (Err).
/// AI đám mây (đã chọn + có key) chạy trên máy chủ nên không phụ thuộc sức máy; Apple Intelligence chạy trên máy nên
/// chế độ Tự động chỉ bật khi máy chip Apple và đủ pin.
pub(crate) fn live_engine(st: &AppState) -> Result<MeetAi, String> {
    let mode = st.store.lock().unwrap().settings.meeting_live.clone();
    if mode == "after" {
        return Err("Đã chọn chỉ tóm tắt sau khi họp".into());
    }
    let ai = pick_ai(st, None)?;
    if matches!(ai, MeetAi::Cloud(..)) || mode == "live" {
        return Ok(ai);
    }
    if !cfg!(target_arch = "aarch64") {
        return Err("Máy chip Intel — ghi chú sau khi họp cho nhẹ máy".into());
    }
    let sys = assistant::sysinfo();
    if sys.battery_pct.is_some_and(|b| b < 25) && sys.charging != Some(true) {
        return Err("Pin yếu — ghi chú sau khi họp".into());
    }
    Ok(ai)
}

/// Một bước ghi chú trong lúc họp (khi đã có ~3 phút lời nói chưa ghi chú). Trả về false = ngừng ghi chú trực tiếp,
/// phần còn lại tóm tắt sau khi họp. Lỗi / không ra biên bản 2 lần liền → ngừng (không hỏi đi hỏi lại cùng một khúc).
fn live_note_step(app: &AppHandle, st: &AppState, dir: &std::path::Path, id: &str, engine: &MeetAi, fails: &mut u8) -> bool {
    let Ok(m) = meeting::load(dir, id) else { return true };
    // ~3 phút lời nói (hoặc ≤ 2000 ký tự) mỗi lần — vừa sức Apple Intelligence; AI đám mây cũng ghi theo nhịp này
    let Some((a, b)) = meeting::next_live_chunk(&m.lines, m.noted, 180.0, 2000) else { return true };
    let stop = |reason: String| {
        let _ = app.emit("meeting-live-mode", serde_json::json!({ "id": id, "live": false, "reason": reason }));
        false
    };
    if matches!(engine, MeetAi::Apple) {
        let sys = assistant::sysinfo();
        if st.store.lock().unwrap().settings.meeting_live == "auto" && sys.battery_pct.is_some_and(|p| p < 20) && sys.charging != Some(true) {
            return stop("Pin yếu — phần còn lại sẽ tóm tắt sau khi họp".into());
        }
    }
    let en = meeting::is_en(&m.locale);
    let lines = &m.lines[a..b];
    let got: Result<Summary, String> = match engine {
        // Apple: khúc quá dài thì tự chia đôi rồi gộp
        MeetAi::Apple => {
            let mut parts = vec![];
            let mut err = None;
            for (x, _, r) in apple_lines(st, apple_system(en), lines, en) {
                match r {
                    Ok(reply) => parts.push(meeting::parse_summary_text(&reply).fill_stamps(Some(lines[x].at))),
                    Err(e) => err = Some(e),
                }
            }
            if parts.is_empty() { Err(err.unwrap_or_default()) } else { Ok(meeting::merge(parts)) }
        }
        MeetAi::Cloud(p, key) => {
            let prompt = format!("{}:\n{}", if en { "Transcript" } else { "Đoạn ghi chép" }, meeting::transcript_text_in(lines, en));
            cloud_ask(app, st, *p, key, &cloud_summary_system(en, true), &prompt, Duration::from_secs(60)).map(|(raw, _)| meeting::parse_summary_json(&raw).unwrap_or_default())
        }
    };
    match got {
        Ok(s) if !s.is_empty() => {
            *fails = 0;
            let mut s = s.fill_stamps(Some(lines[0].at));
            s.engine = engine.name();
            let note = meeting::LiveNote { from: m.lines[a].at, to: m.lines[b - 1].at, summary: s };
            let n2 = note.clone();
            let _ = update(dir, id, move |m| {
                m.live.push(n2);
                m.noted = b;
            });
            let _ = app.emit("meeting-live", serde_json::json!({ "id": id, "note": note }));
            true
        }
        other => {
            *fails += 1;
            if *fails < 2 {
                return true; // thử lại một lần ở đoạn kế tiếp
            }
            let why = match other {
                Err(e) => e,
                Ok(_) => "không đọc được kết quả".into(),
            };
            stop(format!("{}: {why} — sẽ tóm tắt sau khi họp", engine.name()))
        }
    }
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
        // giữ mốc nghe lại của các ý không đổi chữ
        let key = |s: &str| crate::nlp::fold(s.trim());
        let keep = |xs: &[String], ats: &[Option<f32>], new: &[String]| -> Vec<Option<f32>> { new.iter().map(|n| xs.iter().position(|x| key(x) == key(n)).and_then(|i| ats.get(i).copied().flatten())).collect() };
        let overview = clean(summary.overview.clone());
        let decisions = clean(summary.decisions.clone());
        m.summary = Some(Summary {
            overview_at: keep(&old.overview, &old.overview_at, &overview),
            decisions_at: keep(&old.decisions, &old.decisions_at, &decisions),
            overview,
            decisions,
            actions: summary
                .actions
                .iter()
                .filter(|a| !a.task.trim().is_empty())
                .map(|a| meeting::ActionItem { task: a.task.trim().into(), owner: a.owner.trim().into(), due: a.due.trim().into(), at: a.at.or_else(|| old.actions.iter().find(|o| key(&o.task) == key(&a.task)).and_then(|o| o.at)) })
                .collect(),
            engine: if old.engine.is_empty() { "Bạn tự viết".into() } else if old.engine.ends_with("· đã sửa") { old.engine } else { format!("{} · đã sửa", old.engine) },
            at: now_ms(),
            note: String::new(),
        });
        m.translation = None;
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

/// Lúc khởi động: dọn các cuộc họp bị bỏ dở do app tắt giữa chừng (đặt lại số đoạn chờ, xoá file đoạn tạm) và nén nốt
/// file ghi âm còn ở dạng wav.
pub(crate) fn meeting_recover(data_dir: PathBuf) {
    std::thread::spawn(move || {
        for id in meeting::recover(&data_dir) {
            let _ = meeting::compress_audio(&data_dir, &id);
        }
    });
}

/// Thoát app khi đang ghi: dừng thu cho đàng hoàng để file ghi âm được ghi đủ tới giây cuối (phần chữ của đoạn cuối
/// không kịp chuyển — nghe lại trong tab Họp).
pub(crate) fn meeting_stop_for_quit(st: &AppState) {
    let Some(mut s) = session().lock().unwrap().take() else { return };
    let secs = *s.secs.lock().unwrap();
    if let Some(r) = s.rec.take() {
        let _ = r.finish();
    }
    let _ = update(&st.data_dir, &s.id, |m| {
        m.duration = m.duration.max(secs as u32);
        m.pending = 0;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_note_parsing() {
        let t = "[00:05] Bạn: Chào mọi người, hôm nay mình chốt kế hoạch quý 4.\nNgân sách giữ nguyên.\n\n[01:10] Người khác: Bên em đề xuất chạy thử TikTok hai tuần.";
        let p = parse_full_parts(t, 0.0);
        assert_eq!(p.len(), 2);
        assert_eq!((p[0].at, p[1].at), (5.0, 70.0));
        assert!(p[0].text.ends_with("Ngân sách giữ nguyên.") && p[0].text.starts_with("Bạn:"));
        let j = parse_full_parts(r#"{"paragraphs":[{"at":"00:05","text":"You: Hello all."},{"at":70,"text":"Others: Let's test TikTok."}]}"#, 0.0);
        assert_eq!((j.len(), j[1].at, j[1].text.as_str()), (2, 70.0, "Others: Let's test TikTok."));
        // không có mốc → dùng mốc của khúc
        assert_eq!(parse_full_parts("Đoạn một.\n\nĐoạn hai.", 42.0).iter().map(|x| x.at).collect::<Vec<_>>(), vec![42.0, 42.0]);
    }
}
