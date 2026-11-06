//! Định nghĩa luật dọn dẹp và nạp luật theo hệ điều hành.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    /// Dữ liệu tự tái tạo được — dọn bằng 1 click.
    Safe,
    /// Cần người dùng xem lại và chọn.
    Review,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Free,
    Pro,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Contents,
    Glob,
    Artifacts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    /// Xoá hẳn — chỉ dùng cho cache/dữ liệu tái tạo được.
    Delete,
    /// Chuyển vào Thùng rác — hoàn tác được.
    Trash,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactSpec {
    pub dir: String,
    pub marker: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub category: String,
    pub level: Level,
    pub tier: Tier,
    pub kind: Kind,
    pub paths: Vec<String>,
    pub action: Action,
    #[serde(default)]
    pub patterns: Vec<String>,
    #[serde(default = "default_depth")]
    pub depth: usize,
    #[serde(default)]
    pub min_age_days: u64,
    #[serde(default)]
    pub min_size_mb: u64,
    /// Bỏ qua mục có file thay đổi trong N phút gần đây (đang được dùng).
    #[serde(default)]
    pub active_guard_minutes: u64,
    #[serde(default)]
    pub exclude: Vec<String>,
    #[serde(default)]
    pub artifacts: Vec<ArtifactSpec>,
}

fn default_depth() -> usize {
    1
}

#[derive(Debug, Deserialize)]
struct RuleFile {
    rule: Vec<Rule>,
}

pub const MACOS_RULES: &str = include_str!("../rules/macos.toml");
pub const LINUX_RULES: &str = include_str!("../rules/linux.toml");
pub const WINDOWS_RULES: &str = include_str!("../rules/windows.toml");

/// Bộ luật nhúng sẵn cho hệ điều hành hiện tại.
pub fn builtin_rules_src() -> &'static str {
    if cfg!(target_os = "macos") {
        MACOS_RULES
    } else if cfg!(target_os = "windows") {
        WINDOWS_RULES
    } else {
        LINUX_RULES
    }
}

pub fn parse_rules(src: &str) -> Result<Vec<Rule>, String> {
    let f: RuleFile = toml::from_str(src).map_err(|e| format!("Lỗi đọc file luật: {e}"))?;
    let mut seen = std::collections::HashSet::new();
    for r in &f.rule {
        if !seen.insert(r.id.clone()) {
            return Err(format!("Trùng id luật: {}", r.id));
        }
        if r.kind == Kind::Artifacts && r.artifacts.is_empty() {
            return Err(format!("Luật {} kiểu artifacts nhưng thiếu danh sách artifacts", r.id));
        }
        if r.kind == Kind::Glob && r.patterns.is_empty() {
            return Err(format!("Luật {} kiểu glob nhưng thiếu patterns", r.id));
        }
        // Dữ liệu người dùng (review) mà xoá hẳn thì chỉ cho phép với luật đã được duyệt kỹ:
        // file cài đặt / file lớn / downloads... bắt buộc vào Thùng rác.
        if r.category == "files" && r.action == Action::Delete {
            return Err(format!("Luật {} thuộc nhóm files phải dùng action = \"trash\"", r.id));
        }
    }
    Ok(f.rule)
}

/// Mở rộng "~" thành thư mục home (có thể là home giả lập khi test).
pub fn expand(p: &str, home: &Path) -> PathBuf {
    if p == "~" {
        home.to_path_buf()
    } else if let Some(rest) = p.strip_prefix("~/") {
        home.join(rest)
    } else {
        PathBuf::from(p)
    }
}
