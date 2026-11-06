//! Nhật ký thao tác (JSON Lines) — để xem lịch sử và hoàn tác file đã vào Thùng rác.

use crate::rules::Action;
use crate::scan::Candidate;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, Write};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpRecord {
    pub at: String,
    pub rule_id: String,
    pub path: PathBuf,
    pub bytes: u64,
    pub action: Action,
}

impl OpRecord {
    pub fn new(c: &Candidate) -> Self {
        Self {
            at: humantime::format_rfc3339_seconds(std::time::SystemTime::now()).to_string(),
            rule_id: c.rule_id.clone(),
            path: c.path.clone(),
            bytes: c.bytes,
            action: c.action,
        }
    }
}

pub struct OpLog {
    pub file: PathBuf,
}

impl OpLog {
    pub fn new(file: PathBuf) -> Self {
        Self { file }
    }

    pub fn append(&self, rec: &OpRecord) {
        if let Some(d) = self.file.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&self.file) {
            if let Ok(s) = serde_json::to_string(rec) {
                let _ = writeln!(f, "{s}");
            }
        }
    }

    pub fn read_last(&self, n: usize) -> Vec<OpRecord> {
        let Ok(f) = std::fs::File::open(&self.file) else { return vec![] };
        let all: Vec<OpRecord> = std::io::BufReader::new(f)
            .lines()
            .map_while(Result::ok)
            .filter_map(|l| serde_json::from_str(&l).ok())
            .collect();
        let start = all.len().saturating_sub(n);
        all[start..].to_vec()
    }
}
