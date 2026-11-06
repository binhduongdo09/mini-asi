//! Tự động kiểm tra và cập nhật Mini-Asi trực tiếp từ GitHub Releases.
//!
//! Luồng hoạt động:
//! 1. Kiểm tra bản phát hành mới nhất trên GitHub (`https://api.github.com/repos/<repo>/releases`).
//!    Hỗ trợ cả repo Public lẫn repo Private (qua `gh auth token` hoặc token cài đặt).
//! 2. So sánh phiên bản ngữ nghĩa (`semver`) với `CARGO_PKG_VERSION` đang chạy.
//! 3. Khi người dùng bấm "Cập nhật ngay":
//!    - Tải bộ cài đặt phù hợp với hệ điều hành (`.dmg` / `.app.tar.gz` trên macOS, `.msi`/`.exe` trên Windows, `.AppImage`/`.deb` trên Linux).
//!    - Trên macOS: tự gắn (mount) file `.dmg` ở chế độ ẩn, chép `Mini-Asi.app` mới sang bản tạm,
//!      gỡ cờ cách ly (`xattr -cr`), tráo nguyên tử vào `/Applications/Mini-Asi.app`, tháo `.dmg` và tự khởi động lại app.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

pub const DEFAULT_REPO: &str = "binhduongdo09/mini-asi";
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Asset {
    pub id: u64,
    pub name: String,
    pub size: u64,
    pub browser_download_url: String,
    pub api_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReleaseInfo {
    /// Phiên bản đang chạy (vd. "0.10.0")
    pub current_version: String,
    /// Phiên bản mới nhất trên GitHub (vd. "0.11.0")
    pub latest_version: String,
    pub tag: String,
    pub title: String,
    pub notes: String,
    pub published_at: String,
    pub html_url: String,
    pub has_update: bool,
    pub asset: Option<Asset>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct UpdateProgress {
    /// idle | checking | available | up_to_date | downloading | installing | restarting | error
    pub phase: String,
    pub percent: u8,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub message: String,
    pub error: Option<String>,
    pub release: Option<ReleaseInfo>,
}

/// Phân tích chuỗi phiên bản dạng "v0.10.2", "0.10.2", hoặc tìm trong tên file "Mini-Asi_0.11.0_universal.dmg"
pub fn parse_semver(s: &str) -> Option<(u32, u32, u32)> {
    let clean = s.trim().trim_start_matches(['v', 'V']);
    for part in clean.split(|c: char| !c.is_ascii_digit() && c != '.') {
        let mut nums = part.split('.');
        if let (Some(ma), Some(mi), Some(pa)) = (
            nums.next().and_then(|x| x.parse().ok()),
            nums.next().and_then(|x| x.parse().ok()),
            nums.next().and_then(|x| x.parse().ok()),
        ) {
            if nums.next().is_none() {
                return Some((ma, mi, pa));
            }
        }
    }
    None
}

/// Kiểm tra xem phiên bản `remote` có mới hơn `current` không
pub fn is_newer(current: &str, remote: &str) -> bool {
    match (parse_semver(current), parse_semver(remote)) {
        (Some(cur), Some(rem)) => rem > cur,
        _ => false,
    }
}

/// Chọn file cài đặt phù hợp nhất cho hệ điều hành và kiến trúc máy
pub fn pick_asset<'a>(assets: &'a [Asset], os: &str, arch: &str) -> Option<&'a Asset> {
    let lower: Vec<(&Asset, String)> = assets.iter().map(|a| (a, a.name.to_lowercase())).collect();
    match os {
        "macos" => {
            // Ưu tiên bản universal .dmg, sau đó tới đúng kiến trúc (aarch64/arm64 hoặc x86_64/x64), rồi tới .dmg bất kỳ
            let is_dmg = |n: &str| n.ends_with(".dmg");
            let is_tar = |n: &str| n.ends_with(".app.tar.gz");
            let arch_keys: &[&str] = if arch == "aarch64" { &["universal", "aarch64", "arm64"] } else { &["universal", "x86_64", "x64"] };
            for k in arch_keys {
                if let Some((a, _)) = lower.iter().find(|(_, n)| is_dmg(n) && n.contains(k)) {
                    return Some(a);
                }
            }
            if let Some((a, _)) = lower.iter().find(|(_, n)| is_dmg(n)) {
                return Some(a);
            }
            lower.iter().find(|(_, n)| is_tar(n)).map(|(a, _)| *a)
        }
        "windows" => {
            lower
                .iter()
                .find(|(_, n)| n.ends_with(".msi") || n.ends_with("-setup.exe") || n.ends_with(".exe"))
                .map(|(a, _)| *a)
        }
        "linux" => {
            lower
                .iter()
                .find(|(_, n)| n.ends_with(".appimage"))
                .or_else(|| lower.iter().find(|(_, n)| n.ends_with(".deb")))
                .map(|(a, _)| *a)
        }
        _ => None,
    }
}

