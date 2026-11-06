use asi_clean::clean::Selection;
use asi_clean::disk::human;
use asi_clean::rules::{self, Level, Tier};
use asi_clean::{config, Engine};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "asi-clean", version, about = "Mini-Asi — quét & dọn rác an toàn")]
struct Cli {
    /// In kết quả dạng JSON (cho app Tauri đọc)
    #[arg(long, global = true)]
    json: bool,
    /// Dùng bộ luật của hệ điều hành khác (macos|linux|windows) — để kiểm thử
    #[arg(long, global = true)]
    os: Option<String>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Tình trạng ổ đĩa
    Status,
    /// Liệt kê các luật dọn dẹp
    Rules,
    /// Quét và báo cáo — không thay đổi gì
    Scan {
        #[arg(long = "rule")]
        rules: Vec<String>,
        /// Hiện chi tiết từng mục
        #[arg(short, long)]
        verbose: bool,
    },
    /// Dọn dẹp. Mặc định chỉ xem trước; thêm --yes để thực hiện.
    Clean {
        /// Luật cần dọn. Bỏ trống = toàn bộ luật "An toàn" của gói hiện tại
        #[arg(long = "rule")]
        rules: Vec<String>,
        #[arg(long)]
        yes: bool,
    },
    /// Lịch sử dọn dẹp
    History {
        #[arg(short, default_value_t = 20)]
        n: usize,
    },
}

fn main() {
    let cli = Cli::parse();
    let paths = config::Paths::detect();
    let tier = config::current_tier();
    let mut engine = match Engine::new(paths, tier) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Lỗi: {e}");
            std::process::exit(2);
        }
    };
    if let Some(os) = &cli.os {
        let src = match os.as_str() {
            "macos" => rules::MACOS_RULES,
            "linux" => rules::LINUX_RULES,
            "windows" => rules::WINDOWS_RULES,
            _ => {
                eprintln!("--os phải là macos|linux|windows");
                std::process::exit(2);
            }
        };
        engine.rules = rules::parse_rules(src).expect("luật nhúng hợp lệ");
    }

    match cli.cmd {
        Cmd::Status => {
            let d = engine.disk();
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&d).unwrap());
            } else if let Some(d) = d {
                let face = match d.level {
                    asi_clean::disk::DiskLevel::Ok => "(•‿•)  ổn",
                    asi_clean::disk::DiskLevel::Warn => "(•_•;) sắp đầy",
                    asi_clean::disk::DiskLevel::Critical => "(⊙﹏⊙) báo động!",
                };
                println!("Ổ đĩa: trống {} / {} ({:.1}%)  {face}", human(d.free_bytes), human(d.total_bytes), d.free_percent);
            } else {
                println!("Không đọc được thông tin ổ đĩa");
            }
        }
        Cmd::Rules => {
            for r in &engine.rules {
                println!(
                    "{:<26} {:<7} {:<4} {:<6} {}",
                    r.id,
                    lvl(r.level),
                    tier_s(r.tier),
                    format!("{:?}", r.action).to_lowercase(),
                    r.name
                );
            }
        }
        Cmd::Scan { rules, verbose } => {
            let rep = engine.scan(rules);
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&rep).unwrap());
                return;
            }
            println!("Gói hiện tại: {}\n", tier_s(engine.tier));
            for r in rep.rules.iter().filter(|r| r.bytes > 0) {
                let lock = if r.locked { "🔒 Pro" } else { "" };
                println!(
                    "  {} {:<52} {:>10}  {:>4} mục  {lock}",
                    if r.level == Level::Safe { "🟢" } else { "🟡" },
                    r.name,
                    human(r.bytes),
                    r.items.len()
                );
                if verbose {
                    for c in r.items.iter().take(15) {
                        println!("        {:>10}  {:>4} ngày  {}", human(c.bytes), c.age_days, c.path.display());
                    }
                }
            }
            println!("\n  Dọn an toàn ngay (1 click):  {}", human(rep.safe_bytes));
            println!("  Tổng có thể giải phóng:      {}", human(rep.available_bytes));
            if rep.locked_bytes > 0 {
                println!("  ✨ Bản Pro dọn thêm được:     {}", human(rep.locked_bytes));
            }
            if !rep.skipped.is_empty() {
                println!("\n  Đã bỏ qua {} mục để an toàn (dùng --verbose để xem)", rep.skipped.len());
                if verbose {
                    for s in rep.skipped.iter().take(30) {
                        println!("        [{}] {} — {}", s.rule_id, s.path.display(), s.reason);
                    }
                }
            }
        }
        Cmd::Clean { rules, yes } => {
            let rep = engine.scan(vec![]);
            let rules = if rules.is_empty() {
                rep.rules.iter().filter(|r| r.level == Level::Safe && !r.locked).map(|r| r.id.clone()).collect()
            } else {
                rules
            };
            let sel = Selection { rules, paths: vec![] };
            let res = engine.clean(&rep, &sel, !yes);
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&res).unwrap());
                return;
            }
            if res.dry_run {
                println!("XEM TRƯỚC (chưa xoá gì). Chạy lại với --yes để thực hiện.");
            } else {
                println!("ĐÃ DỌN XONG ٩(◕‿◕)۶");
            }
            println!("  Xoá hẳn (giải phóng ngay):   {}", human(res.deleted_bytes));
            println!("  Chuyển vào Thùng rác:        {}", human(res.trashed_bytes));
            println!("  Số mục:                      {}", res.items_done);
            for l in &res.locked_rules {
                println!("  🔒 {l} cần bản Pro");
            }
            for f in &res.failures {
                println!("  ⚠ {} — {}", f.path.display(), f.reason);
            }
            if !res.dry_run {
                println!("  Nhật ký: {}", engine.log_file().display());
            }
        }
        Cmd::History { n } => {
            let h = engine.history(n);
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&h).unwrap());
                return;
            }
            for r in h {
                println!("{}  {:<6} {:>10}  {}", r.at, format!("{:?}", r.action).to_lowercase(), human(r.bytes), r.path.display());
            }
        }
    }
}

fn lvl(l: Level) -> &'static str {
    match l {
        Level::Safe => "an-toan",
        Level::Review => "xem-lai",
    }
}

fn tier_s(t: Tier) -> &'static str {
    match t {
        Tier::Free => "Free",
        Tier::Pro => "Pro",
    }
}
