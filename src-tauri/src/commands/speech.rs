//! Asi đọc to: giọng máy (say) hoặc bộ đọc tải thêm (VieNeu / NghiTTS, xem tts.rs).

#[allow(unused_imports)]
use crate::prelude::*;

pub(crate) static SAY_PID: Mutex<Option<u32>> = Mutex::new(None);

/// Asi đọc to: bằng bộ đọc đã tải (VieNeu / NghiTTS), chưa có hoặc đang nạp thì dùng giọng của máy
#[tauri::command]
pub(crate) fn asi_speak(st: S, text: String) {
    let (on, lang, engine, voice) = {
        let s = st.store.lock().unwrap();
        (s.settings.voice_reply, s.settings.voice_lang.clone(), s.settings.tts_engine.clone(), s.settings.tts_voice.clone())
    };
    stop_all_speaking(&st);
    if !on || text.trim().is_empty() {
        return;
    }
    // bỏ emoji cho giọng đọc tự nhiên hơn
    let clean: String = text.chars().filter(|c| c.is_alphanumeric() || c.is_whitespace() || ",.;:!?%()/-–“”\"'…".contains(*c)).collect();
    if let Some(e) = tts::Engine::from_id(&engine) {
        if tts_pack(&st, e).speak(&clean, voice_for(e, &voice)) {
            return;
        }
    }
    system_say(&clean, looks_english(&text, &lang) || lang.starts_with("en"));
}

pub(crate) fn tts_pack(st: &AppState, e: tts::Engine) -> &Arc<tts::Tts> {
    match e {
        tts::Engine::VieNeu => &st.tts,
        tts::Engine::Nghi => &st.nghi,
    }
}

pub(crate) fn voice_for(e: tts::Engine, v: &str) -> &str {
    if e.has_voice(v) { v } else { e.default_voice() }
}

pub(crate) fn track_audio_child(mut child: std::process::Child) {
    let pid = child.id();
    *SAY_PID.lock().unwrap() = Some(pid);
    std::thread::spawn(move || {
        let _ = child.wait();
        let mut g = SAY_PID.lock().unwrap();
        if *g == Some(pid) {
            *g = None;
        }
    });
}

pub(crate) fn system_say(clean: &str, en: bool) {
    #[cfg(target_os = "macos")]
    let child = std::process::Command::new("say").args(["-v", if en { "Samantha" } else { "Linh" }, clean]).spawn();
    #[cfg(not(target_os = "macos"))]
    let child = std::process::Command::new("spd-say").args(["-l", if en { "en" } else { "vi" }, clean]).spawn();
    if let Ok(c) = child {
        track_audio_child(c);
    }
}

pub(crate) fn stop_all_speaking(st: &AppState) {
    st.tts.stop();
    st.nghi.stop();
    if let Some(pid) = SAY_PID.lock().unwrap().take() {
        tts::kill_pid(pid);
    }
}

#[tauri::command]
pub(crate) fn asi_stop_speaking(st: S) {
    stop_all_speaking(&st);
}

#[derive(Serialize)]
pub(crate) struct TtsPackInfo {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) status: tts::Status,
    /// (tên, giới tính, mô tả)
    pub(crate) voices: Vec<(String, String, String)>,
    /// dung lượng mô hình ước lượng (MB) cho bản mặc định
    pub(crate) model_mb: u32,
    pub(crate) blurb: String,
}

#[derive(Serialize)]
pub(crate) struct TtsInfo {
    /// system | vieneu | nghi
    pub(crate) engine: String,
    pub(crate) voice: String,
    pub(crate) packs: Vec<TtsPackInfo>,
    /// máy Intel cũ có thể chậm hơn
    pub(crate) arm: bool,
    /// bản của VieNeu: (mã, tên, dung lượng MB)
    pub(crate) qualities: Vec<(String, String, u32)>,
    pub(crate) default_quality: String,
}