/// Đọc thông tin bản phát hành từ JSON của GitHub Releases API
pub fn parse_release(v: &Value, current_ver: &str, os: &str, arch: &str) -> Option<ReleaseInfo> {
    if v.get("draft").and_then(|d| d.as_bool()) == Some(true) {
        return None;
    }
    let tag = v.get("tag_name").and_then(|x| x.as_str())?.to_string();
    let title = v.get("name").and_then(|x| x.as_str()).filter(|s| !s.is_empty()).unwrap_or(&tag).to_string();
    let notes = v.get("body").and_then(|x| x.as_str()).unwrap_or("").trim().to_string();
    let published_at = v.get("published_at").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let html_url = v.get("html_url").and_then(|x| x.as_str()).unwrap_or("").to_string();

    let assets: Vec<Asset> = v
        .get("assets")
        .and_then(|x| x.as_array())
        .into_iter()
        .flatten()
        .filter_map(|a| {
            Some(Asset {
                id: a.get("id")?.as_u64()?,
                name: a.get("name")?.as_str()?.to_string(),
                size: a.get("size").and_then(|s| s.as_u64()).unwrap_or(0),
                browser_download_url: a.get("browser_download_url")?.as_str()?.to_string(),
                api_url: a.get("url").and_then(|u| u.as_str()).unwrap_or("").to_string(),
            })
        })
        .collect();

    let asset = pick_asset(&assets, os, arch).cloned();
    // Lấy phiên bản từ tag (vd. "v0.11.0"), nếu tag là "build-12" thì lấy từ tên file cài đặt hoặc tiêu đề
    let detected_ver = parse_semver(&tag)
        .or_else(|| asset.as_ref().and_then(|a| parse_semver(&a.name)))
        .or_else(|| parse_semver(&title))
        .map(|(ma, mi, pa)| format!("{ma}.{mi}.{pa}"))
        .unwrap_or_else(|| tag.trim_start_matches('v').to_string());

    let has_update = is_newer(current_ver, &detected_ver);

    Some(ReleaseInfo {
        current_version: current_ver.to_string(),
        latest_version: detected_ver,
        tag,
        title,
        notes,
        published_at,
        html_url,
        has_update,
        asset,
    })
}

/// Tìm token GitHub (nếu có) để hỗ trợ cả trường hợp repo đang để Private
pub fn github_token(data_dir: &Path) -> Option<String> {
    if let Some(t) = crate::threads::secret::get("github_update_token", data_dir).filter(|s| !s.trim().is_empty()) {
        return Some(t.trim().to_string());
    }
    for env_key in ["MINI_ASI_GITHUB_TOKEN", "GITHUB_TOKEN", "GH_TOKEN"] {
        if let Ok(t) = std::env::var(env_key) {
            if !t.trim().is_empty() {
                return Some(t.trim().to_string());
            }
        }
    }
    for bin in ["/opt/homebrew/bin/gh", "/usr/local/bin/gh", "gh"] {
        if let Ok(out) = Command::new(bin).args(["auth", "token"]).output() {
            if out.status.success() {
                let t = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !t.is_empty() {
                    return Some(t);
                }
            }
        }
    }
    None
}

