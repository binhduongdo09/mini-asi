//! Cầu nối Apple Intelligence (FoundationModels, macOS 26+, Apple Silicon) — chạy hoàn toàn trên máy.
//!
//! Trợ lý nhỏ viết bằng Swift (`resources/asi_ai.swift`) nhận chỉ dẫn + câu hỏi, trả về một dòng JSON
//! `{reply, action, argument}` nhờ "guided generation": mô hình vừa trả lời vừa chọn việc cho app làm.
//!
//! Lấy file chạy theo thứ tự:
//! 1. Bản biên dịch sẵn lúc build (build.rs, cần Xcode 26 trên máy build) → máy người dùng không cần công cụ gì.
//! 2. Máy người dùng có Command Line Tools → tự biên dịch 1 lần. Kiểm tra bằng `xcode-select -p` trước,
//!    để KHÔNG bật hộp thoại "Cài công cụ dành cho nhà phát triển" trên máy khách.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[cfg(target_os = "macos")]
const PREBUILT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/asi_ai_bin"));
#[cfg(target_os = "macos")]
const SWIFT_SRC: &str = include_str!("../resources/asi_ai.swift");
const TIMEOUT: Duration = Duration::from_secs(45);

/// Các hành động mô hình được phép chọn (liệt kê trong mô tả trường `action` của asi_ai.swift)
pub const ACTIONS: &[&str] = &["none", "add_task", "add_note", "start_focus", "show_today", "show_overdue", "check_mac", "clean_junk", "open_tab"];

#[derive(Debug, Clone, Serialize)]
pub struct AppleAiStatus {
    pub available: bool,
    pub engine: String,
    pub detail: String,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct AiTurn {
    #[serde(default)]
    pub reply: String,
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub argument: String,
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn fnv(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{:08x}", h as u32)
}

/// Đường dẫn file chạy trợ lý Apple Intelligence (biên dịch/giải nén nếu cần). `None` = máy không hỗ trợ.
pub fn helper(data_dir: &Path) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let _ = std::fs::create_dir_all(data_dir);
        for old in ["apple_ai_v1_bin", "apple_ai_v1.swift"] {
            let _ = std::fs::remove_file(data_dir.join(old)); // bản cũ (≤ 0.12)
        }
        if !cfg!(target_arch = "aarch64") && !rosetta_host_is_arm() {
            return None; // Apple Intelligence chỉ có trên chip Apple
        }
        if !PREBUILT.is_empty() {
            let p = data_dir.join(format!("asi_ai_{}", fnv(PREBUILT)));
            if !p.exists() {
                let tmp = p.with_extension("part");
                std::fs::write(&tmp, PREBUILT).ok()?;
                set_exec(&tmp);
                std::fs::rename(&tmp, &p).ok()?;
            }
            return Some(p);
        }
        let p = data_dir.join(format!("asi_ai_src_{}", fnv(SWIFT_SRC.as_bytes())));
        if p.exists() {
            return Some(p);
        }
        let failed = p.with_extension("failed");
        let recent_fail = std::fs::metadata(&failed).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|e| e < Duration::from_secs(7 * 86400));
        if recent_fail || !has_dev_tools() {
            return None;
        }
        let src = data_dir.join("asi_ai.swift");
        std::fs::write(&src, SWIFT_SRC).ok()?;
        // ưu tiên bản có cấu trúc (chọn được hành động); không được thì bản chỉ trả lời chữ
        let compile = |structured: bool| {
            let mut c = Command::new("/usr/bin/xcrun");
            c.args(["swiftc", "-parse-as-library", "-O", "-target", "arm64-apple-macos26.0"]);
            if structured {
                c.args(["-D", "ASI_STRUCTURED"]);
            }
            c.arg(&src).arg("-o").arg(&p).stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false)
        };
        let ok = compile(true) || compile(false);
        let _ = std::fs::remove_file(&src);
        if ok && p.exists() {
            return Some(p);
        }
        let _ = std::fs::write(&failed, "");
        None
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = data_dir;
        None
    }
}

#[cfg(target_os = "macos")]
fn set_exec(p: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755));
}

/// App bản Intel chạy qua Rosetta trên chip Apple vẫn gọi được trợ lý arm64
#[cfg(target_os = "macos")]
fn rosetta_host_is_arm() -> bool {
    Command::new("/usr/sbin/sysctl").args(["-n", "hw.optional.arm64"]).output().map(|o| String::from_utf8_lossy(&o.stdout).trim() == "1").unwrap_or(false)
}

/// Có Xcode / Command Line Tools thật (không phải lối tắt sẽ bật hộp thoại cài đặt)
#[cfg(target_os = "macos")]
fn has_dev_tools() -> bool {
    Command::new("/usr/bin/xcode-select").arg("-p").stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false)
}

