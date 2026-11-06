//! Lịch sử clipboard (chữ, ảnh, file) và dán nhanh.

#[allow(unused_imports)]
use crate::prelude::*;

// ───────────────────────── Clipboard ─────────────────────────

#[tauri::command]
pub(crate) fn clip_list(st: S, query: Option<String>, kind: Option<String>) -> Vec<clipboard::ClipView> {
    st.clips.lock().unwrap().list(query.as_deref().unwrap_or(""), kind.as_deref().unwrap_or("all"))
}

/// Chép lại một mục vào clipboard. `paste` = dán luôn vào app đang dùng (cần quyền Trợ năng).
/// Trả về true nếu đã tự dán.
#[tauri::command]
pub(crate) fn clip_copy(app: AppHandle, st: S, id: u64, paste: bool) -> Result<bool, String> {
    let data = st.clips.lock().unwrap().data(id).ok_or("Mục này không còn")?;
    if let notch_native::clip::ClipData::Files(f) = &data {
        if f.is_empty() {
            return Err("Các file này đã bị xoá hoặc di chuyển".into());
        }
    }
    notch_native::clip::write(&data)?;
    st.clips.lock().unwrap().mark_used(id, now_ms());
    if paste && notch_native::clip::accessibility_trusted() {
        let _ = app.run_on_main_thread(notch_native::deactivate_app);
        std::thread::spawn(|| {
            std::thread::sleep(Duration::from_millis(350));
            notch_native::clip::send_paste();
        });
        return Ok(true);
    }
    Ok(false)
}

/// Gộp nhiều mục chữ thành một (mỗi mục một dòng) rồi chép
#[tauri::command]
pub(crate) fn clip_merge(st: S, ids: Vec<u64>) -> Result<(), String> {
    let c = st.clips.lock().unwrap();
    let parts: Vec<String> = ids.iter().filter_map(|id| c.get(*id)).filter(|x| x.kind == clipboard::ClipKind::Text).map(|x| x.text.clone()).collect();
    drop(c);
    if parts.is_empty() {
        return Err("Chọn ít nhất 1 mục chữ".into());
    }
    notch_native::clip::write(&notch_native::clip::ClipData::Text(parts.join("\n")))
}

#[tauri::command]
pub(crate) fn clip_copy_files(paths: Vec<String>) -> Result<(), String> {
    notch_native::clip::write(&notch_native::clip::ClipData::Files(paths))
}

#[tauri::command]
pub(crate) fn clip_copy_image(data_url: String) -> Result<(), String> {
    use base64::Engine;
    let raw = data_url.split(',').nth(1).unwrap_or(&data_url);
    let png = base64::engine::general_purpose::STANDARD.decode(raw.trim()).map_err(|e| e.to_string())?;
    notch_native::clip::write(&notch_native::clip::ClipData::Image(png))
}

#[tauri::command]
pub(crate) fn clip_pin(st: S, id: u64) {
    st.clips.lock().unwrap().toggle_pin(id);
}

#[tauri::command]
pub(crate) fn clip_delete(st: S, id: u64) {
    let hash = st.clips.lock().unwrap().get(id).map(|c| c.hash);
    st.clips.lock().unwrap().delete(id);
    if let Some(h) = hash {
        st.store.lock().unwrap().tomb("clip", h.to_string());
    }
}

#[tauri::command]
pub(crate) fn clip_clear(st: S) {
    st.clips.lock().unwrap().clear();
}

#[tauri::command]
pub(crate) fn clip_image(st: S, id: u64) -> Option<String> {
    use base64::Engine;
    let b = st.clips.lock().unwrap().image_bytes(id)?;
    Some(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(b)))
}

#[tauri::command]
pub(crate) fn clip_to_note(app: AppHandle, st: S, id: u64) -> Result<store::Note, String> {
    let text = {
        let c = st.clips.lock().unwrap();
        let x = c.get(id).ok_or("Mục này không còn")?;
        if x.kind == clipboard::ClipKind::Files { x.files.join("\n") } else { x.text.clone() }
    };
    let n = st.store.lock().unwrap().upsert_note(None, text);
    kick_sync(&app);
    Ok(n)
}

#[tauri::command]
pub(crate) fn clip_to_task(app: AppHandle, st: S, id: u64, offset: Option<i32>) -> Result<Task, String> {
    let text = st.clips.lock().unwrap().get(id).map(|c| c.text.lines().next().unwrap_or("").to_string()).unwrap_or_default();
    add_task(app, st, text, offset)
}

#[tauri::command]
pub(crate) fn accessibility_status() -> bool {
    notch_native::clip::accessibility_trusted()
}

#[tauri::command]
pub(crate) fn open_privacy(pane: String) {
    #[cfg(target_os = "macos")]
    {
        let anchor = match pane.as_str() {
            "accessibility" => "Privacy_Accessibility",
            "calendar" => "Privacy_Calendars",
            "automation" => "Privacy_Automation",
            "microphone" => "Privacy_Microphone",
            "camera" => "Privacy_Camera",
            "speech" => "Privacy_SpeechRecognition",
            _ => "Privacy",
        };
        let _ = std::process::Command::new("open").arg(format!("x-apple.systempreferences:com.apple.preference.security?{anchor}")).spawn();
    }
    let _ = pane;
}

pub(crate) fn clip_loop(app: AppHandle, st: Arc<AppState>) {
    let mut last = notch_native::clip::change_count();
    loop {
        std::thread::sleep(Duration::from_millis(600));
        let cc = notch_native::clip::change_count();
        if cc == last {
            continue;
        }
        last = cc;
        let (on, max) = {
            let s = st.store.lock().unwrap();
            (s.settings.clip_enabled, s.settings.clip_max)
        };
        if !on {
            continue;
        }
        if let Some(d) = notch_native::clip::read() {
            let id = st.store.lock().unwrap().new_id();
            let mut c = st.clips.lock().unwrap();
            c.max = max.max(10);
            if c.ingest(d, id, now_ms()) {
                drop(c);
                let _ = app.emit("clip", ());
            }
        }
    }
}
