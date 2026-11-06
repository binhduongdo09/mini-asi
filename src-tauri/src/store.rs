//! Lưu dữ liệu cục bộ dạng JSON trong thư mục dữ liệu của app.
//! Ghi an toàn: ghi ra file tạm rồi đổi tên (không bao giờ để file hỏng giữa chừng).

use chrono::{Local, NaiveDateTime, TimeZone};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: u64,
    pub title: String,
    /// Hạn (unix ms). None = không có hạn.
    pub due: Option<i64>,
    pub created: i64,
    pub done_at: Option<i64>,
    /// Đã bắn thông báo nhắc cho hạn hiện tại chưa
    #[serde(default)]
    pub notified: bool,
    /// Lần sửa cuối (unix ms) — dùng khi đồng bộ 2 chiều
    #[serde(default)]
    pub updated: i64,
    /// id bên Lời nhắc (Reminders) của Apple
    #[serde(default)]
    pub ext_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: u64,
    pub text: String,
    pub updated: i64,
    #[serde(default)]
    pub pinned: bool,
    /// id bên app Ghi chú (Notes) của Apple
    #[serde(default)]
    pub ext_id: Option<String>,
}

/// Trạng thái đồng bộ với Apple
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SyncState {
    pub last_tasks: i64,
    pub last_notes: i64,
    /// id phía Apple của các mục đã xoá ở Mini-Asi → xoá bên kia ở lần đồng bộ tới
    pub deleted_reminders: Vec<String>,
    pub deleted_notes: Vec<String>,
    /// id Lời nhắc đã thấy ở lần kéo về trước (chỉ những mục này mới có thể bị xoá theo phía Apple)
    pub seen_reminders: Vec<String>,
    pub last_error: Option<String>,
    pub last_run: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub break_enabled: bool,
    pub break_minutes: u32,
    pub water_enabled: bool,
    pub water_minutes: u32,
    pub sleep_hour: u32,
    /// Chế độ tập trung: tắt màu đỏ & thông báo nhẹ
    pub focus_mode: bool,
    pub launch_at_login: bool,
    pub license_key: Option<String>,
    pub pdf_quality: String,
    pub shortcut: String,
    // Clipboard
    pub clip_enabled: bool,
    pub clip_max: usize,
    pub clip_shortcut: String,
    /// Chọn mục là dán luôn vào app đang dùng (cần quyền Trợ năng)
    pub clip_autopaste: bool,
    // Đồng bộ Apple
    pub sync_reminders: bool,
    pub reminders_list: String,
    pub sync_notes: bool,
    pub notes_folder: String,
    pub show_calendar: bool,
    pub event_alert_minutes: u32,
    // Giọng nói & nhật ký
    pub voice_lang: String,
    pub journal_enabled: bool,
    pub journal_hour: u32,
    // Threads
    pub threads_app_id: String,
    pub threads_redirect: String,
    pub threads_notify: bool,
    pub threads_user: String,
    pub threads_expires: i64,
    // Bộ gõ: "auto" (tự dùng khi máy đang bật bộ gõ tiếng Việt) | "telex" | "vni" | "off"
    pub viet_engine: String,
    // Sao lưu & đồng bộ nhiều máy
    pub backup_dir: String,
    pub auto_backup: bool,
    pub device_sync: bool,
    pub device_id: String,
    pub device_name: String,
    pub last_backup: i64,
    pub last_device_sync: i64,
    // Kích thước bảng do người dùng kéo (0 = mặc định)
    pub panel_w: f64,
    pub panel_h: f64,
    pub threads_w: f64,
    pub threads_h: f64,
    /// (cũ, không dùng nữa)
    pub threads_inline: bool,
    /// Thử nghiệm: nhúng Threads vào bảng tai thỏ (mặc định tắt — dùng cửa sổ riêng)
    pub threads_embed_beta: bool,
    /// "notch" | "pill" | "bubble"
    pub display_mode: String,
    pub bubble_x: f64,
    pub bubble_y: f64,
    /// Asi đọc to câu trả lời
    pub voice_reply: bool,
    /// Giọng đọc của Asi: "system" (Linh/Samantha của máy) | "vieneu" (VieNeu-TTS, cần tải)
    pub tts_engine: String,
    pub tts_voice: String,
    /// Tính năng hiện trên thanh tab (còn lại nằm trong trang "Tất cả")
    pub visible_tabs: Vec<String>,
    /// Thứ hiện trên tai thỏ khi thu gọn: focus, overdue, event, next, time, tasks, notes, clips, disk, mic
    pub idle_items: Vec<String>,
    /// Độ rộng hai bên tai thỏ: s | m | l
    pub wing_size: String,
    /// Học từ mới tiếng Anh: bật/tắt, số từ mỗi lần mở tai thỏ
    pub vocab_enabled: bool,
    pub vocab_count: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            break_enabled: true,
            break_minutes: 50,
            water_enabled: true,
            water_minutes: 60,
            sleep_hour: 23,
            focus_mode: false,
            launch_at_login: false,
            license_key: None,
            pdf_quality: "ebook".into(),
            shortcut: "CmdOrCtrl+Shift+Space".into(),
            clip_enabled: true,
            clip_max: 200,
            clip_shortcut: if cfg!(target_os = "macos") { "Control+Shift+V".into() } else { "Alt+Shift+V".into() },
            clip_autopaste: false,
            sync_reminders: false,
            reminders_list: "Mini-Asi".into(),
            sync_notes: false,
            notes_folder: "Mini-Asi".into(),
            show_calendar: false,
            event_alert_minutes: 10,
            voice_lang: "vi-VN".into(),
            journal_enabled: true,
            journal_hour: 21,
            threads_app_id: String::new(),
            threads_redirect: "https://localhost/".into(),
            threads_notify: true,
            threads_user: String::new(),
            threads_expires: 0,
            viet_engine: "auto".into(),
            backup_dir: String::new(),
            auto_backup: true,
            device_sync: false,
            device_id: String::new(),
            device_name: String::new(),
            last_backup: 0,
            last_device_sync: 0,
            panel_w: 0.0,
            panel_h: 0.0,
            threads_w: 0.0,
            threads_h: 0.0,
            threads_inline: false,
            threads_embed_beta: false,
            display_mode: "notch".into(),
            bubble_x: 0.0,
            bubble_y: 0.0,
            voice_reply: true,
            tts_engine: "system".into(),
            tts_voice: crate::tts::DEFAULT_VOICE.into(),
            visible_tabs: ["home", "tasks", "notes", "clips", "journal", "words", "threads"].iter().map(|s| s.to_string()).collect(),
            wing_size: "s".into(),
            vocab_enabled: true,
            vocab_count: 3,
            idle_items: ["focus", "overdue", "event", "next", "tasks", "mic"].iter().map(|s| s.to_string()).collect(),
        }
    }
}

