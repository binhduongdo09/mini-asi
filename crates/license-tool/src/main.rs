//! Công cụ cho NGƯỜI BÁN (không phát hành cùng app).
//!
//!   masi-license keygen                       → tạo cặp khoá mới
//!   masi-license sign <email> [ngày_hết_hạn]  → cấp khoá Pro (đọc khoá bí mật từ MASI_SECRET_KEY)
//!   masi-license verify <khoá> <khoá_công_khai>

use masi_license::{new_keypair, sign, verify, License};
use std::time::{SystemTime, UNIX_EPOCH};

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("keygen") => {
            let (sk, pk) = new_keypair();
            println!("KHOÁ BÍ MẬT (giữ kín, KHÔNG commit):\n{sk}\n");
            println!("KHOÁ CÔNG KHAI (dán vào src-tauri/src/license.rs → PUBLIC_KEY):\n{pk}");
        }
        Some("sign") => {
            let Some(email) = args.get(1) else { return usage() };
            let sk = std::env::var("MASI_SECRET_KEY").expect("đặt biến MASI_SECRET_KEY");
            let x = args.get(2).map(|d| now() + d.parse::<u64>().expect("số ngày") * 86_400);
            let lic = License { e: email.clone(), p: "pro".into(), i: now(), x };
            println!("{}", sign(&lic, &sk).expect("ký thất bại"));
        }
        Some("verify") => {
            let (Some(k), Some(pk)) = (args.get(1), args.get(2)) else { return usage() };
            match verify(k, pk, now()) {
                Ok(l) => println!("HỢP LỆ: {l:?}"),
                Err(e) => println!("KHÔNG HỢP LỆ: {e}"),
            }
        }
        _ => usage(),
    }
}

fn usage() {
    eprintln!("masi-license keygen | sign <email> [số_ngày] | verify <khoá> <khoá_công_khai>");
}
