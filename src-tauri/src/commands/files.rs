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
    window_low(&app, true);
    let a2 = app.clone();
    let res = tauri::async_runtime::spawn_blocking(move || {
        a2.dialog()
            .file()
            .blocking_pick_files()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|f| f.into_path().ok())
            .map(|p| p.to_string_lossy().to_string())
            .collect()
    })
    .await
    .unwrap_or_default();
    window_low(&app, false);
    res
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

/// Mở bảng chia sẻ AirDrop gốc của macOS cho danh sách file (kéo thả vào tai thỏ hoặc chọn từ khay File).
#[tauri::command]
pub(crate) fn file_airdrop(app: AppHandle, paths: Vec<String>) -> Result<(), String> {
    if paths.is_empty() {
        return Err("Chưa có file nào để gửi AirDrop".into());
    }
    // Hạ tạm tầng cửa sổ tai thỏ xuống mức nổi thường để hộp thoại AirDrop của macOS hiện lên trên cùng
    window_low_for(&app, 45); // bấm lại vào tai thỏ thì nâng lên ngay (popup_done)
    on_main(&app, move || notch_native::airdrop_files(&paths)).unwrap_or_else(|| Err("Không mở được AirDrop".into()))
}

/// Kiểm tra con trỏ chuột đang ở nửa trái ("left" = AirDrop) hay nửa phải ("right" = Khay File) của tai thỏ khi kéo thả.
#[tauri::command]
pub(crate) fn cursor_side(app: AppHandle) -> String {
    if let Some(win) = app.get_webview_window("notch") {
        if let (Ok(c), Ok(p), Ok(sz)) = (app.cursor_position(), win.outer_position(), win.outer_size()) {
            let mid = p.x as f64 + (sz.width as f64) / 2.0;
            return if c.x < mid { "left".into() } else { "right".into() };
        }
    }
    "left".into()
}

// ───────────────────────── Điều khiển nhanh trên tai thỏ (Media · Mic · Camera) ─────────────────────────

use std::sync::atomic::{AtomicU8, Ordering};

static LAST_MIC_VOL: AtomicU8 = AtomicU8::new(80);
static MEDIA_CACHE: OnceLock<Mutex<(i64, MediaQuickStatus)>> = OnceLock::new();

fn media_cache() -> &'static Mutex<(i64, MediaQuickStatus)> {
    MEDIA_CACHE.get_or_init(|| Mutex::new((0, MediaQuickStatus::default())))
}

#[derive(Debug, Clone, Serialize, serde::Deserialize, Default)]
#[serde(default)]
pub(crate) struct MediaQuickStatus {
    pub(crate) title: String,
    pub(crate) artist: String,
    pub(crate) app: String,
    pub(crate) playing: bool,
    pub(crate) output_volume: u8,
    pub(crate) output_muted: bool,
    pub(crate) mic_volume: u8,
    pub(crate) mic_muted: bool,
    pub(crate) cam_granted: bool,
}

#[cfg(target_os = "macos")]
const MEDIA_STATUS_JXA: &str = r#"
const cur = Application.currentApplication();
cur.includeStandardAdditions = true;
let vol = { outputVolume: 50, inputVolume: 80, outputMuted: false };
try { vol = cur.getVolumeSettings(); } catch (e) {}
let title = '', artist = '', appName = '', playing = false;
for (const name of ['Spotify', 'Music']) {
  try {
    const a = Application(name);
    if (a.running()) {
      const st = String(a.playerState());
      if (st === 'playing' || (!title && st === 'paused')) {
        const tr = a.currentTrack;
        title = String(tr.name() || '');
        artist = String(tr.artist() || '');
        appName = name === 'Music' ? 'Apple Music' : 'Spotify';
        playing = (st === 'playing');
        if (playing) break;
      }
    }
  } catch (e) {}
}
JSON.stringify({
  title: title,
  artist: artist,
  app: appName,
  playing: playing,
  output_volume: Number(vol.outputVolume ?? 50),
  output_muted: !!vol.outputMuted,
  mic_volume: Number(vol.inputVolume ?? 80),
  mic_muted: Number(vol.inputVolume ?? 80) === 0
});
"#;