pub struct Store {
    dir: PathBuf,
    pub tasks: Vec<Task>,
    pub notes: Vec<Note>,
    pub settings: Settings,
    pub sync: SyncState,
    /// Dấu vết các mục đã xoá → để máy khác cũng xoá khi đồng bộ
    pub tombstones: Vec<crate::backup::Tomb>,
    next_id: u64,
}

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub fn local_to_ms(dt: NaiveDateTime) -> i64 {
    Local
        .from_local_datetime(&dt)
        .earliest()
        .map(|d| d.timestamp_millis())
        .unwrap_or_else(|| dt.and_utc().timestamp_millis())
}

fn load<T: DeserializeOwned + Default>(p: &Path) -> T {
    std::fs::read_to_string(p).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn save<T: Serialize>(p: &Path, v: &T) -> Result<(), String> {
    let tmp = p.with_extension("json.tmp");
    let s = serde_json::to_string_pretty(v).map_err(|e| e.to_string())?;
    std::fs::write(&tmp, s).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, p).map_err(|e| e.to_string())
}

impl Store {
    pub fn open(dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&dir);
        let tasks: Vec<Task> = load(&dir.join("tasks.json"));
        let notes: Vec<Note> = load(&dir.join("notes.json"));
        let settings: Settings = load(&dir.join("settings.json"));
        let sync: SyncState = load(&dir.join("sync.json"));
        let tombstones: Vec<crate::backup::Tomb> = load(&dir.join("tombstones.json"));
        let next_id = tasks.iter().map(|t| t.id).chain(notes.iter().map(|n| n.id)).max().unwrap_or(0) + 1;
        Self { dir, tasks, notes, settings, sync, tombstones, next_id }
    }

    fn id(&mut self) -> u64 {
        // id tăng dần, kết hợp thời gian để không trùng giữa các lần chạy
        let t = now_ms() as u64;
        self.next_id = self.next_id.max(t) + 1;
        self.next_id
    }

    pub fn save_tasks(&self) -> Result<(), String> {
        save(&self.dir.join("tasks.json"), &self.tasks)
    }
    pub fn save_notes(&self) -> Result<(), String> {
        save(&self.dir.join("notes.json"), &self.notes)
    }
    pub fn dir(&self) -> &Path {
        &self.dir
    }
    pub fn tomb(&mut self, kind: &str, id: String) {
        self.tombstones.push(crate::backup::Tomb { kind: kind.into(), id, at: now_ms() });
        // giữ gọn: 2000 dấu vết gần nhất
        let n = self.tombstones.len();
        if n > 2000 {
            self.tombstones.drain(0..n - 2000);
        }
        let _ = save(&self.dir.join("tombstones.json"), &self.tombstones);
    }
    pub fn save_tombstones(&self) -> Result<(), String> {
        save(&self.dir.join("tombstones.json"), &self.tombstones)
    }
    pub fn save_sync(&self) -> Result<(), String> {
        save(&self.dir.join("sync.json"), &self.sync)
    }
    pub fn save_settings(&self) -> Result<(), String> {
        save(&self.dir.join("settings.json"), &self.settings)
    }

    // ── Task
    pub fn add_task(&mut self, title: String, due: Option<i64>) -> Task {
        let now = now_ms();
        let t = Task { id: self.id(), title, due, created: now, done_at: None, notified: false, updated: now, ext_id: None };
        self.tasks.push(t.clone());
        let _ = self.save_tasks();
        t
    }

    pub fn task_mut(&mut self, id: u64) -> Option<&mut Task> {
        self.tasks.iter_mut().find(|t| t.id == id)
    }

    pub fn delete_task(&mut self, id: u64) {
        if let Some(ext) = self.tasks.iter().find(|t| t.id == id).and_then(|t| t.ext_id.clone()) {
            self.sync.deleted_reminders.push(ext);
            let _ = self.save_sync();
        }
        self.tasks.retain(|t| t.id != id);
        let _ = self.save_tasks();
        self.tomb("task", id.to_string());
    }

    /// Tạo id mới (dùng chung cho mọi loại dữ liệu)
    pub fn new_id(&mut self) -> u64 {
        self.id()
    }

    // ── Note
    pub fn upsert_note(&mut self, id: Option<u64>, text: String) -> Note {
        let now = now_ms();
        if let Some(id) = id {
            if let Some(n) = self.notes.iter_mut().find(|n| n.id == id) {
                n.text = text;
                n.updated = now;
                let out = n.clone();
                let _ = self.save_notes();
                return out;
            }
        }
        let n = Note { id: self.id(), text, updated: now, pinned: false, ext_id: None };
        self.notes.push(n.clone());
        let _ = self.save_notes();
        n
    }

    pub fn delete_note(&mut self, id: u64) {
        if let Some(ext) = self.notes.iter().find(|n| n.id == id).and_then(|n| n.ext_id.clone()) {
            self.sync.deleted_notes.push(ext);
            let _ = self.save_sync();
        }
        self.notes.retain(|n| n.id != id);
        let _ = self.save_notes();
        self.tomb("note", id.to_string());
    }
}

