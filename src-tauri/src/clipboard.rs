//! Lịch sử clipboard: chữ, ảnh, file. Ghim để giữ mãi, chọn lại để dán ra nhiều nơi.
//! Dữ liệu lưu trên máy: clips.json + thư mục clips/ chứa ảnh PNG.

use notch_native::clip::ClipData;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

const MAX_TEXT: usize = 200_000;
const MAX_IMAGE: usize = 25 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClipKind {
    Text,
    Image,
    Files,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Clip {
    pub id: u64,
    pub kind: ClipKind,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub files: Vec<String>,
    /// Tên file ảnh trong thư mục clips/
    #[serde(default)]
    pub image: Option<String>,
    pub bytes: u64,
    pub hash: u64,
    pub created: i64,
    /// Lần chép / dùng gần nhất
    pub used: i64,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub uses: u32,
    /// Tên gợi nhớ cho mục ghim (vd. "STK công ty")
    #[serde(default)]
    pub label: Option<String>,
    /// Lúc ghim — mục ghim xếp theo thứ tự ghim (cũ trước), không nhảy lên đầu khi chép lại
    #[serde(default)]
    pub pinned_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileRef {
    pub name: String,
    pub path: String,
    pub exists: bool,
}

/// Bản rút gọn gửi lên giao diện
#[derive(Debug, Clone, Serialize)]
pub struct ClipView {
    pub id: u64,
    pub kind: ClipKind,
    pub preview: String,
    pub chars: usize,
    pub files: Vec<FileRef>,
    pub bytes: u64,
    pub created: i64,
    pub used: i64,
    pub pinned: bool,
    pub uses: u32,
    pub label: Option<String>,
}

pub struct ClipStore {
    dir: PathBuf,
    pub items: Vec<Clip>,
    pub max: usize,
}

fn hash_of(d: &ClipData) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    match d {
        ClipData::Text(t) => (0u8, t).hash(&mut h),
        ClipData::Image(b) => (1u8, b).hash(&mut h),
        ClipData::Files(f) => (2u8, f).hash(&mut h),
    }
    h.finish()
}

impl ClipStore {
    pub fn open(data_dir: &Path, max: usize) -> Self {
        let dir = data_dir.join("clips");
        let _ = std::fs::create_dir_all(&dir);
        let items: Vec<Clip> = std::fs::read_to_string(data_dir.join("clips.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self { dir, items, max: max.max(10) }
    }

    fn index_file(&self) -> PathBuf {
        self.dir.parent().unwrap_or(&self.dir).join("clips.json")
    }

    pub fn save(&self) {
        let p = self.index_file();
        let tmp = p.with_extension("json.tmp");
        if let Ok(s) = serde_json::to_string(&self.items) {
            if std::fs::write(&tmp, s).is_ok() {
                let _ = std::fs::rename(&tmp, &p);
            }
        }
    }

    /// Thêm một mục mới chép. Trùng nội dung → đưa mục cũ lên đầu. Trả về true nếu danh sách thay đổi.
    pub fn ingest(&mut self, data: ClipData, id: u64, now: i64) -> bool {
        let data = match data {
            ClipData::Text(t) if t.trim().is_empty() => return false,
            ClipData::Text(t) if t.len() > MAX_TEXT => ClipData::Text(t.chars().take(MAX_TEXT).collect()),
            ClipData::Image(b) if b.len() > MAX_IMAGE || b.is_empty() => return false,
            ClipData::Files(f) if f.is_empty() => return false,
            d => d,
        };
        let h = hash_of(&data);
        if let Some(pos) = self.items.iter().position(|c| c.hash == h) {
            let mut c = self.items.remove(pos);
            c.used = now;
            self.items.insert(0, c);
            self.save();
            return true;
        }
        let mut clip = Clip { id, kind: ClipKind::Text, text: String::new(), files: vec![], image: None, bytes: 0, hash: h, created: now, used: now, pinned: false, uses: 0, label: None, pinned_at: 0 };
        match data {
            ClipData::Text(t) => {
                clip.bytes = t.len() as u64;
                clip.text = t;
            }
            ClipData::Files(f) => {
                clip.kind = ClipKind::Files;
                clip.bytes = f.iter().filter_map(|p| std::fs::metadata(p).ok()).map(|m| m.len()).sum();
                clip.files = f;
            }
            ClipData::Image(b) => {
                let name = format!("{id}.png");
                if std::fs::write(self.dir.join(&name), &b).is_err() {
                    return false;
                }
                clip.kind = ClipKind::Image;
                clip.bytes = b.len() as u64;
                clip.image = Some(name);
            }
        }
        self.items.insert(0, clip);
        self.trim();
        self.save();
        true
    }

    /// Giữ tối đa `max` mục chưa ghim; mục ghim không bao giờ bị xoá tự động.
    fn trim(&mut self) {
        let mut unpinned = 0;
        let mut drop = vec![];
        for (i, c) in self.items.iter().enumerate() {
            if !c.pinned {
                unpinned += 1;
                if unpinned > self.max {
                    drop.push(i);
                }
            }
        }
        for i in drop.into_iter().rev() {
            let c = self.items.remove(i);
            self.remove_files(&c);
        }
    }

    fn remove_files(&self, c: &Clip) {
        if let Some(img) = &c.image {
            let _ = std::fs::remove_file(self.dir.join(img));
        }
    }

    pub fn get(&self, id: u64) -> Option<&Clip> {
        self.items.iter().find(|c| c.id == id)
    }

    pub fn data(&self, id: u64) -> Option<ClipData> {
        let c = self.get(id)?;
        Some(match c.kind {
            ClipKind::Text => ClipData::Text(c.text.clone()),
            ClipKind::Files => ClipData::Files(c.files.iter().filter(|p| Path::new(p).exists()).cloned().collect()),
            ClipKind::Image => ClipData::Image(std::fs::read(self.dir.join(c.image.as_ref()?)).ok()?),
        })
    }

    pub fn image_bytes(&self, id: u64) -> Option<Vec<u8>> {
        let c = self.get(id)?;
        std::fs::read(self.dir.join(c.image.as_ref()?)).ok()
    }

    pub fn mark_used(&mut self, id: u64, now: i64) {
        if let Some(c) = self.items.iter_mut().find(|c| c.id == id) {
            c.used = now;
            c.uses += 1;
        }
        self.save();
    }

    pub fn toggle_pin(&mut self, id: u64) {
        let now = chrono::Utc::now().timestamp_millis();
        if let Some(c) = self.items.iter_mut().find(|c| c.id == id) {
            c.pinned = !c.pinned;
            c.pinned_at = if c.pinned { now } else { 0 };
            if !c.pinned {
                c.label = None;
            }
        }
        self.save();
    }

    /// Đặt tên gợi nhớ cho mục (rỗng = bỏ tên). Đặt tên cũng tự ghim luôn.
    pub fn set_label(&mut self, id: u64, label: &str) {
        let now = chrono::Utc::now().timestamp_millis();
        if let Some(c) = self.items.iter_mut().find(|c| c.id == id) {
            let l: String = label.trim().chars().take(60).collect();
            c.label = if l.is_empty() { None } else { Some(l) };
            if c.label.is_some() && !c.pinned {
                c.pinned = true;
                c.pinned_at = now;
            }
        }
        self.save();
    }

    pub fn delete(&mut self, id: u64) {
        if let Some(pos) = self.items.iter().position(|c| c.id == id) {
            let c = self.items.remove(pos);
            self.remove_files(&c);
            self.save();
        }
    }

    /// Xoá lịch sử, giữ mục đã ghim
    pub fn clear(&mut self) {
        let (keep, gone): (Vec<Clip>, Vec<Clip>) = self.items.drain(..).partition(|c| c.pinned);
        for c in &gone {
            self.remove_files(c);
        }
        self.items = keep;
        self.save();
    }

    /// `kind`: all | text | image | files | pinned (theo thứ tự ghim) | history (chưa ghim)
    pub fn list(&self, query: &str, kind: &str) -> Vec<ClipView> {
        let q = crate::nlp::fold(query.trim());
        let mut items: Vec<&Clip> = self
            .items
            .iter()
            .filter(|c| match kind {
                "text" => c.kind == ClipKind::Text,
                "image" => c.kind == ClipKind::Image,
                "files" => c.kind == ClipKind::Files,
                "pinned" => c.pinned,
                "history" => !c.pinned,
                _ => true,
            })
            .filter(|c| {
                q.is_empty()
                    || crate::nlp::fold(&c.text).contains(&q)
                    || c.label.as_deref().is_some_and(|l| crate::nlp::fold(l).contains(&q))
                    || c.files.iter().any(|f| crate::nlp::fold(f).contains(&q))
            })
            .collect();
        if kind == "pinned" {
            items.sort_by_key(|c| (c.pinned_at, c.id));
        }
        items
            .into_iter()
            .take(300)
            .map(|c| ClipView {
                id: c.id,
                kind: c.kind,
                preview: c.text.chars().take(400).collect(),
                chars: c.text.chars().count(),
                files: c
                    .files
                    .iter()
                    .map(|p| FileRef {
                        name: Path::new(p).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| p.clone()),
                        path: p.clone(),
                        exists: Path::new(p).exists(),
                    })
                    .collect(),
                bytes: c.bytes,
                created: c.created,
                used: c.used,
                pinned: c.pinned,
                uses: c.uses,
                label: c.label.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_kept_forever_ordered_and_labelled() {
        let d = tempfile::tempdir().unwrap();
        let mut s = ClipStore::open(d.path(), 10);
        s.ingest(ClipData::Text("STK 0123".into()), 1, 1);
        s.ingest(ClipData::Text("mẫu trả lời".into()), 2, 2);
        s.set_label(1, "STK công ty");
        s.toggle_pin(2);
        for i in 0..30 {
            s.ingest(ClipData::Text(format!("rác {i}")), 100 + i, 10 + i as i64);
        }
        s.clear();
        let pinned = s.list("", "pinned");
        assert_eq!(pinned.iter().map(|c| c.id).collect::<Vec<_>>(), vec![1, 2], "ghim còn mãi, theo thứ tự ghim");
        assert_eq!(pinned[0].label.as_deref(), Some("STK công ty"));
        assert_eq!(s.list("cong ty", "all").len(), 1, "tìm được theo tên");
        assert!(s.list("", "history").is_empty());
        let reopened = ClipStore::open(d.path(), 10);
        assert_eq!(reopened.list("", "pinned").len(), 2, "vẫn còn sau khi mở lại app");
    }

    #[test]
    fn dedupe_trim_pin_and_images() {
        let d = tempfile::tempdir().unwrap();
        let mut s = ClipStore::open(d.path(), 10);
        assert!(s.ingest(ClipData::Text("xin chào".into()), 1, 1));
        assert!(!s.ingest(ClipData::Text("   ".into()), 2, 2), "chuỗi rỗng bỏ qua");
        s.ingest(ClipData::Text("hello".into()), 3, 3);
        s.ingest(ClipData::Text("xin chào".into()), 4, 4); // trùng → lên đầu
        assert_eq!(s.items.len(), 2);
        assert_eq!(s.items[0].id, 1);
        s.toggle_pin(1);
        for i in 0..20 {
            s.ingest(ClipData::Text(format!("mục {i}")), 100 + i, 10 + i as i64);
        }
        assert!(s.get(1).is_some(), "mục ghim không bị xoá");
        assert_eq!(s.items.iter().filter(|c| !c.pinned).count(), 10);
        // ảnh
        s.ingest(ClipData::Image(vec![137, 80, 78, 71, 1, 2, 3]), 500, 50);
        assert_eq!(s.image_bytes(500).unwrap().len(), 7);
        assert!(d.path().join("clips/500.png").exists());
        s.delete(500);
        assert!(!d.path().join("clips/500.png").exists());
        // file
        let f = d.path().join("Hợp đồng.pdf");
        std::fs::write(&f, b"x").unwrap();
        s.ingest(ClipData::Files(vec![f.to_string_lossy().to_string()]), 600, 60);
        assert_eq!(s.list("hop dong", "files").len(), 1, "tìm không dấu");
        // lưu & mở lại
        let s2 = ClipStore::open(d.path(), 10);
        assert_eq!(s2.items.len(), s.items.len());
        let mut s3 = s2;
        s3.clear();
        assert_eq!(s3.items.len(), 1, "chỉ còn mục ghim");
    }
}