fn api_get(url: &str, token: Option<&str>) -> Result<Value, String> {
    let mut req = ureq::get(url)
        .timeout(std::time::Duration::from_secs(15))
        .set("User-Agent", "Mini-Asi-Updater")
        .set("Accept", "application/vnd.github+json");
    if let Some(t) = token {
        req = req.set("Authorization", &format!("Bearer {t}"));
    }
    match req.call() {
        Ok(resp) => resp.into_json::<Value>().map_err(|e| format!("Dữ liệu GitHub không hợp lệ: {e}")),
        Err(ureq::Error::Status(404, _)) => Err("404".into()),
        Err(ureq::Error::Status(code, _)) => Err(format!("GitHub trả về mã lỗi {code}")),
        Err(ureq::Error::Transport(t)) => Err(format!("Không kết nối được GitHub: {t}")),
    }
}

/// Kiểm tra bản phát hành mới nhất trên GitHub
pub fn check_latest(repo: &str, data_dir: &Path) -> Result<ReleaseInfo, String> {
    let repo = if repo.trim().is_empty() { DEFAULT_REPO } else { repo.trim() };
    let token = github_token(data_dir);
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    let latest_url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let val = match api_get(&latest_url, token.as_deref()) {
        Ok(v) => v,
        Err(e) if e == "404" => {
            // Có thể chưa có bản Release chính thức (hoặc chỉ có danh sách releases)
            let list_url = format!("https://api.github.com/repos/{repo}/releases?per_page=10");
            let list = api_get(&list_url, token.as_deref()).map_err(|e2| {
                if e2 == "404" {
                    format!("Chưa có bản phát hành nào trên GitHub ({repo}) hoặc repo đang để Private")
                } else {
                    e2
                }
            })?;
            list.as_array()
                .and_then(|arr| arr.iter().find(|r| r.get("draft").and_then(|d| d.as_bool()) != Some(true)).cloned())
                .ok_or_else(|| format!("Chưa có bản phát hành nào trong {repo} — hãy chạy workflow build trên GitHub trước"))?
        }
        Err(e) => return Err(e),
    };

    parse_release(&val, CURRENT_VERSION, os, arch).ok_or_else(|| "Không đọc được thông tin phiên bản từ GitHub".into())
}

/// Tải file cài đặt về `dest` kèm báo tiến độ
pub fn download_asset(asset: &Asset, token: Option<&str>, dest: &Path, on_progress: &dyn Fn(u64, u64)) -> Result<(), String> {
    if let Some(p) = dest.parent() {
        std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }
    let tmp = dest.with_extension("part");
    // Nếu có token (repo Private) và có api_url thì tải qua API với Accept: application/octet-stream
    let resp = if let (Some(t), true) = (token, !asset.api_url.is_empty()) {
        ureq::get(&asset.api_url)
            .timeout(std::time::Duration::from_secs(300))
            .set("User-Agent", "Mini-Asi-Updater")
            .set("Accept", "application/octet-stream")
            .set("Authorization", &format!("Bearer {t}"))
            .call()
            .or_else(|_| {
                ureq::get(&asset.browser_download_url)
                    .timeout(std::time::Duration::from_secs(300))
                    .set("User-Agent", "Mini-Asi-Updater")
                    .call()
            })
    } else {
        ureq::get(&asset.browser_download_url)
            .timeout(std::time::Duration::from_secs(300))
            .set("User-Agent", "Mini-Asi-Updater")
            .call()
    }
    .map_err(|e| format!("Không tải được bản cập nhật: {e}"))?;

    let total = resp
        .header("Content-Length")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(asset.size);
    let mut reader = resp.into_reader();
    let mut file = std::fs::File::create(&tmp).map_err(|e| format!("Không tạo được file tạm: {e}"))?;
    let mut buf = vec![0u8; 64 * 1024];
    let mut got = 0u64;
    let mut last_pct = 255u8;

    loop {
        let n = reader.read(&mut buf).map_err(|e| format!("Lỗi mạng khi đang tải: {e}"))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|e| format!("Không ghi được file cập nhật: {e}"))?;
        got += n as u64;
        let pct = if total > 0 { ((got * 100) / total).min(100) as u8 } else { 50 };
        if pct != last_pct {
            last_pct = pct;
            on_progress(got, total);
        }
    }
    file.flush().map_err(|e| e.to_string())?;
    drop(file);
    std::fs::rename(&tmp, dest).map_err(|e| format!("Không lưu được gói cập nhật: {e}"))
}

