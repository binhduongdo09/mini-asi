//! Cài đặt & bản quyền Pro.

#[allow(unused_imports)]
use crate::prelude::*;

// ───────────────────────── Cài đặt & bản quyền ─────────────────────────

#[tauri::command]
pub(crate) fn get_settings(st: S) -> store::Settings {
    st.store.lock().unwrap().settings.clone()
}

#[tauri::command]
pub(crate) fn save_settings(app: AppHandle, st: S, settings: store::Settings) -> Result<(), String> {
    apply_settings(&app, &st, settings)
}

/// Chỉ đổi vài khoá (vd. `{"modules": [...]}`) — hai cửa sổ (tai thỏ & Cài đặt) không ghi đè nhau bằng bản cũ.
#[tauri::command]
pub(crate) fn patch_settings(app: AppHandle, st: S, patch: serde_json::Value) -> Result<store::Settings, String> {
    let merged = {
        let s = st.store.lock().unwrap();
        merge_settings(&s.settings, &patch)?
    };
    apply_settings(&app, &st, merged)?;
    Ok(st.store.lock().unwrap().settings.clone())
}

/// Gộp các khoá trong `patch` vào bản cài đặt hiện tại (khoá lạ bị bỏ qua, kiểu sai → báo lỗi).
pub(crate) fn merge_settings(cur: &store::Settings, patch: &serde_json::Value) -> Result<store::Settings, String> {
    let mut v = serde_json::to_value(cur).map_err(|e| e.to_string())?;
    if let (Some(dst), Some(src)) = (v.as_object_mut(), patch.as_object()) {
        for (k, val) in src {
            if k != "license_key" && dst.contains_key(k) {
                dst.insert(k.clone(), val.clone());
            }
        }
    }
    serde_json::from_value(v).map_err(|e| format!("Cài đặt không hợp lệ: {e}"))
}

fn apply_settings(app: &AppHandle, st: &S, settings: store::Settings) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let mut s = st.store.lock().unwrap();
    let key = s.settings.license_key.clone(); // bản quyền chỉ đổi qua activate_license
    let autostart_changed = s.settings.launch_at_login != settings.launch_at_login;
    s.settings = settings;
    s.settings.license_key = key;
    s.settings.break_minutes = s.settings.break_minutes.clamp(10, 180);
    s.settings.water_minutes = s.settings.water_minutes.clamp(15, 240);
    s.save_settings()?;
    if autostart_changed {
        let al = app.autolaunch();
        let _ = if s.settings.launch_at_login { al.enable() } else { al.disable() };
    }
    let snap = s.settings.clone();
    drop(s);
    let _ = app.emit("settings-changed", &snap);
    push_status(app);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_only_known_keys_and_keeps_license() {
        let mut cur = store::Settings::default();
        cur.license_key = Some("MASI1.secret".into());
        let patch = serde_json::json!({ "modules": ["journal"], "break_minutes": 30, "khong_ton_tai": 1, "license_key": "hack" });
        let m = merge_settings(&cur, &patch).unwrap();
        assert_eq!(m.modules, vec!["journal".to_string()]);
        assert_eq!(m.break_minutes, 30);
        assert_eq!(m.license_key.as_deref(), Some("MASI1.secret"));
        assert_eq!(m.clip_max, cur.clip_max);
    }

    #[test]
    fn merge_rejects_wrong_type() {
        let cur = store::Settings::default();
        assert!(merge_settings(&cur, &serde_json::json!({ "break_minutes": "nhiều" })).is_err());
    }
}

#[derive(Serialize)]
pub(crate) struct LicenseInfo {
    pub(crate) pro: bool,
    pub(crate) email: Option<String>,
    pub(crate) expires: Option<u64>,
    pub(crate) error: Option<String>,
}

pub(crate) fn license_info_of(key: Option<&str>) -> LicenseInfo {
    match key.map(license::check) {
        Some(Ok(l)) => LicenseInfo { pro: true, email: Some(l.e), expires: l.x, error: None },
        Some(Err(e)) => LicenseInfo { pro: false, email: None, expires: None, error: Some(e.to_string()) },
        None => LicenseInfo { pro: false, email: None, expires: None, error: None },
    }
}

#[tauri::command]
pub(crate) fn license_info(st: S) -> LicenseInfo {
    let k = st.store.lock().unwrap().settings.license_key.clone();
    license_info_of(k.as_deref())
}

#[tauri::command]
pub(crate) fn activate_license(app: AppHandle, st: S, key: String) -> LicenseInfo {
    let info = license_info_of(Some(key.trim()));
    if info.pro {
        let mut s = st.store.lock().unwrap();
        s.settings.license_key = Some(key.trim().to_string());
        let _ = s.save_settings();
        let snap = s.settings.clone();
        drop(s);
        let _ = app.emit("settings-changed", &snap);
        push_status(&app);
    }
    info
}

#[tauri::command]
pub(crate) fn deactivate_license(app: AppHandle, st: S) {
    let mut s = st.store.lock().unwrap();
    s.settings.license_key = None;
    let _ = s.save_settings();
    let snap = s.settings.clone();
    drop(s);
    let _ = app.emit("settings-changed", &snap);
    push_status(&app);
}

#[tauri::command]
pub(crate) fn quit(app: AppHandle) {
    app.exit(0);
}
