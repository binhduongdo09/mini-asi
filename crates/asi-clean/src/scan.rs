//! Quét: tìm ứng viên theo từng luật, tính dung lượng, áp điều kiện an toàn.
//! Quét KHÔNG BAO GIỜ thay đổi gì trên ổ đĩa.

use crate::guard::{Guard, Verdict};
use crate::rules::{expand, Action, Kind, Level, Rule, Tier};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize)]
pub struct Candidate {
    pub rule_id: String,
    pub path: PathBuf,
    /// Dung lượng thực chiếm trên đĩa (byte), đã trừ phần thuộc luật khác.
    pub bytes: u64,
    pub action: Action,
    /// Số ngày kể từ lần thay đổi gần nhất bên trong mục.
    pub age_days: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Skipped {
    pub rule_id: String,
    pub path: PathBuf,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuleReport {
    pub id: String,
    pub name: String,
    pub category: String,
    pub level: Level,
    pub tier: Tier,
    pub action: Action,
    /// true nếu luật thuộc bản Pro mà giấy phép hiện tại là Free.
    pub locked: bool,
    pub bytes: u64,
    pub items: Vec<Candidate>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanReport {
    pub rules: Vec<RuleReport>,
    pub skipped: Vec<Skipped>,
    pub total_bytes: u64,
    /// Dung lượng giải phóng được với giấy phép hiện tại.
    pub available_bytes: u64,
    /// Dung lượng chỉ bản Pro mới dọn được (để hiển thị gợi ý nâng cấp).
    pub locked_bytes: u64,
    pub safe_bytes: u64,
}

#[derive(Debug, Clone, Default)]
pub struct ScanOptions {
    /// Chỉ quét các luật này (rỗng = tất cả).
    pub only_rules: Vec<String>,
    pub disabled_rules: Vec<String>,
    pub extra_project_roots: Vec<String>,
    pub tier: Option<Tier>,
}

struct Stat {
    bytes: u64,
    newest: SystemTime,
}

/// Dung lượng chiếm trên đĩa + thời điểm thay đổi mới nhất. Không đi theo symlink.
fn stat_tree(p: &Path) -> Stat {
    let mut bytes = 0u64;
    let mut newest = SystemTime::UNIX_EPOCH;
    for e in WalkDir::new(p).follow_links(false).into_iter().filter_map(|e| e.ok()) {
        if let Ok(m) = e.metadata() {
            bytes += disk_usage(&m);
            if let Ok(t) = m.modified() {
                if t > newest {
                    newest = t;
                }
            }
        }
    }
    Stat { bytes, newest }
}

#[cfg(unix)]
fn disk_usage(m: &std::fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    if m.is_dir() {
        return 0;
    }
    // blocks = số block 512 byte thực sự cấp phát (đúng với file thưa, APFS clone...)
    let b = m.blocks() * 512;
    if b == 0 { m.len().min(4096) } else { b }
}

#[cfg(not(unix))]
fn disk_usage(m: &std::fs::Metadata) -> u64 {
    if m.is_dir() { 0 } else { m.len() }
}

fn build_globs(pats: &[String]) -> GlobSet {
    let mut b = GlobSetBuilder::new();
    for p in pats {
        if let Ok(g) = Glob::new(p) {
            b.add(g);
        }
    }
    b.build().unwrap_or_else(|_| GlobSet::empty())
}

fn age_days(now: SystemTime, t: SystemTime) -> u64 {
    now.duration_since(t).unwrap_or(Duration::ZERO).as_secs() / 86_400
}

pub struct Scanner<'a> {
    pub guard: &'a Guard,
    pub now: SystemTime,
}

/// Ứng viên thô trước khi áp điều kiện.
struct Raw {
    path: PathBuf,
}

impl<'a> Scanner<'a> {
    pub fn new(guard: &'a Guard) -> Self {
        Self { guard, now: SystemTime::now() }
    }

    pub fn scan(&self, rules: &[Rule], opts: &ScanOptions) -> ScanReport {
        let tier = opts.tier.unwrap_or(Tier::Free);
        let mut reports = Vec::new();
        let mut skipped = Vec::new();
        // Các đường dẫn đã được luật trước nhận — tránh đếm trùng.
        let mut claimed: Vec<(PathBuf, u64)> = Vec::new();

        for rule in rules {
            if !opts.only_rules.is_empty() && !opts.only_rules.contains(&rule.id) {
                continue;
            }
            if opts.disabled_rules.contains(&rule.id) {
                continue;
            }
            let raws = self.collect(rule, opts, &mut skipped);
            let mut items = Vec::new();
            for raw in raws {
                if let Some(c) = self.evaluate(rule, raw, &claimed, &mut skipped) {
                    items.push(c);
                }
            }
            for c in &items {
                claimed.push((c.path.clone(), c.bytes));
            }
            items.sort_by(|a, b| b.bytes.cmp(&a.bytes));
            let bytes = items.iter().map(|c| c.bytes).sum();
            reports.push(RuleReport {
                id: rule.id.clone(),
                name: rule.name.clone(),
                category: rule.category.clone(),
                level: rule.level,
                tier: rule.tier,
                action: rule.action,
                locked: rule.tier > tier,
                bytes,
                items,
            });
        }

        let total_bytes = reports.iter().map(|r| r.bytes).sum();
        let locked_bytes = reports.iter().filter(|r| r.locked).map(|r| r.bytes).sum();
        let safe_bytes = reports
            .iter()
            .filter(|r| !r.locked && r.level == Level::Safe)
            .map(|r| r.bytes)
            .sum();
        ScanReport {
            rules: reports,
            skipped,
            total_bytes,
            available_bytes: total_bytes - locked_bytes,
            locked_bytes,
            safe_bytes,
        }
    }

    fn collect(&self, rule: &Rule, opts: &ScanOptions, skipped: &mut Vec<Skipped>) -> Vec<Raw> {
        let home = self.guard.home();
        let mut roots: Vec<PathBuf> = rule.paths.iter().map(|p| expand(p, home)).collect();
        if rule.kind == Kind::Artifacts {
            roots.extend(opts.extra_project_roots.iter().map(|p| expand(p, home)));
        }
        let exclude = build_globs(&rule.exclude);
        let mut out = Vec::new();
        for root in roots {
            // Gốc không tồn tại hoặc là symlink → bỏ qua
            match std::fs::symlink_metadata(&root) {
                Ok(m) if m.is_dir() => {}
                _ => continue,
            }
            match rule.kind {
                Kind::Contents => match std::fs::read_dir(&root) {
                    Ok(rd) => {
                        for e in rd.filter_map(|e| e.ok()) {
                            let name = e.file_name().to_string_lossy().to_string();
                            if exclude.is_match(&name) || name == ".DS_Store" || name == ".localized" {
                                continue;
                            }
                            out.push(Raw { path: e.path() });
                        }
                    }
                    Err(e) => skipped.push(Skipped {
                        rule_id: rule.id.clone(),
                        path: root.clone(),
                        reason: permission_reason(&e),
                    }),
                },
                Kind::Glob => {
                    let pats = build_globs(&rule.patterns);
                    let mut it = WalkDir::new(&root).min_depth(1).max_depth(rule.depth).follow_links(false).into_iter();
                    while let Some(Ok(e)) = it.next() {
                        let name = e.file_name().to_string_lossy().to_string();
                        if name.starts_with('.') || exclude.is_match(&name) {
                            if e.file_type().is_dir() {
                                it.skip_current_dir();
                            }
                            continue;
                        }
                        // Bỏ qua gói ứng dụng / thư viện ảnh (là thư mục nhưng người dùng xem như 1 file)
                        if e.file_type().is_dir() && is_bundle(&name) {
                            it.skip_current_dir();
                            continue;
                        }
                        if pats.is_match(&name) {
                            // với pattern "*" chỉ lấy file, không lấy thư mục
                            if rule.patterns.iter().any(|p| p == "*") && !e.file_type().is_file() {
                                continue;
                            }
                            out.push(Raw { path: e.path().to_path_buf() });
                            if e.file_type().is_dir() {
                                it.skip_current_dir();
                            }
                        }
                    }
                }
                Kind::Artifacts => self.collect_artifacts(rule, &root, &mut out, skipped),
            }
        }
        out
    }

    fn collect_artifacts(&self, rule: &Rule, root: &Path, out: &mut Vec<Raw>, skipped: &mut Vec<Skipped>) {
        let mut it = WalkDir::new(root).min_depth(1).max_depth(rule.depth).follow_links(false).into_iter();
        while let Some(Ok(e)) = it.next() {
            if !e.file_type().is_dir() {
                continue;
            }
            let name = e.file_name().to_string_lossy().to_string();
            if name == ".git" {
                it.skip_current_dir();
                continue;
            }
            let Some(spec) = rule.artifacts.iter().find(|a| a.dir == name) else { continue };
            it.skip_current_dir(); // không đi sâu vào node_modules...
            let project = match e.path().parent() {
                Some(p) => p.to_path_buf(),
                None => continue,
            };
            if !has_marker(&project, &spec.marker) {
                continue;
            }
            // Thư mục đang được git theo dõi (đã commit) → không phải rác build
            if is_git_tracked(&project, &name) {
                skipped.push(Skipped {
                    rule_id: rule.id.clone(),
                    path: e.path().to_path_buf(),
                    reason: "thư mục được git theo dõi".into(),
                });
                continue;
            }
            // Dự án có hoạt động gần đây → giữ lại
            if let Some(t) = project_activity(&project, &rule.artifacts) {
                if age_days(self.now, t) < rule.min_age_days {
                    skipped.push(Skipped {
                        rule_id: rule.id.clone(),
                        path: e.path().to_path_buf(),
                        reason: format!("dự án có thay đổi trong {} ngày qua", rule.min_age_days),
                    });
                    continue;
                }
            }
            out.push(Raw { path: e.path().to_path_buf() });
        }
    }

    fn evaluate(&self, rule: &Rule, raw: Raw, claimed: &[(PathBuf, u64)], skipped: &mut Vec<Skipped>) -> Option<Candidate> {
        let skip = |skipped: &mut Vec<Skipped>, reason: String| {
            skipped.push(Skipped { rule_id: rule.id.clone(), path: raw.path.clone(), reason });
        };
        // Đã thuộc một luật trước đó
        if claimed.iter().any(|(c, _)| raw.path.starts_with(c)) {
            return None;
        }
        if let Verdict::Deny(reason) = self.guard.check(&raw.path) {
            skip(skipped, reason);
            return None;
        }
        let st = stat_tree(&raw.path);
        let age = age_days(self.now, st.newest);
        if rule.active_guard_minutes > 0 {
            let recent = self.now.duration_since(st.newest).unwrap_or(Duration::ZERO);
            if recent < Duration::from_secs(rule.active_guard_minutes * 60) {
                skip(skipped, "đang được ứng dụng sử dụng".into());
                return None;
            }
        }
        if rule.kind != Kind::Artifacts && age < rule.min_age_days {
            return None; // chưa đủ cũ — không phải lỗi, không cần báo
        }
        // Trừ phần dung lượng đã được luật khác nhận bên trong mục này
        let inner: u64 = claimed.iter().filter(|(c, _)| c.starts_with(&raw.path)).map(|(_, b)| *b).sum();
        let bytes = st.bytes.saturating_sub(inner);
        if rule.min_size_mb > 0 && bytes < rule.min_size_mb * 1024 * 1024 {
            return None;
        }
        if bytes == 0 {
            return None;
        }
        Some(Candidate { rule_id: rule.id.clone(), path: raw.path, bytes, action: rule.action, age_days: age })
    }
}

fn permission_reason(e: &std::io::Error) -> String {
    if e.kind() == std::io::ErrorKind::PermissionDenied {
        "cần cấp quyền Full Disk Access".into()
    } else {
        format!("không đọc được: {e}")
    }
}

fn is_bundle(name: &str) -> bool {
    // .pkg dạng thư mục KHÔNG nằm ở đây vì nó vẫn là file cài đặt cần dọn
    [".app", ".photoslibrary", ".bundle", ".framework", ".musiclibrary", ".fcpbundle", ".imovielibrary", ".sparsebundle"]
        .iter()
        .any(|ext| name.ends_with(ext))
}

fn has_marker(project: &Path, marker: &str) -> bool {
    if !marker.contains('*') {
        return project.join(marker).exists();
    }
    let g = match Glob::new(marker) {
        Ok(g) => g.compile_matcher(),
        Err(_) => return false,
    };
    std::fs::read_dir(project)
        .map(|rd| rd.filter_map(|e| e.ok()).any(|e| g.is_match(e.file_name())))
        .unwrap_or(false)
}

/// Thời điểm thay đổi mới nhất của các file cấp 1 trong dự án (trừ các thư mục build).
fn project_activity(project: &Path, specs: &[crate::rules::ArtifactSpec]) -> Option<SystemTime> {
    let rd = std::fs::read_dir(project).ok()?;
    let mut newest: Option<SystemTime> = None;
    for e in rd.filter_map(|e| e.ok()) {
        let n = e.file_name().to_string_lossy().to_string();
        if specs.iter().any(|s| s.dir == n) {
            continue;
        }
        if let Ok(t) = e.metadata().and_then(|m| m.modified()) {
            newest = Some(newest.map_or(t, |x: SystemTime| x.max(t)));
        }
    }
    newest
}

fn is_git_tracked(project: &Path, dir: &str) -> bool {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(project)
        .args(["ls-files", "--", dir])
        .stderr(std::process::Stdio::null())
        .output();
    matches!(out, Ok(o) if o.status.success() && !o.stdout.is_empty())
}