#[tauri::command]
pub(crate) fn tts_info(st: S) -> TtsInfo {
    let s = st.store.lock().unwrap();
    let pack = |e: tts::Engine, blurb: &str| TtsPackInfo {
        id: e.id().into(),
        name: e.name().into(),
        status: tts_pack(&st, e).status(),
        voices: e.voices().iter().map(|(a, b, c)| (a.to_string(), b.to_string(), c.to_string())).collect(),
        model_mb: e.model_mb(tts::default_quality()),
        blurb: blurb.into(),
    };
    TtsInfo {
        engine: s.settings.tts_engine.clone(),
        voice: s.settings.tts_voice.clone(),
        packs: vec![
            pack(tts::Engine::VieNeu, "Tự nhiên nhất, 48 kHz · Apache-2.0"),
            pack(tts::Engine::Nghi, "Nhẹ & nhanh nhất, 22 kHz · giọng thường (không dùng giọng người nổi tiếng)"),
        ],
        arm: cfg!(target_arch = "aarch64"),
        qualities: tts::QUALITIES.iter().map(|(a, b, c)| (a.to_string(), b.to_string(), *c)).collect(),
        default_quality: tts::default_quality().into(),
    }
}

#[tauri::command]
pub(crate) fn tts_install(app: AppHandle, st: S, engine: Option<String>, quality: Option<String>) -> Result<(), String> {
    let e = tts::Engine::from_id(engine.as_deref().unwrap_or("vieneu")).ok_or("Bộ đọc lạ")?;
    let h = app.clone();
    tts_pack(&st, e).install(quality.as_deref().unwrap_or(tts::default_quality()), move |s| {
        if s.phase == "ready" {
            // tải xong → dùng luôn bộ đọc này với giọng mặc định của nó
            if let Some(st) = h.try_state::<Arc<AppState>>() {
                let mut g = st.store.lock().unwrap();
                g.settings.tts_engine = e.id().into();
                if !e.has_voice(&g.settings.tts_voice) {
                    g.settings.tts_voice = e.default_voice().into();
                }
                let _ = g.save_settings();
            }
        }
        let _ = h.emit("tts-progress", serde_json::json!({ "engine": e.id(), "status": s }));
    });
    Ok(())
}

#[tauri::command]
pub(crate) fn tts_uninstall(st: S, engine: Option<String>) -> Result<(), String> {
    let e = tts::Engine::from_id(engine.as_deref().unwrap_or("vieneu")).ok_or("Bộ đọc lạ")?;
    {
        let mut g = st.store.lock().unwrap();
        if g.settings.tts_engine == e.id() {
            g.settings.tts_engine = "system".into();
        }
        let _ = g.save_settings();
    }
    tts_pack(&st, e).uninstall()
}

/// Chọn giọng & nghe thử. engine: system | vieneu | nghi
#[tauri::command]
pub(crate) fn tts_use(st: S, engine: String, voice: String, preview: bool) -> bool {
    let e = tts::Engine::from_id(&engine);
    {
        let mut g = st.store.lock().unwrap();
        g.settings.tts_engine = e.map_or("system", |e| e.id()).into();
        if let Some(e) = e {
            g.settings.tts_voice = voice_for(e, &voice).into();
        }
        let _ = g.save_settings();
    }
    let Some(e) = e else {
        if preview {
            stop_all_speaking(&st);
            system_say("Chào bạn, mình là Asi. Hôm nay bạn thấy thế nào?", false);
        }
        return true;
    };
    if !preview {
        tts_pack(&st, e).ensure_worker();
        return true;
    }
    stop_all_speaking(&st);
    let male = e.voices().iter().any(|v| v.0 == voice && v.1 == "male");
    let line = if male { "Chào bạn, mình là Asi. Hôm nay bạn thấy thế nào?" } else { "Chào bạn, mình là Asi nè. Hôm nay có gì vui không?" };
    tts_pack(&st, e).speak(line, voice_for(e, &voice)) // false = đang nạp mô hình, UI báo đợi chút
}
