//! Trạng thái ổ đĩa — dùng để linh vật đổi biểu cảm / màu cảnh báo.

use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DiskLevel {
    /// Còn trống >= 15%
    Ok,
    /// Còn trống 5–15% → linh vật lo lắng, viền cam
    Warn,
    /// Còn trống < 5% → báo động
    Critical,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiskStatus {
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub free_percent: f64,
    pub level: DiskLevel,
}

pub fn level_for(free_percent: f64) -> DiskLevel {
    if free_percent < 5.0 {
        DiskLevel::Critical
    } else if free_percent < 15.0 {
        DiskLevel::Warn
    } else {
        DiskLevel::Ok
    }
}

#[cfg(unix)]
pub fn status(path: &Path) -> Option<DiskStatus> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let c = CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        return None;
    }
    let frsize = s.f_frsize as u64;
    let total = s.f_blocks as u64 * frsize;
    // f_bavail = dung lượng người dùng thường dùng được.
    // Lưu ý macOS: dung lượng "purgeable" (iCloud, snapshot) không nằm trong số này.
    let free = s.f_bavail as u64 * frsize;
    let pct = if total > 0 { free as f64 * 100.0 / total as f64 } else { 0.0 };
    Some(DiskStatus { total_bytes: total, free_bytes: free, free_percent: pct, level: level_for(pct) })
}

#[cfg(not(unix))]
pub fn status(_path: &Path) -> Option<DiskStatus> {
    None // TODO giai đoạn 3: GetDiskFreeSpaceExW
}

/// Định dạng dung lượng theo hệ 1000 giống Finder của macOS.
pub fn human(bytes: u64) -> String {
    const U: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1000.0 && i < U.len() - 1 {
        v /= 1000.0;
        i += 1;
    }
    if i == 0 { format!("{bytes} B") } else { format!("{v:.1} {}", U[i]) }
}
