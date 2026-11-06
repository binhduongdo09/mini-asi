use asi_clean::clean::{execute, Selection};
use asi_clean::guard::{Guard, Verdict};
use asi_clean::rules::{parse_rules, Tier, LINUX_RULES, MACOS_RULES, WINDOWS_RULES};
use asi_clean::scan::{ScanOptions, Scanner};
use filetime::{set_symlink_file_times, FileTime};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use walkdir::WalkDir;

fn write(p: &Path, kb: usize) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, vec![7u8; kb * 1024]).unwrap();
}

/// Đặt mtime cũ cho toàn bộ cây (cả thư mục), mô phỏng dữ liệu lâu không đụng tới.
fn age(p: &Path, days: u64) {
    let t = FileTime::from_system_time(SystemTime::now() - Duration::from_secs(days * 86_400));
    let mut v: Vec<PathBuf> = WalkDir::new(p).into_iter().filter_map(|e| e.ok()).map(|e| e.into_path()).collect();
    v.reverse(); // con trước, cha sau
    for x in v {
        let _ = set_symlink_file_times(&x, t, t);
    }
}

fn mac_rules() -> Vec<asi_clean::rules::Rule> {
    parse_rules(MACOS_RULES).unwrap()
}

#[test]
fn all_builtin_rule_files_parse() {
    assert!(parse_rules(MACOS_RULES).unwrap().len() >= 15);
    assert!(parse_rules(LINUX_RULES).unwrap().len() >= 5);
    assert!(parse_rules(WINDOWS_RULES).unwrap().len() >= 3);
}

#[test]
fn rule_validation_rejects_deleting_user_files() {
    let bad = r#"
[[rule]]
id = "x"
name = "x"
category = "files"
level = "review"
tier = "free"
kind = "contents"
paths = ["~/Downloads"]
action = "delete"
"#;
    assert!(parse_rules(bad).is_err());
}

#[test]
fn guard_blocks_dangerous_paths() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().to_path_buf();
    let g = Guard::new(home.clone(), &["~/Library/Caches/com.spotify.client".to_string()]);
    let deny = |p: PathBuf| matches!(g.check(&p), Verdict::Deny(_));

    assert!(deny(home.clone()), "home");
    assert!(deny(home.join("Library")), "Library");
    assert!(deny(home.join("Library/Caches")), "Caches gốc");
    assert!(deny(home.join("Downloads")), "Downloads gốc");
    assert!(deny(home.join("Library/Keychains/login.keychain-db")), "keychain");
    assert!(deny(home.join("Library/Mobile Documents/com~apple~CloudDocs/a.pdf")), "iCloud");
    assert!(deny(home.join("Pictures/Photos Library.photoslibrary/originals/1.heic")), "Photos");
    assert!(deny(home.join(".ssh/id_ed25519")), "ssh");
    assert!(deny(PathBuf::from("/etc/passwd")), "ngoài home");
    assert!(deny(home.join("Library/Caches/../Keychains")), "..");
    assert!(deny(home.join("Projects/app/.git/objects")), ".git");
    assert!(deny(home.join("Library/Caches/com.spotify.client")), "whitelist");
    assert!(deny(home.join("Library/Caches/com.spotify.client/data/x")), "whitelist con");

    assert_eq!(g.check(&home.join("Library/Caches/com.foo")), Verdict::Allow);
    assert_eq!(g.check(&home.join("Downloads/setup.dmg")), Verdict::Allow);
}

#[cfg(unix)]
#[test]
fn guard_blocks_symlinked_parent() {
    let tmp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let home = tmp.path().to_path_buf();
    fs::create_dir_all(home.join("Library/Caches")).unwrap();
    std::os::unix::fs::symlink(outside.path(), home.join("Library/Caches/evil")).unwrap();
    let g = Guard::new(home.clone(), &[]);
    assert!(matches!(g.check(&home.join("Library/Caches/evil/victim")), Verdict::Deny(_)));
}

struct Fixture {
    _tmp: tempfile::TempDir,
    home: PathBuf,
}

