//! Asi đọc to bằng giọng tải thêm chạy trên máy (Piper — xem tts.rs). Chưa tải giọng thì Asi chỉ hiện chữ,
//! không dùng giọng tiếng Việt có sẵn của macOS (kém tự nhiên). Câu tiếng Anh dùng giọng tiếng Anh của máy.

#[allow(unused_imports)]
use crate::prelude::*;

pub(crate) static SAY_PID: Mutex<Option<u32>> = Mutex::new(None);

/// Bỏ emoji & ký hiệu markdown cho giọng đọc tự nhiên hơn
pub(crate) fn speakable(text: &str) -> String {
    text.chars().filter(|c| c.is_alphanumeric() || c.is_whitespace() || ",.;:!?%()/-–“”\"'…".contains(*c)).collect()
}

#[tauri::command]
pub(crate) fn asi_speak(st: S, text: String) {
    let (on, lang, voice) = {
        let s = st.store.lock().unwrap();
        (s.settings.voice_reply, s.settings.voice_lang.clone(), s.settings.tts_voice.clone())
    };
    stop_all_speaking(&st);
    let clean = speakable(&text);
    if !on || clean.trim().is_empty() {
        return;
    }
    if looks_english(&text, &lang) {
        system_say(&clean, true); // giọng tiếng Anh của macOS (Samantha) đọc tiếng Anh tốt
        return;
    }
    st.tts.speak(&clean, &voice);
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

/// Giọng của hệ điều hành — chỉ dùng cho tiếng Anh (đọc từ vựng, câu trả lời tiếng Anh)
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
    if let Some(pid) = SAY_PID.lock().unwrap().take() {
        tts::kill_pid(pid);
    }
}

#[tauri::command]
pub(crate) fn asi_stop_speaking(st: S) {
    stop_all_speaking(&st);
}

#[derive(Serialize)]
pub(crate) struct TtsVoice {
    pub(crate) name: String,
    pub(crate) gender: String,
    pub(crate) desc: String,
    pub(crate) downloaded: bool,
}

#[derive(Serialize)]
pub(crate) struct TtsInfo {
    pub(crate) status: tts::Status,
    /// giọng đang chọn
    pub(crate) voice: String,
    pub(crate) voices: Vec<TtsVoice>,
    /// dung lượng tải lần đầu (bộ đọc + 1 giọng) và mỗi giọng thêm (MB)
    pub(crate) first_mb: u32,
    pub(crate) voice_mb: u32,
}

#[tauri::command]
pub(crate) fn tts_info(st: S) -> TtsInfo {
    let voice = st.store.lock().unwrap().settings.tts_voice.clone();
    TtsInfo {
        status: st.tts.status(),
        voice: st.tts.usable_voice(&voice),
        voices: tts::VOICES
            .iter()
            .map(|(n, g, d, _)| TtsVoice { name: n.to_string(), gender: g.to_string(), desc: d.to_string(), downloaded: st.tts.has_downloaded(n) })
            .collect(),
        first_mb: tts::BASE_MB + tts::VOICE_MB,
        voice_mb: tts::VOICE_MB,
    }
}

/// Tải bộ đọc (nếu chưa có) + giọng `voice`. Xong thì dùng luôn giọng đó.
#[tauri::command]
pub(crate) fn tts_install(app: AppHandle, st: S, voice: Option<String>) -> Result<(), String> {
    let v = tts::voice_or_default(voice.as_deref().unwrap_or(tts::DEFAULT_VOICE)).to_string();
    let h = app.clone();
    let chosen = v.clone();
    st.tts.install(&chosen, move |s| {
        if s.phase == "ready" {
            if let Some(st) = h.try_state::<Arc<AppState>>() {
                let snap = {
                    let mut g = st.store.lock().unwrap();
                    g.settings.tts_engine = tts::ENGINE_ID.into();
                    g.settings.tts_voice = v.clone();
                    g.settings.voice_reply = true;
                    let _ = g.save_settings();
                    g.settings.clone()
                };
                let _ = h.emit("settings-changed", &snap);
            }
        }
        let _ = h.emit("tts-progress", serde_json::json!({ "status": s }));
    });
    Ok(())
}

#[tauri::command]
pub(crate) fn tts_uninstall(app: AppHandle, st: S) -> Result<(), String> {
    stop_all_speaking(&st);
    st.tts.uninstall()?;
    let snap = {
        let mut g = st.store.lock().unwrap();
        g.settings.voice_reply = false;
        let _ = g.save_settings();
        g.settings.clone()
    };
    let _ = app.emit("settings-changed", &snap);
    Ok(())
}

/// Chọn giọng (đã tải) và/hoặc nghe thử
#[tauri::command]
pub(crate) fn tts_use(st: S, voice: String, preview: bool) -> Result<(), String> {
    let v = tts::voice_or_default(&voice).to_string();
    if !st.tts.has_downloaded(&v) {
        return Err(format!("Chưa tải giọng {v}"));
    }
    {
        let mut g = st.store.lock().unwrap();
        g.settings.tts_engine = tts::ENGINE_ID.into();
        g.settings.tts_voice = v.clone();
        let _ = g.save_settings();
    }
    if preview {
        stop_all_speaking(&st);
        st.tts.speak("Chào bạn, mình là Asi. Hôm nay có gì cần mình giúp không?", &v);
    } else {
        st.tts.ensure_worker();
    }
    Ok(())
}
