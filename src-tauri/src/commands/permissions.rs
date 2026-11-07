//! Quyền hệ thống: trạng thái & xin quyền (hạ cửa sổ để hộp thoại macOS không bị che).

#[allow(unused_imports)]
use crate::prelude::*;

// ───────────────────────── Quyền hệ thống ─────────────────────────

#[derive(Serialize)]
pub(crate) struct PermItem {
    pub(crate) id: &'static str,
    pub(crate) name: &'static str,
    pub(crate) why: &'static str,
    pub(crate) state: &'static str, // granted | denied | not_asked | unknown | manual
    /// Bắt buộc cho tính năng chính (tuỳ chọn thì không tính vào cảnh báo thiếu quyền)
    pub(crate) required: bool,
}

pub(crate) fn state_str(s: notch_native::perm::State) -> &'static str {
    use notch_native::perm::State::*;
    match s {
        Granted => "granted",
        Denied => "denied",
        NotAsked => "not_asked",
        Unknown => "unknown",
    }
}

pub(crate) fn perm_list(app: &AppHandle) -> Vec<PermItem> {
    use tauri_plugin_notification::NotificationExt;
    use notch_native::perm;
    let notif = match app.notification().permission_state() {
        Ok(tauri::plugin::PermissionState::Denied) => "denied",
        _ => "granted",
    };
    let auto_state = if let Some(st) = app.try_state::<Arc<AppState>>() {
        let g = st.sync.lock().unwrap();
        if g.last_run > 0 && g.last_error.is_none() {
            "granted"
        } else if g.last_error.as_deref().unwrap_or("").contains("Chưa cho phép") {
            "denied"
        } else {
            "manual"
        }
    } else {
        "manual"
    };
    vec![
        PermItem { id: "notifications", name: "Thông báo", why: "Nhắc việc, nhắc nghỉ, báo giờ họp", state: notif, required: true },
        PermItem { id: "microphone", name: "Micro", why: "Ra lệnh, ghi chú & bật/tắt nhanh micro trên tai thỏ", state: state_str(perm::microphone()), required: true },
        PermItem { id: "camera", name: "Camera", why: "Gương xem nhanh & bật/tắt camera trên tai thỏ (tuỳ chọn)", state: state_str(perm::camera()), required: false },
        PermItem { id: "speech", name: "Nhận dạng giọng nói", why: "Chuyển giọng nói tiếng Việt / Anh thành chữ", state: state_str(perm::speech()), required: true },
        PermItem { id: "calendar", name: "Lịch", why: "Hiện lịch họp, báo trước giờ họp", state: state_str(perm::calendar()), required: false },
        PermItem { id: "automation", name: "Lời nhắc & Ghi chú", why: "Đồng bộ việc và ghi chú với app Apple", state: auto_state, required: false },
        PermItem { id: "accessibility", name: "Trợ năng", why: "Tự dán từ Clipboard (tuỳ chọn)", state: state_str(perm::accessibility()), required: false },
        PermItem { id: "disk", name: "Full Disk Access", why: "Quét dọn rác sâu hơn (tuỳ chọn)", state: state_str(perm::full_disk_access()), required: false },
    ]
}

#[cfg(target_os = "macos")]
fn reset_tcc_if_denied(service: &str) {
    let _ = std::process::Command::new("tccutil").args(["reset", service, "com.miniasi.app"]).status();
}

#[tauri::command]
pub(crate) fn permissions_status(app: AppHandle) -> Vec<PermItem> {
    perm_list(&app)
}

/// Xin quyền: `which` = "all" hoặc id của một quyền. Quyền nào macOS không cho hỏi trực tiếp
/// (Trợ năng, Full Disk Access) thì mở đúng trang Cài đặt hệ thống.
#[tauri::command]
pub(crate) async fn request_permissions(app: AppHandle, which: String) -> Vec<PermItem> {
    use tauri_plugin_notification::NotificationExt;
    let all = which == "all";
    window_low(&app, true);
    let a2 = app.clone();
    let which2 = which.clone();
    let _ = tauri::async_runtime::spawn_blocking(move || {
        let app = a2;
        let which = which2;
        let want = |id: &str| all || which == id;
        if want("notifications") {
            let _ = app.notification().request_permission();
        }
        if want("microphone") {
            #[cfg(target_os = "macos")]
            if notch_native::perm::microphone() == notch_native::perm::State::Denied {
                reset_tcc_if_denied("Microphone");
            }
            notch_native::perm::request_microphone();
        }
        if want("camera") {
            #[cfg(target_os = "macos")]
            if notch_native::perm::camera() == notch_native::perm::State::Denied {
                reset_tcc_if_denied("Camera");
            }
            notch_native::perm::request_camera();
        }
        if want("speech") {
            #[cfg(target_os = "macos")]
            if notch_native::perm::speech() == notch_native::perm::State::Denied {
                reset_tcc_if_denied("SpeechRecognition");
            }
            let _ = notch_native::speech::ensure_access();
        }
        if want("calendar") {
            #[cfg(target_os = "macos")]
            if notch_native::perm::calendar() == notch_native::perm::State::Denied {
                reset_tcc_if_denied("Calendar");
            }
            let _ = notch_native::calendar::ensure_access();
        }
        if want("automation") {
            #[cfg(target_os = "macos")]
            reset_tcc_if_denied("AppleEvents");
            // chạm nhẹ vào Lời nhắc & Ghi chú → macOS hỏi quyền Tự động hoá và lưu ngay kết quả
            let r1 = apple::reminders_pull("Mini-Asi");
            let r2 = apple::notes_pull("Mini-Asi");
            if let Some(st) = app.try_state::<Arc<AppState>>() {
                let mut g = st.sync.lock().unwrap();
                g.last_run = now_ms();
                g.last_error = match (r1, r2) {
                    (Err(e), _) | (_, Err(e)) => Some(e),
                    _ => None,
                };
                let _ = g.save();
            }
        }
        if want("accessibility") && notch_native::perm::accessibility() != notch_native::perm::State::Granted {
            #[cfg(target_os = "macos")]
            if !all {
                reset_tcc_if_denied("Accessibility");
            }
            let _ = on_main(&app, notch_native::perm::prompt_accessibility);
        }
        if (all || which == "disk") && notch_native::perm::full_disk_access() != notch_native::perm::State::Granted {
            #[cfg(target_os = "macos")]
            let _ = std::process::Command::new("open").arg("x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles").spawn();
        }
    })
    .await;
    window_low(&app, false);
    perm_list(&app)
}
