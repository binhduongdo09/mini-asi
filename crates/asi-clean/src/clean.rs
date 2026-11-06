//! Thực thi dọn dẹp. Mỗi mục được kiểm tra lại bằng Guard ngay trước khi xoá.

use crate::guard::{Guard, Verdict};
use crate::oplog::{OpLog, OpRecord};
use crate::rules::Action;
use crate::scan::{Candidate, ScanReport};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Default)]
pub struct Selection {
    /// Các luật được chọn dọn (toàn bộ mục trong luật).
    pub rules: Vec<String>,
    /// Hoặc chọn từng mục cụ thể (dùng cho nhóm "Cần xem lại").
    pub paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Failure {
    pub path: PathBuf,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct CleanReport {
    pub dry_run: bool,
    pub deleted_bytes: u64,
    pub trashed_bytes: u64,
    pub items_done: usize,
    pub failures: Vec<Failure>,
    /// Luật Pro bị bỏ qua vì đang dùng bản Free.
    pub locked_rules: Vec<String>,
}

impl CleanReport {
    /// Dung lượng ổ đĩa thực sự được giải phóng ngay (file vào Thùng rác chưa tính).
    pub fn freed_now(&self) -> u64 {
        self.deleted_bytes
    }
}

pub fn select<'r>(report: &'r ScanReport, sel: &Selection) -> (Vec<&'r Candidate>, Vec<String>) {
    let mut out = Vec::new();
    let mut locked = Vec::new();
    for r in &report.rules {
        let rule_selected = sel.rules.contains(&r.id);
        let picked: Vec<&Candidate> = r
            .items
            .iter()
            .filter(|c| rule_selected || sel.paths.contains(&c.path))
            .collect();
        if picked.is_empty() {
            continue;
        }
        if r.locked {
            locked.push(r.id.clone());
            continue;
        }
        out.extend(picked);
    }
    (out, locked)
}

pub fn execute(report: &ScanReport, sel: &Selection, guard: &Guard, log: Option<&OpLog>, dry_run: bool) -> CleanReport {
    let (items, locked_rules) = select(report, sel);
    let mut rep = CleanReport { dry_run, locked_rules, ..Default::default() };
    for c in items {
        // Kiểm tra lại: trạng thái có thể đã đổi từ lúc quét
        if let Verdict::Deny(reason) = guard.check(&c.path) {
            rep.failures.push(Failure { path: c.path.clone(), reason });
            continue;
        }
        if std::fs::symlink_metadata(&c.path).is_err() {
            continue; // đã biến mất — bỏ qua
        }
        if dry_run {
            add(&mut rep, c);
            continue;
        }
        let res = match c.action {
            Action::Delete => remove(&c.path),
            Action::Trash => trash::delete(&c.path).map_err(|e| e.to_string()),
        };
        match res {
            Ok(()) => {
                add(&mut rep, c);
                if let Some(l) = log {
                    l.append(&OpRecord::new(c));
                }
            }
            Err(reason) => rep.failures.push(Failure { path: c.path.clone(), reason }),
        }
    }
    rep
}

fn add(rep: &mut CleanReport, c: &Candidate) {
    rep.items_done += 1;
    match c.action {
        Action::Delete => rep.deleted_bytes += c.bytes,
        Action::Trash => rep.trashed_bytes += c.bytes,
    }
}

fn remove(p: &std::path::Path) -> Result<(), String> {
    let m = std::fs::symlink_metadata(p).map_err(|e| e.to_string())?;
    let r = if m.is_dir() { std::fs::remove_dir_all(p) } else { std::fs::remove_file(p) };
    r.map_err(|e| {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            "không có quyền (cần Full Disk Access)".to_string()
        } else {
            e.to_string()
        }
    })
}
