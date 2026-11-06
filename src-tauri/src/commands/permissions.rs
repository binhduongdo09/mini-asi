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
        Ok(tauri::plugin::PermissionState::Granted) => "granted",
        Ok(tauri::plugin::PermissionState::Denied) => "denied",
        Ok(_) => "not_asked",
        Err(_) => "unknown",
    };
    vec![
        PermItem { id: "notifications", name: "Thông báo", why: "Nhắc việc, nhắc nghỉ, báo giờ họp", state: notif, required: true },
        PermItem { id: "microphone", name: "Micro", why: "Ra lệnh & ghi chú bằng giọng nói", state: state_str(perm::microphone()), required: true },
        PermItem { id: "speech", name: "Nhận dạng giọng nói", why: "Chuyển giọng nói tiếng Việt / Anh thành chữ", state: state_str(perm::speech()), required: true },
        PermItem { id: "calendar", name: "Lịch", why: "Hiện lịch họp, báo trước giờ họp", state: state_str(perm::calendar()), required: false },
        PermItem { id: "automation", name: "Lời nhắc & Ghi chú", why: "Đồng bộ việc và ghi chú với app Apple", state: "manual", required: false },
        PermItem { id: "accessibility", name: "Trợ năng", why: "Tự dán từ Clipboard (tuỳ chọn)", state: state_str(perm::accessibility()), required: false },
        PermItem { id: "disk", name: "Full Disk Access", why: "Quét dọn rác sâu hơn (tuỳ chọn)", state: state_str(perm::full_disk_access()), required: false },
    ]
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
            notch_native::perm::request_microphone();
        }
        if want("speech") {
            let _ = notch_native::speech::ensure_access();
        }
        if want("calendar") {
            let _ = notch_native::calendar::ensure_access();
        }
        if want("automation") {
            // chạm nhẹ vào Lời nhắc & Ghi chú → macOS hỏi quyền Tự động hoá
            let _ = apple::reminders_pull("Mini-Asi");
            let _ = apple::notes_pull("Mini-Asi");
        }
        if want("accessibility") && notch_native::perm::accessibility() != notch_native::perm::State::Granted {
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