/// Tìm đường dẫn `.app` đang chạy trên macOS (hoặc `/Applications/Mini-Asi.app`)
#[cfg(target_os = "macos")]
pub fn macos_target_app() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        // Ví dụ: /Applications/Mini-Asi.app/Contents/MacOS/mini-asi
        for anc in exe.ancestors() {
            if anc.extension().is_some_and(|e| e == "app") {
                return anc.to_path_buf();
            }
        }
    }
    PathBuf::from("/Applications/Mini-Asi.app")
}

/// Cài đặt trực tiếp từ file `.dmg` trên macOS mà không cần người dùng mở Finder kéo thả
#[cfg(target_os = "macos")]
pub fn install_dmg_macos(dmg_path: &Path, target_app: &Path) -> Result<(), String> {
    let work_dir = dmg_path.parent().unwrap_or_else(|| Path::new("/tmp"));
    let mnt = work_dir.join("mnt-asi-dmg");
    let _ = Command::new("hdiutil").args(["detach", &mnt.to_string_lossy(), "-force", "-quiet"]).status();
    let _ = std::fs::create_dir_all(&mnt);

    let attach = Command::new("hdiutil")
        .args(["attach", "-nobrowse", "-readonly", "-noverify", "-noautoopen", "-mountpoint"])
        .arg(&mnt)
        .arg(dmg_path)
        .output()
        .map_err(|e| format!("Không mở được file .dmg: {e}"))?;
    if !attach.status.success() {
        return Err(format!("Không gắn được file .dmg: {}", String::from_utf8_lossy(&attach.stderr).trim()));
    }

    // Tìm *.app bên trong đĩa gắn
    let app_in_dmg = std::fs::read_dir(&mnt)
        .ok()
        .and_then(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path())
                .find(|p| p.extension().is_some_and(|ext| ext == "app"))
        });

    let Some(src_app) = app_in_dmg else {
        let _ = Command::new("hdiutil").args(["detach", &mnt.to_string_lossy(), "-force", "-quiet"]).status();
        return Err("Không tìm thấy Mini-Asi.app bên trong file .dmg".into());
    };

    let parent = target_app.parent().unwrap_or_else(|| Path::new("/Applications"));
    let staging = parent.join(".Mini-Asi-new.app");
    let backup = parent.join(".Mini-Asi-old.app");
    let _ = std::fs::remove_dir_all(&staging);
    let _ = std::fs::remove_dir_all(&backup);

    let copy = Command::new("ditto").arg(&src_app).arg(&staging).output();
    let _ = Command::new("hdiutil").args(["detach", &mnt.to_string_lossy(), "-force", "-quiet"]).status();
    let _ = std::fs::remove_dir(&mnt);

    match copy {
        Ok(o) if o.status.success() => {}
        Ok(o) => return Err(format!("Không chép được bản mới vào {}: {}", parent.display(), String::from_utf8_lossy(&o.stderr).trim())),
        Err(e) => return Err(format!("Lỗi khi chép bản mới: {e}")),
    }

    // Gỡ cờ cách ly của macOS để khi khởi động lại không hỏi "Chuột phải → Open"
    let _ = Command::new("xattr").args(["-cr"]).arg(&staging).status();

    // Tráo nguyên tử: bản cũ -> .Mini-Asi-old.app, bản mới -> Mini-Asi.app
    if target_app.exists() {
        std::fs::rename(target_app, &backup).map_err(|e| format!("Không thay thế được ứng dụng tại {}: {e}", target_app.display()))?;
    }
    if let Err(e) = std::fs::rename(&staging, target_app) {
        let _ = std::fs::rename(&backup, target_app);
        return Err(format!("Không kích hoạt được bản mới: {e}"));
    }
    let _ = std::fs::remove_dir_all(&backup);
    Ok(())
}

