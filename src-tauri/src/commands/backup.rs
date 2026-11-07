//! Sao lưu / khôi phục / đồng bộ nhiều máy qua thư mục đám mây (Google Drive, iCloud…).

#[allow(unused_imports)]
use crate::prelude::*;

// ───────────────────────── Sao lưu & đồng bộ nhiều máy ─────────────────────────

pub(crate) fn device_name() -> String {
    #[cfg(target_os = "macos")]
    if let Ok(o) = std::process::Command::new("scutil").args(["--get", "ComputerName"]).output() {
        let n = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if !n.is_empty() {
            return n;
        }
    }
    std::env::var("HOSTNAME").or_else(|_| std::env::var("COMPUTERNAME")).unwrap_or_else(|_| "May-tinh".into())
}

pub(crate) fn ensure_device(st: &AppState) -> (String, String) {
    let mut s = st.store.lock().unwrap();
    if s.settings.device_id.is_empty() {
        s.settings.device_id = format!("{:x}", now_ms() ^ (std::process::id() as i64) << 20);
    }
    if s.settings.device_name.is_empty() {
        s.settings.device_name = device_name();
    }
    let _ = s.save_settings();
    (s.settings.device_id.clone(), s.settings.device_name.clone())
}

pub(crate) fn home_dir() -> PathBuf {
    asi_clean::config::Paths::detect().home
}

/// Thư mục đám mây đang dùng: do người dùng chọn, hoặc Google Drive tự tìm thấy
pub(crate) fn cloud_dir(st: &AppState) -> Option<PathBuf> {
    let chosen = st.store.lock().unwrap().settings.backup_dir.clone();
    if !chosen.is_empty() {
        let p = PathBuf::from(chosen);
        return p.is_dir().then_some(p);
    }
    backup::find_cloud_dirs(&home_dir()).into_iter().next().map(|d| d.path)
}

pub(crate) fn manifest(st: &AppState) -> backup::Manifest {
    let (id, name) = ensure_device(st);
    backup::Manifest { app: "Mini-Asi".into(), version: env!("CARGO_PKG_VERSION").into(), device: name, device_id: id, created: now_ms() }
}

/// Ghi bộ nhớ hiện tại ra đĩa rồi đóng gói
pub(crate) fn flush(st: &AppState) {
    let s = st.store.lock().unwrap();
    let _ = s.save_tasks();
    let _ = s.save_notes();
    let _ = s.save_tombstones();
    drop(s);
    st.clips.lock().unwrap().save();
}

pub(crate) fn backup_now_inner(st: &AppState) -> Result<(PathBuf, u64), String> {
    let cloud = cloud_dir(st).ok_or("Chưa có thư mục Google Drive — cài Google Drive cho máy tính hoặc chọn thư mục sao lưu")?;
    flush(st);
    let m = manifest(st);
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    let snap = backup::snapshot_path(&cloud, &m.device, &date);
    let size = backup::make_zip(&st.data_dir, &snap, &m)?;
    backup::make_zip(&st.data_dir, &backup::device_path(&cloud, &m.device_id), &m)?;
    backup::prune_snapshots(snap.parent().unwrap(), &m.device);
    let mut s = st.store.lock().unwrap();
    s.settings.last_backup = now_ms();
    let _ = s.save_settings();
    Ok((snap, size))
}

/// Nạp lại toàn bộ dữ liệu từ đĩa (sau khi khôi phục / gộp)
pub(crate) fn reload(st: &AppState) {
    let fresh = Store::open(st.data_dir.clone());
    let max = fresh.settings.clip_max;
    *st.store.lock().unwrap() = fresh;
    *st.clips.lock().unwrap() = clipboard::ClipStore::open(&st.data_dir, max);
    *st.journal.lock().unwrap() = journal::Journal::open(&st.data_dir);
}

#[derive(Serialize, Default)]
pub(crate) struct DeviceSyncReport {
    pub(crate) devices: Vec<String>,
    pub(crate) stats: backup::MergeStats,
}

/// Gộp dữ liệu từ các máy khác (trong <Drive>/Mini-Asi/devices) rồi ghi lại trạng thái máy này
pub(crate) fn device_sync_inner(st: &AppState) -> Result<DeviceSyncReport, String> {
    let cloud = cloud_dir(st).ok_or("Chưa có thư mục Google Drive")?;
    let (my_id, _) = ensure_device(st);
    flush(st);
    let mut rep = DeviceSyncReport::default();
    for f in backup::list_devices(&cloud) {
        if f.path.file_stem().is_some_and(|n| n.to_string_lossy() == my_id) {
            continue;
        }
        let Ok(b) = backup::read_zip(&f.path) else { continue };
        let mut s = st.store.lock().unwrap();
        let mut c = st.clips.lock().unwrap();
        let mut j = st.journal.lock().unwrap();
        let (mut tasks, mut notes, mut clips, mut tombs) = (s.tasks.clone(), s.notes.clone(), c.items.clone(), s.tombstones.clone());
        let mut entries = j.entries.clone();
        let (stats, images) = backup::merge(&mut tasks, &mut notes, &mut entries, &mut clips, &mut tombs, &b);
        for img in images {
            if let Some(bytes) = b.files.get(&img) {
                let _ = std::fs::write(st.data_dir.join(&img), bytes);
            }
        }
        s.tasks = tasks;
        s.notes = notes;
        s.tombstones = tombs;
        let _ = s.save_tasks();
        let _ = s.save_notes();
        let _ = s.save_tombstones();
        c.items = clips;
        c.save();
        j.entries = entries;
        let _ = j.save();
        rep.devices.push(b.manifest.map(|m| m.device).unwrap_or(f.name));
        rep.stats.tasks += stats.tasks;
        rep.stats.notes += stats.notes;
        rep.stats.journal += stats.journal;
        rep.stats.clips += stats.clips;
        rep.stats.removed += stats.removed;
    }
    let m = manifest(st);
    backup::make_zip(&st.data_dir, &backup::device_path(&cloud, &m.device_id), &m)?;
    let mut s = st.store.lock().unwrap();
    s.settings.last_device_sync = now_ms();
    let _ = s.save_settings();
    Ok(rep)
}

