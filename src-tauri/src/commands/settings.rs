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
    use tauri_plugin_autostart::ManagerExt;
    let mut s = st.store.lock().unwrap();
    let key = s.settings.license_key.clone(); // bản quyền chỉ đổi qua activate_license
    s.settings = settings;
    s.settings.license_key = key;
    s.settings.break_minutes = s.settings.break_minutes.clamp(10, 180);
    s.settings.water_minutes = s.settings.water_minutes.clamp(15, 240);
    s.save_settings()?;
    let al = app.autolaunch();
    let _ = if s.settings.launch_at_login { al.enable() } else { al.disable() };
    let snap = s.settings.clone();
    drop(s);
    let _ = app.emit("settings-changed", &snap);
    push_status(&app);
    Ok(())
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