fn mac_fixture() -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let h = tmp.path().to_path_buf();
    // cache chung, cũ
    write(&h.join("Library/Caches/com.foo.app/blob1"), 300);
    age(&h.join("Library/Caches/com.foo.app"), 5);
    // cache Chrome — phải được luật trình duyệt nhận, không bị đếm trùng ở luật chung
    write(&h.join("Library/Caches/Google/Chrome/Default/Cache/data_1"), 500);
    age(&h.join("Library/Caches/Google"), 2);
    // cache đang được dùng (vừa ghi) → phải bỏ qua
    write(&h.join("Library/Caches/com.active.app/live"), 200);
    // cache iCloud → nằm trong exclude
    write(&h.join("Library/Caches/CloudKit/db"), 100);
    age(&h.join("Library/Caches/CloudKit"), 10);
    // log cũ và log mới
    write(&h.join("Library/Logs/OldApp/old.log"), 50);
    age(&h.join("Library/Logs/OldApp"), 30);
    write(&h.join("Library/Logs/NewApp/new.log"), 50);
    // file cài đặt cũ (Pro)
    write(&h.join("Downloads/Photoshop.dmg"), 800);
    age(&h.join("Downloads/Photoshop.dmg"), 10);
    // tài liệu quan trọng — không luật nào được đụng
    write(&h.join("Documents/hopdong.pdf"), 100);
    // dự án cũ có node_modules (Pro)
    write(&h.join("Projects/oldsite/package.json"), 1);
    write(&h.join("Projects/oldsite/node_modules/react/index.js"), 400);
    age(&h.join("Projects/oldsite"), 30);
    // dự án đang làm → giữ
    write(&h.join("Projects/newsite/package.json"), 1);
    write(&h.join("Projects/newsite/node_modules/vue/index.js"), 400);
    age(&h.join("Projects/newsite/node_modules"), 30);
    Fixture { _tmp: tmp, home: h }
}

fn opts(tier: Tier) -> ScanOptions {
    ScanOptions { tier: Some(tier), ..Default::default() }
}

#[test]
fn scan_finds_expected_items_without_double_counting() {
    let f = mac_fixture();
    let g = Guard::new(f.home.clone(), &[]);
    let rep = Scanner::new(&g).scan(&mac_rules(), &opts(Tier::Free));
    let rule = |id: &str| rep.rules.iter().find(|r| r.id == id).unwrap();

    let chrome = rule("browser-chrome");
    assert_eq!(chrome.items.len(), 1);
    assert!(chrome.bytes >= 500 * 1024);

    let caches = rule("system-user-caches");
    let names: Vec<String> = caches.items.iter().map(|c| c.path.file_name().unwrap().to_string_lossy().into()).collect();
    assert!(names.contains(&"com.foo.app".to_string()));
    assert!(!names.contains(&"Google".to_string()), "Google chỉ chứa Chrome đã được nhận → 0 byte còn lại");
    assert!(!names.contains(&"CloudKit".to_string()), "CloudKit bị exclude");
    assert!(!names.contains(&"com.active.app".to_string()), "đang dùng → bỏ qua");
    assert!(rep.skipped.iter().any(|s| s.path.ends_with("com.active.app")));

    let logs = rule("system-logs");
    assert_eq!(logs.items.len(), 1);
    assert!(logs.items[0].path.ends_with("OldApp"));

    let inst = rule("files-installers");
    assert!(inst.locked, "Pro bị khoá ở bản Free");
    assert_eq!(inst.items.len(), 1);

    let art = rule("dev-project-artifacts");
    assert_eq!(art.items.len(), 1);
    assert!(art.items[0].path.ends_with("oldsite/node_modules"));
    assert!(rep.skipped.iter().any(|s| s.path.ends_with("newsite/node_modules")));

    assert!(rep.locked_bytes > 0);
    assert!(rep.safe_bytes > 0);
    // Không mục nào đụng Documents
    for r in &rep.rules {
        for c in &r.items {
            assert!(!c.path.starts_with(f.home.join("Documents")), "{:?}", c.path);
        }
    }
}