#[derive(Serialize)]
pub(crate) struct BackupInfo {
    pub(crate) dirs: Vec<backup::CloudDir>,
    pub(crate) current: Option<PathBuf>,
    pub(crate) chosen: String,
    pub(crate) auto_backup: bool,
    pub(crate) device_sync: bool,
    pub(crate) device_name: String,
    pub(crate) last_backup: i64,
    pub(crate) last_device_sync: i64,
    pub(crate) snapshots: Vec<backup::BackupFile>,
    pub(crate) devices: Vec<backup::BackupFile>,
}

#[tauri::command]
pub(crate) async fn backup_info(st: S<'_>) -> Result<BackupInfo, String> {
    let st = st.inner().clone();
    blocking(move || {
        let (_, name) = ensure_device(&st);
        let cur = cloud_dir(&st);
        let s = st.store.lock().unwrap().settings.clone();
        Ok(BackupInfo {
            dirs: backup::find_cloud_dirs(&home_dir()),
            snapshots: cur.as_ref().map(|c| backup::list_snapshots(c)).unwrap_or_default(),
            devices: cur.as_ref().map(|c| backup::list_devices(c)).unwrap_or_default(),
            current: cur,
            chosen: s.backup_dir,
            auto_backup: s.auto_backup,
            device_sync: s.device_sync,
            device_name: name,
            last_backup: s.last_backup,
            last_device_sync: s.last_device_sync,
        })
    })
    .await
}

#[tauri::command]
pub(crate) async fn backup_pick_dir(app: AppHandle, st: S<'_>) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    apply_level(&app, 0);
    let a2 = app.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || a2.dialog().file().blocking_pick_folder()).await.map_err(|e| e.to_string());
    apply_level(&app, normal_level(&app));
    let Some(p) = picked?.and_then(|f| f.into_path().ok()) else { return Ok(None) };
    let mut s = st.store.lock().unwrap();
    s.settings.backup_dir = p.to_string_lossy().to_string();
    let _ = s.save_settings();
    Ok(Some(s.settings.backup_dir.clone()))
}

#[tauri::command]
pub(crate) fn backup_set(st: S, dir: Option<String>, auto_backup: Option<bool>, device_sync: Option<bool>) -> Result<(), String> {
    let mut s = st.store.lock().unwrap();
    if let Some(d) = dir {
        s.settings.backup_dir = d;
    }
    if let Some(a) = auto_backup {
        s.settings.auto_backup = a;
    }
    if let Some(d) = device_sync {
        s.settings.device_sync = d;
    }
    s.save_settings()
}

#[tauri::command]
pub(crate) async fn backup_now(st: S<'_>) -> Result<(String, u64), String> {
    let st = st.inner().clone();
    blocking(move || backup_now_inner(&st).map(|(p, n)| (p.to_string_lossy().to_string(), n))).await
}

#[tauri::command]
pub(crate) async fn device_sync_now(app: AppHandle, st: S<'_>) -> Result<DeviceSyncReport, String> {
    let st2 = st.inner().clone();
    let r = blocking(move || device_sync_inner(&st2)).await?;
    push_status(&app);
    let _ = app.emit("synced", ());
    Ok(r)
}

/// Khôi phục từ một bản sao lưu. Bản hiện tại được cất vào <dữ liệu app>/before-restore-*.zip trước.
#[tauri::command]
pub(crate) async fn backup_restore(app: AppHandle, st: S<'_>, path: String) -> Result<(), String> {
    let st2 = st.inner().clone();
    blocking(move || {
        let b = backup::read_zip(&PathBuf::from(&path))?;
        flush(&st2);
        let m = manifest(&st2);
        let safety = st2.data_dir.join(format!("before-restore-{}.zip", chrono::Local::now().format("%Y%m%d-%H%M%S")));
        backup::make_zip(&st2.data_dir, &safety, &m)?;
        backup::restore(&b, &st2.data_dir, true)?;
        reload(&st2);
        Ok(())
    })
    .await?;
    push_status(&app);
    let _ = app.emit("synced", ());
    Ok(())
}

/// Tự sao lưu mỗi ngày và (nếu bật) gộp dữ liệu giữa các máy mỗi 15 phút
pub(crate) fn backup_loop(app: AppHandle, st: Arc<AppState>) {
    std::thread::sleep(Duration::from_secs(40));
    loop {
        let (auto, dev, last) = {
            let s = st.store.lock().unwrap();
            (s.settings.auto_backup, s.settings.device_sync, s.settings.last_backup)
        };
        if dev && cloud_dir(&st).is_some() {
            if let Ok(r) = device_sync_inner(&st) {
                if r.stats.tasks + r.stats.notes + r.stats.journal + r.stats.clips + r.stats.removed > 0 {
                    let _ = app.emit("synced", ());
                    push_status(&app);
                }
            }
        }
        if auto && now_ms() - last > 20 * 3_600_000 && cloud_dir(&st).is_some() {
            let _ = backup_now_inner(&st);
        }
        std::thread::sleep(Duration::from_secs(900));
    }
}
