//! Lớp bảo vệ: quyết định đường dẫn nào TUYỆT ĐỐI không được đụng vào.
//! Mọi thao tác xoá đều phải đi qua `Guard::check` ngay trước khi thực thi.

use std::path::{Component, Path, PathBuf};

/// Thư mục mà bản thân nó không được xoá (con bên trong thì tuỳ luật).
const PROTECTED_EXACT: &[&str] = &[
    "", "Library", "Library/Caches", "Library/Logs", "Library/Application Support",
    "Library/Containers", "Library/Developer", "Library/Preferences",
    "Downloads", "Desktop", "Documents", "Movies", "Music", "Pictures", "Videos",
    "Public", "Applications", ".Trash", ".cache", ".local", ".local/share",
    "AppData", "AppData/Local", "AppData/Roaming",
];

/// Cây thư mục cấm hoàn toàn — không bao giờ xoá bất cứ thứ gì bên trong.
const PROTECTED_TREES: &[&str] = &[
    "Library/Keychains",
    "Library/Mobile Documents",         // iCloud Drive
    "Library/CloudStorage",             // Google Drive, OneDrive, Dropbox...
    "Library/Messages",
    "Library/Mail",
    "Library/Calendars",
    "Library/Application Support/AddressBook",
    "Library/Group Containers",
    "Library/Photos",
    "Pictures/Photos Library.photoslibrary",
    "Library/Containers/com.apple.Notes",
    ".ssh", ".gnupg", ".aws", ".kube", ".docker/config.json",
    ".config", ".local/share/keyrings",
    "AppData/Roaming/Microsoft/Credentials",
];

#[derive(Debug, Clone)]
pub struct Guard {
    home: PathBuf,
    /// Danh sách người dùng tự bảo vệ (đã mở rộng ~).
    user_whitelist: globset::GlobSet,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    Allow,
    Deny(String),
}

impl Guard {
    pub fn new(home: PathBuf, whitelist_patterns: &[String]) -> Self {
        let mut b = globset::GlobSetBuilder::new();
        for p in whitelist_patterns {
            let full = crate::rules::expand(p, &home);
            let s = full.to_string_lossy().to_string();
            // bảo vệ cả chính đường dẫn và mọi thứ bên trong
            for pat in [s.clone(), format!("{}/**", s.trim_end_matches('/'))] {
                if let Ok(g) = globset::Glob::new(&pat) {
                    b.add(g);
                }
            }
        }
        let user_whitelist = b.build().unwrap_or_else(|_| globset::GlobSet::empty());
        Self { home, user_whitelist }
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    pub fn is_whitelisted(&self, p: &Path) -> bool {
        self.user_whitelist.is_match(p)
    }

    /// Kiểm tra một đường dẫn ứng viên. Không đi theo symlink.
    pub fn check(&self, p: &Path) -> Verdict {
        // 1. Không chấp nhận đường dẫn tương đối hoặc có ".."
        if !p.is_absolute() {
            return Verdict::Deny("đường dẫn không tuyệt đối".into());
        }
        if p.components().any(|c| matches!(c, Component::ParentDir)) {
            return Verdict::Deny("đường dẫn chứa '..'".into());
        }
        // 2. Phải nằm trong thư mục home
        let rel = match p.strip_prefix(&self.home) {
            Ok(r) => r,
            Err(_) => return Verdict::Deny("nằm ngoài thư mục người dùng".into()),
        };
        // 3. Thư mục cha phải là thư mục thật (không phải symlink) để tránh bị "dẫn" ra ngoài
        let mut cur = self.home.clone();
        for c in rel.parent().unwrap_or(Path::new("")).components() {
            cur.push(c);
            if let Ok(m) = std::fs::symlink_metadata(&cur) {
                if m.file_type().is_symlink() {
                    return Verdict::Deny(format!("thư mục cha là symlink: {}", cur.display()));
                }
            }
        }
        let rel_s = rel.to_string_lossy().replace('\\', "/");
        // 4. Thư mục hệ thống quan trọng
        if PROTECTED_EXACT.iter().any(|x| *x == rel_s) {
            return Verdict::Deny("thư mục gốc được bảo vệ".into());
        }
        for t in PROTECTED_TREES {
            if rel_s == *t || rel_s.starts_with(&format!("{t}/")) {
                return Verdict::Deny(format!("nằm trong vùng cấm ({t})"));
            }
        }
        // 5. Không bao giờ đụng vào kho git
        if rel.components().any(|c| c.as_os_str() == ".git") {
            return Verdict::Deny("nằm trong kho .git".into());
        }
        // 6. Người dùng tự bảo vệ
        if self.is_whitelisted(p) {
            return Verdict::Deny("nằm trong danh sách bảo vệ của bạn".into());
        }
        Verdict::Allow
    }
}