pub fn status(data_dir: &Path) -> AppleAiStatus {
    let none = |detail: &str| AppleAiStatus { available: false, engine: "Apple Intelligence".into(), detail: detail.into() };
    let Some(bin) = helper(data_dir) else {
        return none(if cfg!(target_os = "macos") { "Cần macOS 26 trở lên trên máy chip Apple, đã bật Apple Intelligence" } else { "Chỉ có trên macOS" });
    };
    match Command::new(bin).arg("--check").output() {
        Ok(out) => {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if s == "READY" {
                AppleAiStatus { available: true, engine: "Apple Intelligence".into(), detail: "Sẵn sàng · chạy ngay trên máy, không gửi dữ liệu đi".into() }
            } else {
                none(&explain_unavailable(&s))
            }
        }
        Err(_) => none("Máy này chưa chạy được Apple Intelligence (cần macOS 26)"),
    }
}

fn explain_unavailable(s: &str) -> String {
    if s.contains("appleIntelligenceNotEnabled") {
        "Chưa bật — vào Cài đặt hệ thống › Apple Intelligence & Siri".into()
    } else if s.contains("modelNotReady") {
        "macOS đang tải mô hình Apple Intelligence — thử lại sau ít phút".into()
    } else if s.contains("deviceNotEligible") {
        "Máy này không hỗ trợ Apple Intelligence".into()
    } else {
        "Chưa sẵn sàng — kiểm tra Cài đặt hệ thống › Apple Intelligence & Siri".into()
    }
}

/// Hỏi Apple Intelligence. `system` = chỉ dẫn (vai trò, quy tắc, ngữ cảnh máy), `prompt` = lịch sử + câu hỏi.
/// `allow_actions = false` → chỉ trả lời chữ (ASI_SIMPLE=1), dùng khi mô hình chọn nhầm hành động.
pub fn ask(data_dir: &Path, system: &str, prompt: &str, allow_actions: bool) -> Result<AiTurn, String> {
    let bin = helper(data_dir).ok_or("Máy chưa hỗ trợ Apple Intelligence")?;
    let mut child = Command::new(bin)
        .env("ASI_SYSTEM", system)
        .env("ASI_PROMPT", prompt)
        .env("ASI_SIMPLE", if allow_actions { "0" } else { "1" })
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Không gọi được Apple Intelligence: {e}"))?;
    let mut stdout = child.stdout.take().ok_or("Không đọc được kết quả")?;
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = std::io::Read::read_to_string(&mut stdout, &mut s);
        s
    });
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if start.elapsed() > TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Apple Intelligence trả lời quá lâu".into());
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(40)),
            Err(e) => return Err(e.to_string()),
        }
    }
    parse_output(&reader.join().unwrap_or_default())
}

/// Đọc dòng JSON cuối cùng trợ lý in ra; chuẩn hoá hành động lạ thành "none"
pub fn parse_output(out: &str) -> Result<AiTurn, String> {
    let line = out.lines().rev().find(|l| l.trim_start().starts_with('{')).ok_or("Apple Intelligence không trả về kết quả")?;
    let v: serde_json::Value = serde_json::from_str(line.trim()).map_err(|_| "Kết quả Apple Intelligence không đọc được")?;
    if let Some(e) = v.get("error").and_then(|e| e.as_str()) {
        return Err(e.to_string());
    }
    let mut t: AiTurn = serde_json::from_value(v).map_err(|e| e.to_string())?;
    t.reply = t.reply.trim().to_string();
    t.action = t.action.trim().to_lowercase();
    t.argument = t.argument.trim().to_string();
    if !ACTIONS.contains(&t.action.as_str()) {
        t.action = "none".into();
    }
    if t.reply.is_empty() && t.action == "none" {
        return Err("Apple Intelligence không trả về kết quả".into());
    }
    Ok(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_structured_and_errors() {
        let t = parse_output("log rác\n{\"reply\":\" Đã thêm việc. \",\"action\":\"ADD_TASK\",\"argument\":\"mai 9h gọi khách\"}\n").unwrap();
        assert_eq!(t, AiTurn { reply: "Đã thêm việc.".into(), action: "add_task".into(), argument: "mai 9h gọi khách".into() });
        let t = parse_output("{\"reply\":\"Xin chào\",\"action\":\"hack_mac\",\"argument\":\"x\"}").unwrap();
        assert_eq!(t.action, "none", "hành động lạ bị bỏ");
        assert_eq!(parse_output("{\"error\":\"Chưa bật\"}").unwrap_err(), "Chưa bật");
        assert!(parse_output("").is_err());
        assert!(parse_output("{\"reply\":\"\",\"action\":\"none\",\"argument\":\"\"}").is_err());
    }

    #[test]
    fn unavailable_reasons_are_vietnamese() {
        assert!(explain_unavailable("UNAVAILABLE:unavailable(FoundationModels.SystemLanguageModel.Availability.UnavailableReason.appleIntelligenceNotEnabled)").contains("Chưa bật"));
        assert!(explain_unavailable("UNAVAILABLE:modelNotReady").contains("đang tải"));
    }

    #[test]
    fn swift_actions_match() {
        let src = include_str!("../resources/asi_ai.swift");
        for a in ACTIONS {
            assert!(src.contains(a), "asi_ai.swift thiếu hành động {a}");
        }
    }
}
