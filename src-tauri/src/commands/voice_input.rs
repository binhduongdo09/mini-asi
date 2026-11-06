//! Ghi âm micro → nhận dạng giọng nói (SFSpeechRecognizer trên macOS).

#[allow(unused_imports)]
use crate::prelude::*;

// ───────────────────────── Giọng nói ─────────────────────────

#[tauri::command]
pub(crate) async fn voice_start(app: AppHandle, st: S<'_>) -> Result<(), String> {
    if st.recorder.lock().unwrap().is_some() {
        return Ok(());
    }
    // Lần đầu dùng micro: hạ tai thỏ để hộp thoại xin quyền của macOS không bị che
    if needs_prompt(notch_native::perm::microphone()) {
        window_low(&app, true);
        let ok = tauri::async_runtime::spawn_blocking(notch_native::perm::request_microphone).await.unwrap_or(false);
        window_low(&app, false);
        if !ok {
            return Err("Chưa có quyền Micro — vào Cài đặt › Quyền cho Mini-Asi".into());
        }
    }
    let mut r = st.recorder.lock().unwrap();
    let a1 = app.clone();
    let a2 = app.clone();
    let rec = voice::start(
        move |level, secs| {
            let _ = a1.emit("voice-level", (level, secs));
        },
        move || {
            let _ = a2.emit("voice-ended", ());
        },
    )?;
    *r = Some(rec);
    Ok(())
}

/// Dừng thu và nhận dạng. `lang`: "vi-VN" | "en-US" (bỏ trống = theo cài đặt)
#[tauri::command]
pub(crate) async fn voice_stop(app: AppHandle, st: S<'_>, lang: Option<String>, keep: Option<bool>) -> Result<String, String> {
    if needs_prompt(notch_native::perm::speech()) {
        window_low(&app, true);
        let _ = tauri::async_runtime::spawn_blocking(notch_native::speech::ensure_access).await;
        window_low(&app, false);
    }
    let rec = st.recorder.lock().unwrap().take().ok_or("Chưa bật micro")?;
    let lang = lang.unwrap_or_else(|| st.store.lock().unwrap().settings.voice_lang.clone());
    let wav = st.data_dir.join("voice-last.wav");
    let kept = st.data_dir.join("voice-mine.wav");
    blocking(move || {
        let (samples, rate) = rec.finish()?;
        if !voice::has_speech(&samples, rate) {
            return Err("Không nghe thấy gì — thử nói gần micro hơn nhé".into());
        }
        voice::write_wav(&wav, &samples, rate)?;
        let r = notch_native::speech::transcribe_file(&wav.to_string_lossy(), &lang);
        if keep.unwrap_or(false) {
            let _ = std::fs::rename(&wav, &kept); // giữ lại để người dùng nghe lại giọng mình (học phát âm)
        } else {
            let _ = std::fs::remove_file(&wav);
        }
        r.map(|t| t.trim().to_string())
    })
    .await
}

#[tauri::command]
pub(crate) fn voice_cancel(st: S) {
    if let Some(r) = st.recorder.lock().unwrap().take() {
        std::thread::spawn(move || {
            let _ = r.finish();
        });
    }
}
