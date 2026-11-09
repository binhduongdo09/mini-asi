//! Trạng thái dùng chung của app (AppState), hình học màn hình, các kiểu & tiện ích nhỏ dùng khắp nơi.

#[allow(unused_imports)]
use crate::prelude::*;

pub struct AppState {
    pub store: Mutex<Store>,
    pub health: Mutex<health::HealthState>,
    pub geo: Mutex<Geometry>,
    pub last_scan: Mutex<Option<asi_clean::scan::ScanReport>>,
    pub clips: Mutex<clipboard::ClipStore>,
    pub journal: Mutex<journal::Journal>,
    pub events: Mutex<Vec<EventView>>,
    pub recorder: Mutex<Option<voice::Recorder>>,
    pub sync_tx: Mutex<Option<mpsc::Sender<()>>>,
    pub alerted: Mutex<HashSet<String>>,
    /// Ngày đã hỏi nhật ký + hoãn tới khi nào
    pub journal_ask: Mutex<(String, i64)>,
    pub threads: Mutex<ThreadsCache>,
    /// Bộ gõ hệ thống trước khi Mini-Asi tạm chuyển sang ABC
    pub ime_prev: Mutex<Option<String>>,
    /// Bộ gõ hệ thống thấy được ở lần mở tai thỏ gần nhất (hiện trong Cài đặt)
    pub ime_last: Mutex<String>,
    pub data_dir: PathBuf,
    /// Giọng đọc Asi (Piper, chạy trên máy)
    pub tts: Arc<tts::Tts>,
    pub vocab: Mutex<vocab::Vocab>,
    /// Hội thoại gần đây với Asi (câu hỏi, câu trả lời, thời điểm) — để Apple Intelligence hiểu câu hỏi nối tiếp
    pub chat_log: Mutex<Vec<(String, String, i64)>>,
    /// Trạng thái kiểm tra & tải bản cập nhật
    pub update: Mutex<updater::UpdateProgress>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ThreadsCache {
    pub profile: Option<threads::Profile>,
    pub overview: Option<threads::Overview>,
    pub posts: Vec<threads::Post>,
    pub mentions: Vec<threads::Post>,
    pub unread: u32,
    pub mention_unread: u32,
    pub fetched: i64,
    pub error: Option<String>,
    #[serde(skip)]
    pub seen: HashSet<String>,
    #[serde(skip)]
    pub primed: bool,
}

/// id phím tắt mở clipboard (để phân biệt với phím tắt mở tai thỏ)
pub(crate) static CLIP_SHORTCUT: OnceLock<u32> = OnceLock::new();

#[derive(Debug, Clone, Serialize)]
pub struct EventView {
    pub id: String,
    pub title: String,
    pub start: i64,
    pub end: i64,
    pub all_day: bool,
    pub location: String,
    pub calendar: String,
}

/// Báo luồng đồng bộ chạy sớm (sau khi có thay đổi)
pub(crate) fn kick_sync(app: &AppHandle) {
    if let Some(st) = app.try_state::<Arc<AppState>>() {
        if let Some(tx) = st.sync_tx.lock().unwrap().as_ref() {
            let _ = tx.send(());
        }
    }
}

pub(crate) type S<'a> = State<'a, Arc<AppState>>;

#[derive(Debug, Clone, Serialize, Default)]
pub struct Geometry {
    pub has_notch: bool,
    pub notch_width: f64,
    pub notch_height: f64,
    /// Vùng màn hình đặt cửa sổ (logical)
    pub screen_x: f64,
    pub screen_y: f64,
    pub screen_width: f64,
    pub screen_height: f64,
    pub platform: String,
}