/// Khởi chạy lại ứng dụng mới sau 1 giây
pub fn relaunch_after_exit() {
    #[cfg(target_os = "macos")]
    {
        let app = macos_target_app();
        let _ = Command::new("sh")
            .args(["-c", "sleep 1; open -n \"$0\"", &app.to_string_lossy()])
            .spawn();
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(exe) = std::env::var("APPIMAGE").or_else(|_| std::env::current_exe().map(|p| p.to_string_lossy().to_string())) {
            let _ = Command::new("sh").args(["-c", "sleep 1; \"$0\" &", &exe]).spawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_parsing_and_comparison() {
        assert_eq!(parse_semver("0.10.0"), Some((0, 10, 0)));
        assert_eq!(parse_semver("v0.11.2"), Some((0, 11, 2)));
        assert_eq!(parse_semver("Mini-Asi_0.12.0_universal.dmg"), Some((0, 12, 0)));
        assert!(is_newer("0.10.0", "0.10.1"));
        assert!(is_newer("0.10.0", "v0.11.0"));
        assert!(is_newer("0.10.9", "1.0.0"));
        assert!(!is_newer("0.10.0", "0.10.0"));
        assert!(!is_newer("0.10.0", "0.9.9"));
    }

    #[test]
    fn asset_selection_per_os() {
        let assets = vec![
            Asset {
                id: 1,
                name: "Mini-Asi_0.11.0_universal.dmg".into(),
                size: 100,
                browser_download_url: "https://example.com/mac.dmg".into(),
                api_url: String::new(),
            },
            Asset {
                id: 2,
                name: "Mini-Asi_0.11.0_x64-setup.exe".into(),
                size: 100,
                browser_download_url: "https://example.com/win.exe".into(),
                api_url: String::new(),
            },
            Asset {
                id: 3,
                name: "Mini-Asi_0.11.0_amd64.AppImage".into(),
                size: 100,
                browser_download_url: "https://example.com/lin.AppImage".into(),
                api_url: String::new(),
            },
        ];
        assert_eq!(pick_asset(&assets, "macos", "aarch64").unwrap().id, 1);
        assert_eq!(pick_asset(&assets, "windows", "x86_64").unwrap().id, 2);
        assert_eq!(pick_asset(&assets, "linux", "x86_64").unwrap().id, 3);
    }

    #[test]
    fn release_json_parsing_handles_build_tags() {
        let j = serde_json::json!({
            "tag_name": "build-42",
            "name": "Mini-Asi 42",
            "body": "- Thêm tự động cập nhật\n- Sửa lỗi nhỏ",
            "draft": false,
            "published_at": "2026-11-06T15:00:00Z",
            "html_url": "https://github.com/binhduongdo09/mini-asi/releases/tag/build-42",
            "assets": [
                {
                    "id": 99,
                    "name": "Mini-Asi_0.11.0_universal.dmg",
                    "size": 8500000,
                    "browser_download_url": "https://github.com/binhduongdo09/mini-asi/releases/download/build-42/Mini-Asi_0.11.0_universal.dmg",
                    "url": "https://api.github.com/repos/binhduongdo09/mini-asi/releases/assets/99"
                }
            ]
        });
        let rel = parse_release(&j, "0.10.0", "macos", "aarch64").unwrap();
        assert_eq!(rel.latest_version, "0.11.0");
        assert!(rel.has_update);
        assert_eq!(rel.asset.unwrap().id, 99);

        let same = parse_release(&j, "0.11.0", "macos", "aarch64").unwrap();
        assert!(!same.has_update);
    }
}
