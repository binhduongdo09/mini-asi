//! Sao lưu / khôi phục / đồng bộ nhiều máy qua thư mục đám mây (Google Drive cho máy tính,
//! iCloud Drive, OneDrive, Dropbox… hoặc bất kỳ thư mục nào).
//!
//! Không cần đăng nhập trong Mini-Asi: app chỉ ghi file .zip vào thư mục Google Drive trên máy,
//! Google Drive tự tải lên và đồng bộ sang các máy khác.
//!
//!   <Drive>/Mini-Asi/devices/<máy>.zip        ← trạng thái mới nhất của từng máy (để gộp)
//!   <Drive>/Mini-Asi/backups/Mini-Asi-<máy>-<ngày>.zip  ← bản sao lưu theo ngày (giữ 14 bản)

use crate::clipboard::Clip;
use crate::journal::Entry;
use crate::store::{Note, Task};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// File dữ liệu được sao lưu (settings.json chỉ dùng khi khôi phục, không gộp giữa các máy)
pub const FILES: &[&str] = &["tasks.json", "notes.json", "journal.json", "clips.json", "tombstones.json", "settings.json"];
const KEEP_SNAPSHOTS: usize = 14;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Tomb {
    pub kind: String, // "task" | "note" | "journal" | "clip"
    pub id: String,
    pub at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub app: String,
    pub version: String,
    pub device: String,
    pub device_id: String,
    pub created: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CloudDir {
    pub label: String,
    pub path: PathBuf,
}

/// Tìm các thư mục đám mây có trên máy (Google Drive ưu tiên đầu tiên)
pub fn find_cloud_dirs(home: &Path) -> Vec<CloudDir> {
    let mut out = vec![];
    let cs = home.join("Library/CloudStorage");
    if let Ok(rd) = std::fs::read_dir(&cs) {
        let mut entries: Vec<_> = rd.filter_map(|e| e.ok()).collect();
        entries.sort_by_key(|e| e.file_name());
        for e in &entries {
            let name = e.file_name().to_string_lossy().to_string();
            if let Some(acc) = name.strip_prefix("GoogleDrive-") {
                // "My Drive" (tiếng Anh) / "Drive của tôi" (tiếng Việt) / tên khác theo ngôn ngữ máy
                let mut drive = None;
                for cand in ["My Drive", "Drive của tôi", "Mi unidad", "Mon Drive"] {
                    if e.path().join(cand).is_dir() {
                        drive = Some(e.path().join(cand));
                        break;
                    }
                }
                if drive.is_none() {
                    drive = std::fs::read_dir(e.path()).ok().and_then(|r| {
                        r.filter_map(|x| x.ok()).map(|x| x.path()).find(|p| p.is_dir() && !p.file_name().is_none_or(|n| n.to_string_lossy().starts_with('.')))
                    });
                }
                if let Some(d) = drive {
                    out.push(CloudDir { label: format!("Google Drive ({acc})"), path: d });
                }
            }
        }
        for e in &entries {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("OneDrive") {
                out.push(CloudDir { label: "OneDrive".into(), path: e.path() });
            } else if name.starts_with("Dropbox") {
                out.push(CloudDir { label: "Dropbox".into(), path: e.path() });
            }
        }
    }
    for (label, p) in [
        ("Google Drive", home.join("Google Drive/My Drive")),
        ("Google Drive", home.join("Google Drive")),
        ("Google Drive", PathBuf::from("G:\\My Drive")),
        ("iCloud Drive", home.join("Library/Mobile Documents/com~apple~CloudDocs")),
        ("Dropbox", home.join("Dropbox")),
    ] {
        if p.is_dir() && !out.iter().any(|d| d.path == p) {
            out.push(CloudDir { label: label.into(), path: p });
        }
    }
    out
}

fn safe_name(s: &str) -> String {
    let t: String = s.chars().map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' }).collect();
    t.trim_matches('-').chars().take(40).collect::<String>()
}

pub fn root(cloud: &Path) -> PathBuf {
    cloud.join("Mini-Asi")
}

/// Đóng gói dữ liệu thành .zip
pub fn make_zip(data_dir: &Path, out: &Path, manifest: &Manifest) -> Result<u64, String> {
    if let Some(p) = out.parent() {
        std::fs::create_dir_all(p).map_err(|e| format!("Không tạo được thư mục sao lưu: {e}"))?;
    }
    let tmp = out.with_extension("zip.tmp");
    {
        let f = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
        let mut z = zip::ZipWriter::new(f);
        let o = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        z.start_file("manifest.json", o).map_err(|e| e.to_string())?;
        z.write_all(serde_json::to_string_pretty(manifest).unwrap().as_bytes()).map_err(|e| e.to_string())?;
        for name in FILES {
            if let Ok(b) = std::fs::read(data_dir.join(name)) {
                z.start_file(*name, o).map_err(|e| e.to_string())?;
                z.write_all(&b).map_err(|e| e.to_string())?;
            }
        }
        // ảnh clipboard (PNG đã nén sẵn → lưu thẳng)
        let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        if let Ok(rd) = std::fs::read_dir(data_dir.join("clips")) {
            for e in rd.filter_map(|e| e.ok()) {
                if let Ok(b) = std::fs::read(e.path()) {
                    z.start_file(format!("clips/{}", e.file_name().to_string_lossy()), stored).map_err(|e| e.to_string())?;
                    z.write_all(&b).map_err(|e| e.to_string())?;
                }
            }
        }
        z.finish().map_err(|e| e.to_string())?;
    }
    std::fs::rename(&tmp, out).map_err(|e| e.to_string())?;
    Ok(std::fs::metadata(out).map(|m| m.len()).unwrap_or(0))
}

pub struct Bundle {
    pub manifest: Option<Manifest>,
    pub files: HashMap<String, Vec<u8>>,
}

impl Bundle {
    pub fn json<T: for<'de> Deserialize<'de> + Default>(&self, name: &str) -> T {
        self.files.get(name).and_then(|b| serde_json::from_slice(b).ok()).unwrap_or_default()
    }
}

pub fn read_zip(p: &Path) -> Result<Bundle, String> {
    let f = std::fs::File::open(p).map_err(|e| format!("Không mở được bản sao lưu: {e}"))?;
    let mut a = zip::ZipArchive::new(f).map_err(|_| "File sao lưu bị hỏng".to_string())?;
    let mut files = HashMap::new();
    for i in 0..a.len() {
        let mut e = a.by_index(i).map_err(|e| e.to_string())?;
        let Some(name) = e.enclosed_name().map(|n| n.to_string_lossy().replace('\\', "/")) else { continue };
        if e.is_dir() {
            continue;
        }
        let mut b = vec![];
        e.read_to_end(&mut b).map_err(|e| e.to_string())?;
        files.insert(name, b);
    }
    let manifest = files.get("manifest.json").and_then(|b| serde_json::from_slice(b).ok());
    if manifest.is_none() && !files.contains_key("tasks.json") {
        return Err("Đây không phải bản sao lưu Mini-Asi".into());
    }
    Ok(Bundle { manifest, files })
}

/// Ghi đè dữ liệu máy này bằng bản sao lưu (đã tự sao lưu bản hiện tại trước đó)
pub fn restore(b: &Bundle, data_dir: &Path, keep_settings: bool) -> Result<(), String> {
    for (name, bytes) in &b.files {
        if name == "manifest.json" || (keep_settings && name == "settings.json") {
            continue;
        }
        let ok = FILES.contains(&name.as_str()) || (name.starts_with("clips/") && !name.contains(".."));
        if !ok {
            continue;
        }
        let p = data_dir.join(name);
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        std::fs::write(&p, bytes).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ───────────────────────── Gộp dữ liệu giữa các máy ─────────────────────────

#[derive(Debug, Default, Clone, Serialize)]
pub struct MergeStats {
    pub tasks: usize,
    pub notes: usize,
    pub journal: usize,
    pub clips: usize,
    pub removed: usize,
}

fn dead(tombs: &[Tomb], kind: &str, id: &str, updated: i64) -> bool {
    tombs.iter().any(|t| t.kind == kind && t.id == id && t.at >= updated)
}

/// Gộp dữ liệu từ máy khác vào máy này: thêm mục mới, mục sửa sau thắng, áp dụng các lần xoá.
pub fn merge(
    tasks: &mut Vec<Task>,
    notes: &mut Vec<Note>,
    journal: &mut Vec<Entry>,
    clips: &mut Vec<Clip>,
    tombs: &mut Vec<Tomb>,
    remote: &Bundle,
) -> (MergeStats, Vec<String>) {
    let mut st = MergeStats::default();
    let r_tombs: Vec<Tomb> = remote.json("tombstones.json");
    for t in r_tombs {
        if !tombs.contains(&t) {
            tombs.push(t);
        }
    }
    // Việc
    for r in remote.json::<Vec<Task>>("tasks.json") {
        let ru = r.updated.max(r.created);
        if dead(tombs, "task", &r.id.to_string(), ru) {
            continue;
        }
        match tasks.iter_mut().find(|t| t.id == r.id) {
            Some(l) if ru > l.updated.max(l.created) => {
                *l = r;
                st.tasks += 1;
            }
            Some(_) => {}
            None => {
                tasks.push(r);
                st.tasks += 1;
            }
        }
    }
    let before = tasks.len();
    tasks.retain(|t| !dead(tombs, "task", &t.id.to_string(), t.updated.max(t.created)));
    st.removed += before - tasks.len();
    // Ghi chú
    for r in remote.json::<Vec<Note>>("notes.json") {
        if dead(tombs, "note", &r.id.to_string(), r.updated) {
            continue;
        }
        match notes.iter_mut().find(|n| n.id == r.id) {
            Some(l) if r.updated > l.updated => {
                *l = r;
                st.notes += 1;
            }
            Some(_) => {}
            None => {
                notes.push(r);
                st.notes += 1;
            }
        }
    }
    let before = notes.len();
    notes.retain(|n| !dead(tombs, "note", &n.id.to_string(), n.updated));
    st.removed += before - notes.len();
    // Nhật ký (theo ngày)
    for r in remote.json::<Vec<Entry>>("journal.json") {
        match journal.iter_mut().find(|e| e.date == r.date) {
            Some(l) if r.updated > l.updated => {
                *l = r;
                st.journal += 1;
            }
            Some(_) => {}
            None => {
                journal.push(r);
                st.journal += 1;
            }
        }
    }
    journal.sort_by(|a, b| b.date.cmp(&a.date));
    // Clipboard: gộp theo nội dung, mới nhất lên đầu
    let mut need_images = vec![];
    for r in remote.json::<Vec<Clip>>("clips.json") {
        if dead(tombs, "clip", &r.hash.to_string(), r.used) {
            continue;
        }
        match clips.iter_mut().find(|c| c.hash == r.hash) {
            Some(l) => {
                if r.pinned && !l.pinned && r.used > l.used {
                    l.pinned = true;
                }
                l.used = l.used.max(r.used);
            }
            None => {
                if let Some(img) = &r.image {
                    need_images.push(format!("clips/{img}"));
                }
                clips.push(r);
                st.clips += 1;
            }
        }
    }
    clips.sort_by(|a, b| b.used.cmp(&a.used));
    (st, need_images)
}

/// Giữ lại `KEEP_SNAPSHOTS` bản sao lưu gần nhất của máy này
pub fn prune_snapshots(dir: &Path, device: &str) {
    let prefix = format!("Mini-Asi-{}-", safe_name(device));
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut v: Vec<PathBuf> = rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with(&prefix))).collect();
    v.sort();
    while v.len() > KEEP_SNAPSHOTS {
        let _ = std::fs::remove_file(v.remove(0));
    }
}

pub fn snapshot_path(cloud: &Path, device: &str, date: &str) -> PathBuf {
    root(cloud).join("backups").join(format!("Mini-Asi-{}-{}.zip", safe_name(device), date))
}

pub fn device_path(cloud: &Path, device_id: &str) -> PathBuf {
    root(cloud).join("devices").join(format!("{}.zip", safe_name(device_id)))
}

#[derive(Debug, Clone, Serialize)]
pub struct BackupFile {
    pub path: PathBuf,
    pub name: String,
    pub device: String,
    pub modified: i64,
    pub size: u64,
}

fn list_zip(dir: &Path) -> Vec<BackupFile> {
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
    let mut v: Vec<BackupFile> = rd
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "zip"))
        .map(|e| {
            let m = e.metadata().ok();
            let modified = m.as_ref().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis() as i64).unwrap_or(0);
            let device = read_zip(&e.path()).ok().and_then(|b| b.manifest).map(|m| m.device).unwrap_or_default();
            BackupFile { name: e.file_name().to_string_lossy().to_string(), path: e.path(), device, modified, size: m.map(|m| m.len()).unwrap_or(0) }
        })
        .collect();
    v.sort_by(|a, b| b.modified.cmp(&a.modified));
    v
}

