//! Cấu hình người dùng: ~/.config/mini-asi/clean.toml

use crate::rules::Tier;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// Đường dẫn/mẫu người dùng muốn bảo vệ, ví dụ "~/Library/Caches/com.spotify.client"
    #[serde(default)]
    pub whitelist: Vec<String>,
    #[serde(default)]
    pub disabled_rules: Vec<String>,
    /// Thư mục dự án bổ sung để tìm node_modules, target...
    #[serde(default)]
    pub project_roots: Vec<String>,
    /// File luật tuỳ chỉnh thay cho luật nhúng sẵn (dành cho cập nhật luật qua mạng).
    #[serde(default)]
    pub rules_file: Option<String>,
}

pub struct Paths {
    pub home: PathBuf,
    pub config_file: PathBuf,
    pub log_file: PathBuf,
}

impl Paths {
    /// MINIASI_HOME cho phép chạy trên một "home giả" khi kiểm thử.
    pub fn detect() -> Self {
        let home = std::env::var_os("MINIASI_HOME")
            .map(PathBuf::from)
            .or_else(dirs::home_dir)
            .expect("không xác định được thư mục home");
        Self::for_home(home)
    }

    pub fn for_home(home: PathBuf) -> Self {
        let config_file = home.join(".config/mini-asi/clean.toml");
        let log_file = if cfg!(target_os = "macos") {
            home.join("Library/Logs/MiniAsi/clean.jsonl")
        } else if cfg!(target_os = "windows") {
            home.join("AppData/Local/MiniAsi/clean.jsonl")
        } else {
            home.join(".local/state/mini-asi/clean.jsonl")
        };
        Self { home, config_file, log_file }
    }
}

pub fn load(path: &Path) -> Config {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

/// Giấy phép. Bản thử nghiệm: đọc biến môi trường MINIASI_TIER=pro.
/// Bản thật sẽ xác thực license key (ký số) trong app Tauri.
pub fn current_tier() -> Tier {
    match std::env::var("MINIASI_TIER").as_deref() {
        Ok("pro") => Tier::Pro,
        _ => Tier::Free,
    }
}
