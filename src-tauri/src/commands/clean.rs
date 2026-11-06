//! Dọn máy (quét & dọn theo luật của crate asi-clean).

#[allow(unused_imports)]
use crate::prelude::*;

// ───────────────────────── Dọn máy ─────────────────────────

pub(crate) fn tier(st: &AppState) -> asi_clean::rules::Tier {
    let key = st.store.lock().unwrap().settings.license_key.clone();
    match key.as_deref().map(license::check) {
        Some(Ok(_)) => asi_clean::rules::Tier::Pro,
        _ => asi_clean::rules::Tier::Free,
    }
}

pub(crate) fn engine(st: &AppState) -> Result<asi_clean::Engine, String> {
    asi_clean::Engine::new(asi_clean::config::Paths::detect(), tier(st))
}

#[derive(Serialize)]
pub(crate) struct ScanOut {
    pub(crate) report: asi_clean::scan::ScanReport,
    pub(crate) disk: Option<asi_clean::disk::DiskStatus>,
    pub(crate) pro: bool,
}

#[tauri::command]
pub(crate) async fn clean_scan(st: S<'_>) -> Result<ScanOut, String> {
    let st = st.inner().clone();
    blocking(move || {
        let e = engine(&st)?;
        let report = e.scan(vec![]);
        *st.last_scan.lock().unwrap() = Some(report.clone());
        Ok(ScanOut { disk: e.disk(), pro: e.tier == asi_clean::rules::Tier::Pro, report })
    })
    .await
}

#[tauri::command]
pub(crate) async fn clean_run(st: S<'_>, rules: Vec<String>, paths: Vec<String>) -> Result<asi_clean::clean::CleanReport, String> {
    let st = st.inner().clone();
    blocking(move || {
        let e = engine(&st)?;
        // Quét lại ngay trước khi xoá để dữ liệu không bị cũ
        let report = e.scan(vec![]);
        let sel = asi_clean::clean::Selection { rules, paths: paths.into_iter().map(PathBuf::from).collect() };
        let res = e.clean(&report, &sel, false);
        *st.last_scan.lock().unwrap() = None;
        Ok(res)
    })
    .await
}

#[tauri::command]
pub(crate) fn clean_history(st: S) -> Result<Vec<asi_clean::oplog::OpRecord>, String> {
    Ok(engine(&st)?.history(50))
}

#[tauri::command]
pub(crate) fn open_disk_settings() {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg("x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles").spawn();
}