pub fn list_snapshots(cloud: &Path) -> Vec<BackupFile> {
    list_zip(&root(cloud).join("backups"))
}

pub fn list_devices(cloud: &Path) -> Vec<BackupFile> {
    list_zip(&root(cloud).join("devices"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(id: u64, title: &str, upd: i64) -> Task {
        Task { id, title: title.into(), due: None, created: 1, done_at: None, notified: false, updated: upd, ext_id: None }
    }

    #[test]
    fn zip_roundtrip_and_restore() {
        let a = tempfile::tempdir().unwrap();
        std::fs::write(a.path().join("tasks.json"), serde_json::to_string(&vec![task(1, "A", 5)]).unwrap()).unwrap();
        std::fs::create_dir_all(a.path().join("clips")).unwrap();
        std::fs::write(a.path().join("clips/9.png"), b"png").unwrap();
        let cloud = tempfile::tempdir().unwrap();
        let out = snapshot_path(cloud.path(), "MacBook của Bình", "2026-11-05");
        let m = Manifest { app: "Mini-Asi".into(), version: "0.4.0".into(), device: "MacBook của Bình".into(), device_id: "x".into(), created: 1 };
        assert!(make_zip(a.path(), &out, &m).unwrap() > 0);
        assert!(out.to_string_lossy().contains("Mini-Asi-MacBook-c"));
        let b = read_zip(&out).unwrap();
        assert_eq!(b.manifest.as_ref().unwrap().device, "MacBook của Bình");
        let dest = tempfile::tempdir().unwrap();
        restore(&b, dest.path(), true).unwrap();
        assert!(dest.path().join("tasks.json").exists());
        assert_eq!(std::fs::read(dest.path().join("clips/9.png")).unwrap(), b"png");
        assert_eq!(list_snapshots(cloud.path()).len(), 1);
    }

    #[test]
    fn merge_newer_wins_and_tombstones() {
        let mut files = HashMap::new();
        files.insert("tasks.json".to_string(), serde_json::to_vec(&vec![task(1, "A (máy kia sửa)", 20), task(2, "B mới", 5), task(3, "C", 1)]).unwrap());
        files.insert("tombstones.json".to_string(), serde_json::to_vec(&vec![Tomb { kind: "task".into(), id: "4".into(), at: 50 }]).unwrap());
        let remote = Bundle { manifest: None, files };
        let mut tasks = vec![task(1, "A", 10), task(3, "C (máy này sửa)", 30), task(4, "D bị xoá ở máy kia", 40)];
        let (mut n, mut j, mut c, mut tombs) = (vec![], vec![], vec![], vec![]);
        let (st, _) = merge(&mut tasks, &mut n, &mut j, &mut c, &mut tombs, &remote);
        let title = |id: u64| tasks.iter().find(|t| t.id == id).map(|t| t.title.clone());
        assert_eq!(title(1).as_deref(), Some("A (máy kia sửa)"));
        assert_eq!(title(2).as_deref(), Some("B mới"));
        assert_eq!(title(3).as_deref(), Some("C (máy này sửa)"));
        assert_eq!(title(4), None, "xoá ở máy kia → xoá ở máy này");
        assert_eq!((st.tasks, st.removed), (2, 1));
    }

    #[test]
    fn finds_google_drive_folder() {
        let h = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(h.path().join("Library/CloudStorage/GoogleDrive-binh@gmail.com/Drive của tôi")).unwrap();
        std::fs::create_dir_all(h.path().join("Library/Mobile Documents/com~apple~CloudDocs")).unwrap();
        let d = find_cloud_dirs(h.path());
        assert_eq!(d[0].label, "Google Drive (binh@gmail.com)");
        assert!(d[0].path.ends_with("Drive của tôi"));
        assert!(d.iter().any(|x| x.label == "iCloud Drive"));
    }
}