/// Điểm "trễ việc" → mức màu 0..3 và độ đỏ liên tục 0..1.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Lateness {
    pub overdue: usize,
    pub level: u8,
    pub heat: f64,
}

pub fn lateness(tasks: &[Task], now: i64) -> Lateness {
    let mut overdue = 0usize;
    let mut score = 0.0f64;
    let mut worst_h = 0.0f64;
    for t in tasks.iter().filter(|t| t.done_at.is_none()) {
        if let Some(d) = t.due {
            if d < now {
                overdue += 1;
                let h = (now - d) as f64 / 3_600_000.0;
                worst_h = worst_h.max(h);
                // mỗi việc trễ 1 điểm, trễ càng lâu càng nặng (tối đa +2)
                score += 1.0 + (h / 12.0).min(2.0);
            }
        }
    }
    let level = if overdue == 0 {
        0
    } else if overdue >= 5 || worst_h >= 24.0 {
        3
    } else if overdue >= 3 || worst_h >= 4.0 {
        2
    } else {
        1
    };
    Lateness { overdue, level, heat: (score / 8.0).min(1.0) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(due_h_ago: Option<f64>, done: bool) -> Task {
        let now = 1_000_000_000_000i64;
        Task {
            id: 1,
            title: "x".into(),
            due: due_h_ago.map(|h| now - (h * 3_600_000.0) as i64),
            created: 0,
            done_at: if done { Some(1) } else { None },
            notified: false,
            updated: 0,
            ext_id: None,
        }
    }

    #[test]
    fn lateness_levels() {
        let now = 1_000_000_000_000i64;
        assert_eq!(lateness(&[], now).level, 0);
        assert_eq!(lateness(&[task(Some(-1.0), false)], now).level, 0, "chưa tới hạn");
        assert_eq!(lateness(&[task(Some(1.0), true)], now).level, 0, "đã xong");
        assert_eq!(lateness(&[task(Some(1.0), false)], now).level, 1);
        assert_eq!(lateness(&[task(Some(5.0), false)], now).level, 2);
        assert_eq!(lateness(&vec![task(Some(0.5), false); 3], now).level, 2);
        assert_eq!(lateness(&[task(Some(30.0), false)], now).level, 3);
        assert_eq!(lateness(&vec![task(Some(0.5), false); 5], now).level, 3);
        let h1 = lateness(&[task(Some(1.0), false)], now).heat;
        let h2 = lateness(&vec![task(Some(30.0), false); 4], now).heat;
        assert!(h1 < h2 && h2 <= 1.0);
    }

    #[test]
    fn store_roundtrip() {
        let d = tempfile::tempdir().unwrap();
        let mut s = Store::open(d.path().to_path_buf());
        let t = s.add_task("A".into(), Some(5));
        let n = s.upsert_note(None, "ghi chú".into());
        s.upsert_note(Some(n.id), "ghi chú 2".into());
        s.settings.break_minutes = 40;
        s.save_settings().unwrap();
        let s2 = Store::open(d.path().to_path_buf());
        assert_eq!(s2.tasks.len(), 1);
        assert_eq!(s2.tasks[0].id, t.id);
        assert_eq!(s2.notes[0].text, "ghi chú 2");
        assert_eq!(s2.settings.break_minutes, 40);
    }
}
