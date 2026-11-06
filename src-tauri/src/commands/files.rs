//! Kéo thả file: nén/giải nén ZIP, nén & gộp PDF.

#[allow(unused_imports)]
use crate::prelude::*;

// ───────────────────────── File ─────────────────────────

#[derive(Serialize)]
pub(crate) struct FileInfo {
    pub(crate) path: String,
    pub(crate) name: String,
    pub(crate) size: u64,
    pub(crate) is_dir: bool,
    pub(crate) is_pdf: bool,
    pub(crate) is_archive: bool,
}

#[tauri::command]
pub(crate) fn file_info(paths: Vec<String>) -> Vec<FileInfo> {
    paths
        .into_iter()
        .map(|p| {
            let pb = PathBuf::from(&p);
            let m = std::fs::metadata(&pb).ok();
            FileInfo {
                name: pb.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
                size: m.as_ref().map(|m| m.len()).unwrap_or(0),
                is_dir: m.as_ref().map(|m| m.is_dir()).unwrap_or(false),
                is_pdf: files::is_pdf(&pb),
                is_archive: files::is_archive(&pb),
                path: p,
            }
        })
        .collect()
}

#[tauri::command]
pub(crate) async fn pick_files(app: AppHandle) -> Vec<String> {
    use tauri_plugin_dialog::DialogExt;
    tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .blocking_pick_files()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|f| f.into_path().ok())
            .map(|p| p.to_string_lossy().to_string())
            .collect()
    })
    .await
    .unwrap_or_default()
}

pub(crate) async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub(crate) async fn file_zip(paths: Vec<String>) -> Result<files::FileResult, String> {
    blocking(move || files::zip_paths(&paths.iter().map(PathBuf::from).collect::<Vec<_>>())).await
}

#[tauri::command]
pub(crate) async fn file_extract(path: String) -> Result<files::FileResult, String> {
    blocking(move || files::extract(&PathBuf::from(path))).await
}

#[tauri::command]
pub(crate) async fn file_compress_pdf(path: String, quality: String) -> Result<files::FileResult, String> {
    blocking(move || files::compress_pdf(&PathBuf::from(path), &quality)).await
}

#[tauri::command]
pub(crate) async fn file_merge_pdf(paths: Vec<String>) -> Result<files::FileResult, String> {
    blocking(move || files::merge_pdfs(&paths.iter().map(PathBuf::from).collect::<Vec<_>>())).await
}

#[tauri::command]
pub(crate) fn reveal(path: String) {
    files::reveal(&PathBuf::from(path));
}