pub(crate) fn query_media_status_sync(force: bool) -> MediaQuickStatus {
    let now = now_ms();
    if !force {
        if let Ok(g) = media_cache().lock() {
            if g.0 > 0 && now - g.0 < 2500 {
                return g.1.clone();
            }
        }
    }
    let cam_granted = notch_native::perm::camera() == notch_native::perm::State::Granted;
    #[cfg(target_os = "macos")]
    {
        if let Ok(out) = std::process::Command::new("osascript").args(["-l", "JavaScript", "-e", MEDIA_STATUS_JXA]).output() {
            if out.status.success() {
                if let Ok(mut s) = serde_json::from_slice::<MediaQuickStatus>(&out.stdout) {
                    if s.mic_volume > 0 {
                        LAST_MIC_VOL.store(s.mic_volume, Ordering::Relaxed);
                    }
                    s.cam_granted = cam_granted;
                    if let Ok(mut g) = media_cache().lock() {
                        *g = (now_ms(), s.clone());
                    }
                    return s;
                }
            }
        }
    }
    let fallback = MediaQuickStatus {
        output_volume: 60,
        mic_volume: LAST_MIC_VOL.load(Ordering::Relaxed),
        cam_granted,
        ..Default::default()
    };
    if let Ok(mut g) = media_cache().lock() {
        *g = (now_ms(), fallback.clone());
    }
    fallback
}

#[tauri::command]
pub(crate) async fn media_status() -> MediaQuickStatus {
    tauri::async_runtime::spawn_blocking(|| query_media_status_sync(false)).await.unwrap_or_default()
}

/// Điều khiển nhạc/media: "play" | "next" | "prev" | "mute_out" | "vol_up" | "vol_down"
#[tauri::command]
pub(crate) async fn media_control(app: AppHandle, action: String) -> MediaQuickStatus {
    match action.as_str() {
        "play" | "play_pause" => {
            let _ = on_main(&app, || notch_native::send_media_key(notch_native::MEDIA_KEY_PLAY));
        }
        "next" => {
            let _ = on_main(&app, || notch_native::send_media_key(notch_native::MEDIA_KEY_NEXT));
        }
        "prev" => {
            let _ = on_main(&app, || notch_native::send_media_key(notch_native::MEDIA_KEY_PREV));
        }
        "vol_up" => {
            let _ = on_main(&app, || notch_native::send_media_key(notch_native::MEDIA_KEY_VOL_UP));
        }
        "vol_down" => {
            let _ = on_main(&app, || notch_native::send_media_key(notch_native::MEDIA_KEY_VOL_DOWN));
        }
        "mute_out" => {
            #[cfg(target_os = "macos")]
            {
                let _ = tauri::async_runtime::spawn_blocking(|| {
                    let _ = std::process::Command::new("osascript")
                        .args(["-e", "set volume output muted not (output muted of (get volume settings))"])
                        .status();
                })
                .await;
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = on_main(&app, || notch_native::send_media_key(notch_native::MEDIA_KEY_MUTE));
            }
        }
        _ => {}
    }
    tauri::async_runtime::spawn_blocking(|| {
        std::thread::sleep(Duration::from_millis(90));
        query_media_status_sync(true)
    })
    .await
    .unwrap_or_default()
}

/// Bật / tắt nhanh tiếng Micro đầu vào của hệ thống ngay từ tai thỏ.
#[tauri::command]
pub(crate) async fn toggle_mic_mute() -> MediaQuickStatus {
    tauri::async_runtime::spawn_blocking(|| {
        let mut cur = query_media_status_sync(false);
        #[cfg(target_os = "macos")]
        {
            if cur.mic_muted || cur.mic_volume == 0 {
                let restore = LAST_MIC_VOL.load(Ordering::Relaxed).clamp(40, 100);
                let _ = std::process::Command::new("osascript")
                    .args(["-e", &format!("set volume input volume {restore}")])
                    .status();
                cur.mic_volume = restore;
                cur.mic_muted = false;
            } else {
                LAST_MIC_VOL.store(cur.mic_volume, Ordering::Relaxed);
                let _ = std::process::Command::new("osascript")
                    .args(["-e", "set volume input volume 0"])
                    .status();
                cur.mic_volume = 0;
                cur.mic_muted = true;
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = std::process::Command::new("pactl").args(["set-source-mute", "@DEFAULT_SOURCE@", "toggle"]).status();
            cur.mic_muted = !cur.mic_muted;
            cur.mic_volume = if cur.mic_muted { 0 } else { 80 };
        }
        if let Ok(mut g) = media_cache().lock() {
            *g = (now_ms(), cur.clone());
        }
        cur
    })
    .await
    .unwrap_or_default()
}

/// Kiểm tra và xin quyền Camera trước khi mở gương Camera trên tai thỏ.
#[tauri::command]
pub(crate) async fn ensure_camera_perm(app: AppHandle) -> bool {
    let st = notch_native::perm::camera();
    if st == notch_native::perm::State::Granted {
        return true;
    }
    if st == notch_native::perm::State::Denied {
        #[cfg(target_os = "macos")]
        let _ = std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Camera")
            .spawn();
        return false;
    }
    window_low(&app, true);
    let ok = tauri::async_runtime::spawn_blocking(notch_native::perm::request_camera).await.unwrap_or(false);
    window_low(&app, false);
    ok
}