#[test]
fn dry_run_changes_nothing_and_free_cannot_run_pro() {
    let f = mac_fixture();
    let g = Guard::new(f.home.clone(), &[]);
    let rep = Scanner::new(&g).scan(&mac_rules(), &opts(Tier::Free));
    let all: Vec<String> = rep.rules.iter().map(|r| r.id.clone()).collect();
    let sel = Selection { rules: all.clone(), paths: vec![] };

    let dry = execute(&rep, &sel, &g, None, true);
    assert!(dry.items_done > 0);
    assert!(f.home.join("Library/Caches/com.foo.app/blob1").exists(), "dry-run không được xoá");

    let res = execute(&rep, &sel, &g, None, false);
    assert!(!f.home.join("Library/Caches/com.foo.app").exists());
    assert!(!f.home.join("Library/Caches/Google/Chrome/Default").exists());
    assert!(f.home.join("Library/Caches/Google/Chrome").exists(), "thư mục gốc của luật được giữ");
    assert!(f.home.join("Library/Caches/com.active.app/live").exists());
    assert!(f.home.join("Library/Caches/CloudKit/db").exists());
    assert!(f.home.join("Library/Logs/NewApp/new.log").exists());
    assert!(f.home.join("Downloads/Photoshop.dmg").exists(), "Pro bị khoá → không xoá");
    assert!(f.home.join("Projects/oldsite/node_modules").exists(), "Pro bị khoá → không xoá");
    assert!(f.home.join("Documents/hopdong.pdf").exists());
    assert!(res.locked_rules.contains(&"files-installers".to_string()));
    assert!(res.deleted_bytes > 0);
}

#[test]
fn pro_cleans_artifacts_and_trashes_user_files() {
    let f = mac_fixture();
    // Thùng rác freedesktop trỏ vào thư mục tạm để không đụng máy thật
    let trash_home = tempfile::tempdir().unwrap();
    std::env::set_var("XDG_DATA_HOME", trash_home.path());
    let g = Guard::new(f.home.clone(), &[]);
    let rep = Scanner::new(&g).scan(&mac_rules(), &opts(Tier::Pro));
    let sel = Selection { rules: vec!["files-installers".into(), "dev-project-artifacts".into()], paths: vec![] };
    let res = execute(&rep, &sel, &g, None, false);
    assert!(res.failures.is_empty(), "{:?}", res.failures);
    assert!(!f.home.join("Projects/oldsite/node_modules").exists());
    assert!(f.home.join("Projects/oldsite/package.json").exists(), "mã nguồn dự án còn nguyên");
    assert!(f.home.join("Projects/newsite/node_modules").exists(), "dự án đang làm còn nguyên");
    assert!(!f.home.join("Downloads/Photoshop.dmg").exists());
    assert!(res.trashed_bytes > 0, "dmg phải vào Thùng rác, không xoá hẳn");
}

#[cfg(unix)]
#[test]
fn symlink_candidate_removes_link_only() {
    let tmp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let h = tmp.path().to_path_buf();
    write(&outside.path().join("precious.txt"), 10);
    fs::create_dir_all(h.join("Library/Caches")).unwrap();
    std::os::unix::fs::symlink(outside.path(), h.join("Library/Caches/link")).unwrap();
    age(&h.join("Library/Caches/link"), 5);
    let g = Guard::new(h.clone(), &[]);
    let rep = Scanner::new(&g).scan(&mac_rules(), &opts(Tier::Free));
    let sel = Selection { rules: vec!["system-user-caches".into()], paths: vec![] };
    execute(&rep, &sel, &g, None, false);
    assert!(outside.path().join("precious.txt").exists(), "không bao giờ xoá xuyên qua symlink");
}

#[test]
fn git_tracked_build_dir_is_kept() {
    if std::process::Command::new("git").arg("--version").output().is_err() {
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let h = tmp.path().to_path_buf();
    let p = h.join("Projects/lib");
    write(&p.join("package.json"), 1);
    write(&p.join("dist/bundle.js"), 50);
    let git = |args: &[&str]| {
        std::process::Command::new("git").arg("-C").arg(&p).args(args).output().unwrap();
    };
    git(&["init", "-q"]);
    git(&["add", "."]);
    git(&["-c", "user.email=a@b", "-c", "user.name=a", "commit", "-qm", "x"]);
    age(&p, 30);
    let g = Guard::new(h.clone(), &[]);
    let rep = Scanner::new(&g).scan(&mac_rules(), &opts(Tier::Pro));
    let art = rep.rules.iter().find(|r| r.id == "dev-project-artifacts").unwrap();
    assert!(art.items.is_empty(), "dist đã commit vào git → không phải rác");
}

#[test]
fn user_whitelist_is_respected() {
    let f = mac_fixture();
    let g = Guard::new(f.home.clone(), &["~/Library/Caches/com.foo.app".into()]);
    let rep = Scanner::new(&g).scan(&mac_rules(), &opts(Tier::Free));
    let caches = rep.rules.iter().find(|r| r.id == "system-user-caches").unwrap();
    assert!(caches.items.iter().all(|c| !c.path.ends_with("com.foo.app")));
}
