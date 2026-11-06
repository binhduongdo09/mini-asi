//! asi-clean — bộ máy dọn dẹp của Mini-Asi.
//!
//! Nguyên tắc:
//! 1. Quét không bao giờ thay đổi ổ đĩa.
//! 2. Cache tái tạo được → xoá hẳn; dữ liệu người dùng → Thùng rác.
//! 3. Mọi mục được Guard kiểm tra hai lần: lúc quét và ngay trước khi xoá.
//! 4. Mọi thao tác được ghi nhật ký.
//! 5. Luật Pro vẫn được quét để hiển thị "bản Pro giải phóng thêm X GB" nhưng không thực thi.

pub mod clean;
pub mod config;
pub mod disk;
pub mod guard;
pub mod oplog;
pub mod rules;
pub mod scan;

use std::path::PathBuf;

/// API gọn cho app Tauri gọi vào.
pub struct Engine {
    pub paths: config::Paths,
    pub config: config::Config,
    pub rules: Vec<rules::Rule>,
    pub guard: guard::Guard,
    pub tier: rules::Tier,
}

impl Engine {
    pub fn new(paths: config::Paths, tier: rules::Tier) -> Result<Self, String> {
        let config = config::load(&paths.config_file);
        let src = match &config.rules_file {
            Some(f) => std::fs::read_to_string(rules::expand(f, &paths.home)).map_err(|e| e.to_string())?,
            None => rules::builtin_rules_src().to_string(),
        };
        let rules = rules::parse_rules(&src)?;
        let guard = guard::Guard::new(paths.home.clone(), &config.whitelist);
        Ok(Self { paths, config, rules, guard, tier })
    }

    pub fn scan(&self, only_rules: Vec<String>) -> scan::ScanReport {
        let opts = scan::ScanOptions {
            only_rules,
            disabled_rules: self.config.disabled_rules.clone(),
            extra_project_roots: self.config.project_roots.clone(),
            tier: Some(self.tier),
        };
        scan::Scanner::new(&self.guard).scan(&self.rules, &opts)
    }

    pub fn clean(&self, report: &scan::ScanReport, sel: &clean::Selection, dry_run: bool) -> clean::CleanReport {
        let log = oplog::OpLog::new(self.paths.log_file.clone());
        clean::execute(report, sel, &self.guard, Some(&log), dry_run)
    }

    pub fn disk(&self) -> Option<disk::DiskStatus> {
        disk::status(&self.paths.home)
    }

    pub fn history(&self, n: usize) -> Vec<oplog::OpRecord> {
        oplog::OpLog::new(self.paths.log_file.clone()).read_last(n)
    }

    pub fn log_file(&self) -> PathBuf {
        self.paths.log_file.clone()
    }
}
