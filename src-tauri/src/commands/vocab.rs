//! Học từ mới tiếng Anh (lệnh cho ui/vocab.js, logic ở vocab.rs).

#[allow(unused_imports)]
use crate::prelude::*;

// ───────────────────────── Học từ mới tiếng Anh ─────────────────────────

#[derive(Serialize)]
pub(crate) struct VocabView {
    pub(crate) cards: Vec<vocab::Card>,
    pub(crate) learned: usize,
    pub(crate) total: usize,
}

pub(crate) fn vocab_view(st: &AppState, words: &[String]) -> VocabView {
    let v = st.vocab.lock().unwrap();
    let (learned, total) = v.stats();
    VocabView { cards: words.iter().filter_map(|w| v.card(w)).collect(), learned, total }
}

/// Bộ từ đang học. `fresh` = vừa mở tai thỏ → đổi sang 2–3 từ mới. Từ chưa có phiên âm được tra online ở nền.
#[tauri::command]
pub(crate) fn vocab_batch(app: AppHandle, st: S, fresh: bool) -> VocabView {
    let n = st.store.lock().unwrap().settings.vocab_count.clamp(1, 5) as usize;
    let words = st.vocab.lock().unwrap().batch(n, fresh, now_ms());
    let missing: Vec<String> = { let v = st.vocab.lock().unwrap(); words.iter().filter(|w| v.needs_fetch(w)).cloned().collect() };
    if !missing.is_empty() {
        let h = app.clone();
        std::thread::spawn(move || {
            for w in missing {
                if let Ok(d) = vocab::fetch(&w, now_ms()) {
                    if let Some(st) = h.try_state::<Arc<AppState>>() {
                        let mut v = st.vocab.lock().unwrap();
                        v.state.cache.insert(w.clone(), d);
                        v.save();
                    }
                    let _ = h.emit("vocab-updated", w);
                }
            }
        });
    }
    vocab_view(&st, &words)
}

#[tauri::command]
pub(crate) fn vocab_learned(st: S, word: String, learned: bool) {
    st.vocab.lock().unwrap().set_learned(&word, learned);
}

/// Chấm phát âm: so từ cần đọc với câu máy nghe được (tiếng Anh)
#[tauri::command]
pub(crate) fn vocab_check(st: S, word: String, heard: String) -> vocab::Check {
    let c = vocab::score(&word, &heard);
    st.vocab.lock().unwrap().record(&word, &c);
    c
}

/// Phát âm mẫu: file chuẩn US/UK (tải & lưu cache), không có thì để máy đọc
#[tauri::command]
pub(crate) async fn vocab_play(st: S<'_>, word: String, accent: Option<String>) -> Result<(), String> {
    let uk = accent.as_deref() == Some("uk");
    let url = {
        let v = st.vocab.lock().unwrap();
        v.card(&word).and_then(|c| if uk { c.details.audio_uk.or(c.details.audio_us) } else { c.details.audio_us.or(c.details.audio_uk) })
    };
    let dir = st.data_dir.join("vocab-audio");
    stop_all_speaking(&st);
    blocking(move || {
        if let Some(url) = url.filter(|u| u.starts_with("https://")) {
            let f = dir.join(format!("{}-{}.mp3", word.to_lowercase(), if uk { "uk" } else { "us" }));
            if !f.exists() {
                let _ = std::fs::create_dir_all(&dir);
                let resp = ureq::get(&url).timeout(Duration::from_secs(8)).call().map_err(|e| e.to_string())?;
                let mut buf = vec![];
                std::io::Read::read_to_end(&mut resp.into_reader(), &mut buf).map_err(|e| e.to_string())?;
                std::fs::write(&f, buf).map_err(|e| e.to_string())?;
            }
            if play_audio_file(&f) {
                return Ok(());
            }
        }
        system_say(&word, true);
        Ok(())
    })
    .await
}

/// Nghe lại giọng mình vừa đọc
#[tauri::command]
pub(crate) fn vocab_play_mine(st: S) -> Result<(), String> {
    let f = st.data_dir.join("voice-mine.wav");
    if !f.exists() {
        return Err("Chưa có bản ghi — bấm 🎙 Đọc thử trước nhé".into());
    }
    stop_all_speaking(&st);
    play_audio_file(&f);
    Ok(())
}

pub(crate) fn play_audio_file(f: &std::path::Path) -> bool {
    #[cfg(target_os = "macos")]
    let c = std::process::Command::new("afplay").arg(f).spawn();
    #[cfg(target_os = "windows")]
    let c = std::process::Command::new("powershell").args(["-NoProfile", "-c", &format!("Add-Type -AssemblyName presentationCore; $p = New-Object System.Windows.Media.MediaPlayer; $p.Open('{}'); $p.Play(); Start-Sleep 3", f.display())]).spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let c = std::process::Command::new("sh").args(["-c", "ffplay -nodisp -autoexit -loglevel quiet \"$0\" 2>/dev/null || mpg123 -q \"$0\" 2>/dev/null || paplay \"$0\"", &f.to_string_lossy()]).spawn();
    match c {
        Ok(c) => {
            track_audio_child(c);
            true
        }
        Err(_) => false,
    }
}
