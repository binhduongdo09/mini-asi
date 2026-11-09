//! Bước build: (1) trên Mac có Xcode 26 → biên dịch sẵn trợ lý Apple Intelligence (`resources/asi_ai.swift`)
//! và nhúng vào app, để máy người dùng KHÔNG cần cài Xcode Command Line Tools; (2) tauri_build.
use std::path::PathBuf;
use std::process::Command;

fn main() {
    build_apple_ai();
    tauri_build::build()
}

fn build_apple_ai() {
    println!("cargo:rerun-if-changed=resources/asi_ai.swift");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("asi_ai_bin");
    let target_mac = std::env::var("CARGO_CFG_TARGET_OS").map(|t| t == "macos").unwrap_or(false);
    let mut ok = false;
    if target_mac && cfg!(target_os = "macos") {
        let has_swiftc = Command::new("xcrun").args(["--find", "swiftc"]).output().map(|o| o.status.success()).unwrap_or(false);
        if has_swiftc {
            // ưu tiên bản có cấu trúc (mô hình chọn được hành động), không được thì bản chỉ trả lời chữ
            let compile = |structured: bool| {
                let mut c = Command::new("xcrun");
                c.args(["swiftc", "-parse-as-library", "-O", "-target", "arm64-apple-macos26.0"]);
                if structured {
                    c.args(["-D", "ASI_STRUCTURED"]);
                }
                c.args(["resources/asi_ai.swift", "-o"]).arg(&out).status().map(|s| s.success()).unwrap_or(false)
            };
            ok = compile(true) || compile(false);
            if !ok {
                println!("cargo:warning=Chưa biên dịch sẵn được trợ lý Apple Intelligence (cần Xcode 26). App sẽ tự biên dịch trên máy có Command Line Tools.");
            }
        }
    }
    if !ok {
        std::fs::write(&out, b"").expect("ghi asi_ai_bin rỗng");
    }
}
