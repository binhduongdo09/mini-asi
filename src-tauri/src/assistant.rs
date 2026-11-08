//! Trợ lý Asi: hiểu câu lệnh tiếng Việt / tiếng Anh (gõ hoặc nói) và trả lời bằng dữ liệu thật
//! trên máy — chạy hoàn toàn offline, không gửi gì ra ngoài.

use crate::nlp::fold;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Intent {
    Status,
    Clean,
    Overdue,
    Today,
    AddTask,
    AddNote,
    Focus,
    Break,
    Disk,
    Battery,
    LastClip,
    Journal,
    Vocab,
    Threads,
    Time,
    Help,
    Greet,
    Thanks,
    Unknown,
}

fn has_any(s: &str, keys: &[&str]) -> bool {
    keys.iter().any(|k| s.contains(k))
}

/// Phân loại câu nói. Trả về ý định + phần nội dung còn lại (vd. nội dung việc cần thêm).
pub fn classify(input: &str) -> (Intent, String) {
    use unicode_normalization::UnicodeNormalization;
    let raw: String = input.trim().trim_end_matches(['.', '?', '!']).nfc().map(|c| if c == ':' { ' ' } else { c }).collect();
    // dấu phẩy sau từ khoá (vd. "Nhắc tôi, 3h chiều…" do nhận dạng giọng nói thêm vào) → coi như dấu cách
    let f = format!(" {} ", fold(&raw).replace(',', " "));
    // Thêm việc / ghi chú: lấy phần sau từ khoá
    for (keys, intent) in [
        (&[
            "them cong viec ", "tao cong viec ", "them mot cong viec ", "tao mot cong viec ", "them mot viec ", "tao mot viec ", "them mot task ", "tao mot task ",
            "them nhiem vu ", "tao nhiem vu ", "them viec ", "tao viec ", "them task ", "tao task ", "them lich ", "nhac nho toi ", "nhac nho minh ",
            "nhac toi ", "nhac minh ", "nhac em ", "nhac anh ", "add a task ", "create a task ", "add task ", "remind me to ", "remind me ", "new task ",
        ][..], Intent::AddTask),
        (&["ghi chu ", "ghi lai ", "luu y ", "note ", "take a note ", "write down "][..], Intent::AddNote),
    ] {
        for k in keys {
            if let Some(pos) = f.find(k) {
                // cắt trên chuỗi gốc theo số ký tự (fold giữ nguyên số ký tự)
                let start_chars = f[..pos + k.len()].chars().count().saturating_sub(1);
                let rest: String = raw.chars().skip(start_chars).collect::<String>().trim_start_matches([':', ',', '-', ' ']).to_string();
                if !rest.trim().is_empty() || intent == Intent::AddNote {
                    return (intent, rest.trim().to_string());
                }
            }
        }
    }
    // Các câu yêu cầu suy luận, tóm tắt, dịch thuật, giải thích, viết lách → chuyển thẳng cho Apple Intelligence (Intent::Unknown)
    if has_any(
        &f,
        &[
            "tom tat ", "dich ", "viet ", "giai thich ", "goi y ", "phan tich ", "so sanh ", "tai sao ", "lam sao ",
            "uu tien ", "ke hoach ", "summarize ", "translate ", "explain ", "write ", "why ", "brainstorm ", "prioritize ",
        ],
    ) {
        return (Intent::Unknown, raw);
    }
    let rules: &[(&[&str], Intent)] = &[
        (&["don dep", "don rac", "don may", "giai phong", "xoa rac", "clean", "free up", "junk"], Intent::Clean),
        (&["tre", "cham", "qua han", "tre han", "overdue", "late", "lo han", "chua xong"], Intent::Overdue),
        (&["pin ", "pin?", "battery", "sac pin"], Intent::Battery),
        (&["dung luong", "o dia", "o cung", "bo nho trong", "disk", "storage", "space"], Intent::Disk),
        (&["tinh trang", "may minh the nao", "may the nao", "co van de", "kiem tra may", "check may", "suc khoe may", "may on", "may cham", "may nong", "status", "how is my", "problem", "health check", "lag"], Intent::Status),
        (&["hom nay co gi", "lich hom nay", "hom nay lam gi", "viec hom nay", "today", "agenda", "schedule", "lich hop", "cuoc hop"], Intent::Today),
        (&["tap trung", "focus", "pomodoro"], Intent::Focus),
        (&["nghi ngoi", "nghi mat", "toi met", "minh met", "met qua", "break", "tired", "rest"], Intent::Break),
        (&["vua chep", "clipboard", "vua copy", "da chep"], Intent::LastClip),
        (&["nhat ky", "journal", "diary"], Intent::Journal),
        (&["tu moi", "hoc tu", "hoc tieng anh", "tu vung", "vocabulary", "new word"], Intent::Vocab),
        (&["threads", "thread"], Intent::Threads),
        (&["may gio", "bay gio la", "what time", "hom nay la ngay", "ngay bao nhieu"], Intent::Time),
        (&["lam duoc gi", "giup gi", "huong dan", "help", "what can you"], Intent::Help),
        (&["cam on", "thank"], Intent::Thanks),
        (&["xin chao", "chao asi", "hello", " hi ", "hey asi", "chao ban"], Intent::Greet),
    ];
    for (keys, intent) in rules {
        if has_any(&f, keys) {
            return (*intent, String::new());
        }
    }
    // Câu có giờ hẹn rõ ràng → hiểu là thêm việc
    if crate::nlp::parse(&raw, chrono::Local::now().naive_local()).due.is_some() {
        return (Intent::AddTask, raw);
    }
    (Intent::Unknown, raw)
}

// ───────────────────────── Thông tin máy ─────────────────────────

#[derive(Debug, Clone, Serialize, Default)]
pub struct SysInfo {
    pub cpu_load: Option<f64>, // 0..1+ (tải trung bình 1 phút / số nhân)
    pub cores: usize,
    pub mem_used_pct: Option<f64>,
    pub mem_total_gb: Option<f64>,
    pub battery_pct: Option<u32>,
    pub charging: Option<bool>,
    pub uptime_days: Option<f64>,
}

pub fn sysinfo() -> SysInfo {
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    let mut s = SysInfo { cores, ..Default::default() };
    #[cfg(unix)]
    {
        let mut l = [0f64; 3];
        if unsafe { libc::getloadavg(l.as_mut_ptr(), 3) } > 0 {
            s.cpu_load = Some(l[0] / cores as f64);
        }
    }
    let run = |c: &str, a: &[&str]| std::process::Command::new(c).args(a).output().ok().map(|o| String::from_utf8_lossy(&o.stdout).to_string());
    #[cfg(target_os = "macos")]
    {
        if let Some(total) = run("sysctl", &["-n", "hw.memsize"]).and_then(|v| v.trim().parse::<f64>().ok()) {
            s.mem_total_gb = Some(total / 1e9);
            if let Some(vm) = run("vm_stat", &[]) {
                let page = vm.lines().next().and_then(|l| l.split("page size of").nth(1)).and_then(|x| x.split_whitespace().next()).and_then(|x| x.parse::<f64>().ok()).unwrap_or(16384.0);
                let get = |k: &str| vm.lines().find(|l| l.starts_with(k)).and_then(|l| l.split(':').nth(1)).and_then(|v| v.trim().trim_end_matches('.').parse::<f64>().ok()).unwrap_or(0.0);
                let used = (get("Pages active") + get("Pages wired down") + get("Pages occupied by compressor")) * page;
                s.mem_used_pct = Some((used / total * 100.0).min(100.0));
            }
        }
        if let Some(b) = run("pmset", &["-g", "batt"]) {
            if let Some(p) = b.split('%').next().and_then(|x| x.rsplit(|c: char| !c.is_ascii_digit()).next()).and_then(|x| x.parse().ok()) {
                s.battery_pct = Some(p);
                s.charging = Some(b.contains("AC Power") || b.contains("charging;") && !b.contains("discharging"));
            }
        }
        if let Some(bt) = run("sysctl", &["-n", "kern.boottime"]) {
            if let Some(sec) = bt.split("sec = ").nth(1).and_then(|x| x.split(',').next()).and_then(|x| x.trim().parse::<i64>().ok()) {
                s.uptime_days = Some((chrono::Utc::now().timestamp() - sec) as f64 / 86400.0);
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(m) = std::fs::read_to_string("/proc/meminfo") {
            let get = |k: &str| m.lines().find(|l| l.starts_with(k)).and_then(|l| l.split_whitespace().nth(1)).and_then(|v| v.parse::<f64>().ok());
            if let (Some(t), Some(a)) = (get("MemTotal:"), get("MemAvailable:")) {
                s.mem_total_gb = Some(t * 1024.0 / 1e9);
                s.mem_used_pct = Some((1.0 - a / t) * 100.0);
            }
        }
        if let Ok(c) = std::fs::read_to_string("/sys/class/power_supply/BAT0/capacity") {
            s.battery_pct = c.trim().parse().ok();
            s.charging = std::fs::read_to_string("/sys/class/power_supply/BAT0/status").ok().map(|x| x.contains("Charging"));
        }
        if let Ok(u) = std::fs::read_to_string("/proc/uptime") {
            s.uptime_days = u.split_whitespace().next().and_then(|x| x.parse::<f64>().ok()).map(|x| x / 86400.0);
        }
    }
    let _ = run;
    s
}

// ───────────────────────── Câu trả lời ─────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct Metric {
    pub label: String,
    pub value: String,
    /// 0 tốt · 1 lưu ý · 2 xấu
    pub level: u8,
    /// 0..1 để vẽ thanh
    pub ratio: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskBrief {
    pub id: u64,
    pub title: String,
    pub due: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Card {
    Metrics { items: Vec<Metric> },
    Tasks { items: Vec<TaskBrief> },
    Events { items: Vec<(String, i64, bool)> },
    Text { text: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct Action {
    pub label: String,
    /// lệnh: clean_safe | open_tab | focus | break_done | task_done | task_snooze | speak_stop
    pub cmd: String,
    pub arg: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Reply {
    pub intent: Intent,
    pub text: String,
    /// câu ngắn để đọc to
    pub speak: String,
    pub mood: String,
    pub cards: Vec<Card>,
    pub actions: Vec<Action>,
}

impl Reply {
    pub fn new(intent: Intent, text: impl Into<String>) -> Self {
        let t = text.into();
        Self { intent, speak: t.clone(), text: t, mood: "happy".into(), cards: vec![], actions: vec![] }
    }
    pub fn act(mut self, label: &str, cmd: &str, arg: &str) -> Self {
        self.actions.push(Action { label: label.into(), cmd: cmd.into(), arg: arg.into() });
        self
    }
}

pub fn help(en: bool) -> Reply {
    let t = if en {
        "I can: check your Mac, clean junk, list overdue tasks, show today's agenda, add tasks (\"remind me to call John at 3pm\"), take notes, start focus timers…"
    } else {
        "Asi giúp được: kiểm tra máy, dọn rác, xem việc trễ, lịch hôm nay, thêm việc (\"nhắc tôi 3h chiều gọi khách\"), ghi chú, hẹn giờ tập trung, mở nhật ký, mở Threads…"
    };
    let mut r = Reply::new(Intent::Help, t);
    r.speak = if en { "Here is what I can do.".into() } else { "Đây là những việc Asi làm được nè.".into() };
    r.act(if en { "Check my Mac" } else { "Máy mình thế nào?" }, "ask", if en { "how is my mac" } else { "máy mình thế nào" })
        .act(if en { "Clean junk" } else { "Dọn rác giúp mình" }, "ask", "dọn rác")
        .act(if en { "Overdue tasks" } else { "Có việc nào trễ không?" }, "ask", "việc nào trễ")
}

// ───────────────────────── Apple Intelligence (FoundationModels On-Device) ─────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct AppleAiStatus {
    pub available: bool,
    pub engine: String,
    pub detail: String,
}

#[cfg(target_os = "macos")]
const APPLE_AI_SWIFT_SRC: &str = r#"import Foundation
import FoundationModels

@main
struct AsiAppleAI {
    static func main() async {
        let args = CommandLine.arguments
        if args.contains("--check") {
            if SystemLanguageModel.default.isAvailable {
                print("READY")
            } else {
                print("UNAVAILABLE:\(SystemLanguageModel.default.availability)")
            }
            return
        }
        let sysPrompt = ProcessInfo.processInfo.environment["ASI_SYSTEM"] ?? "You are Asi, a smart desktop assistant living in the MacBook notch."
        let userPrompt = ProcessInfo.processInfo.environment["ASI_PROMPT"] ?? ""
        guard !userPrompt.isEmpty else {
            print("ERR:Câu hỏi trống")
            return
        }
        guard SystemLanguageModel.default.isAvailable else {
            print("ERR:Apple Intelligence chưa được bật trên máy Mac này.")
            return
        }
        do {
            let model = SystemLanguageModel(useCase: .general, guardrails: .permissiveContentTransformations)
            let session = LanguageModelSession(model: model, instructions: sysPrompt)
            let res = try await session.respond(to: userPrompt)
            print(res.content)
        } catch {
            do {
                let model2 = SystemLanguageModel(useCase: .general, guardrails: .permissiveContentTransformations)
                let session2 = LanguageModelSession(model: model2, instructions: "Answer the user's request helpfully, concisely, and naturally in the same language as the user (Vietnamese or English).")
                let res2 = try await session2.respond(to: userPrompt)
                print(res2.content)
            } catch {
                print("ERR:\(error.localizedDescription)")
            }
        }
    }
}
"#;

/// Biên dịch (1 lần duy nhất) công cụ gọi `FoundationModels` của macOS 26+ vào thư mục dữ liệu ứng dụng.
/// Trả về đường dẫn file thực thi nếu máy hỗ trợ Apple Intelligence.
pub fn ensure_apple_ai_bin(data_dir: &std::path::Path) -> Option<std::path::PathBuf> {
    #[cfg(target_os = "macos")]
    {
        if !std::path::Path::new("/usr/bin/swiftc").exists() {
            return None;
        }
        let _ = std::fs::create_dir_all(data_dir);
        let swift_path = data_dir.join("apple_ai_v1.swift");
        let bin_path = data_dir.join("apple_ai_v1_bin");
        let need_compile = !bin_path.exists()
            || std::fs::read_to_string(&swift_path).ok().as_deref() != Some(APPLE_AI_SWIFT_SRC);
        if need_compile {
            let _ = std::fs::write(&swift_path, APPLE_AI_SWIFT_SRC);
            let status = std::process::Command::new("/usr/bin/swiftc")
                .args(["-parse-as-library", "-O"])
                .arg(&swift_path)
                .arg("-o")
                .arg(&bin_path)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .ok()?;
            if !status.success() {
                return None;
            }
        }
        if bin_path.exists() {
            return Some(bin_path);
        }
        None
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = data_dir;
        None
    }
}

pub fn check_apple_ai(data_dir: &std::path::Path) -> AppleAiStatus {
    if let Some(bin) = ensure_apple_ai_bin(data_dir) {
        if let Ok(out) = std::process::Command::new(bin).arg("--check").output() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if s == "READY" {
                return AppleAiStatus {
                    available: true,
                    engine: "Apple Intelligence (FoundationModels)".into(),
                    detail: "Sẵn sàng · Chạy trực tiếp trên chip Apple Silicon (100% riêng tư, offline)".into(),
                };
            }
            return AppleAiStatus {
                available: false,
                engine: "Apple Intelligence".into(),
                detail: format!("Chưa sẵn sàng trên hệ thống ({s}) — hãy bật Apple Intelligence trong Cài đặt hệ thống"),
            };
        }
    }
    AppleAiStatus {
        available: false,
        engine: "Bộ phân tích ngôn ngữ tự nhiên nội bộ".into(),
        detail: "Yêu cầu macOS 26+ với Apple Intelligence đã bật".into(),
    }
}

pub fn ask_apple_ai(data_dir: &std::path::Path, context: &str, prompt: &str, en: bool) -> Result<String, String> {
    let bin = ensure_apple_ai_bin(data_dir).ok_or("Máy chưa hỗ trợ Apple Intelligence (FoundationModels)")?;
    let lang_rule = if en {
        "Always reply in English unless the user explicitly asks for Vietnamese."
    } else {
        "Always reply in natural, friendly Vietnamese (Tiếng Việt có dấu) unless the user asks to translate into another language."
    };
    let sys_prompt = format!(
        "You are Asi, a smart, warm desktop AI assistant living in the MacBook notch (Mini-Asi app).\n\
         {lang_rule}\n\
         Keep your response concise, well-structured, and directly helpful (around 2-6 sentences or bullet points).\n\
         Live context from the user's Mac right now (use this only when relevant to the user's question):\n\
         {context}"
    );
    let out = std::process::Command::new(bin)
        .env("ASI_SYSTEM", sys_prompt)
        .env("ASI_PROMPT", prompt)
        .output()
        .map_err(|e| format!("Lỗi gọi Apple Intelligence: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if text.is_empty() {
        return Err("Apple Intelligence không trả về kết quả".into());
    }
    if let Some(err) = text.strip_prefix("ERR:") {
        return Err(err.trim().to_string());
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intents_vi_en() {
        let i = |s: &str| classify(s).0;
        assert_eq!(i("Máy mình thế nào?"), Intent::Status);
        assert_eq!(i("may co van de gi khong"), Intent::Status);
        assert_eq!(i("Kiểm tra tình trạng máy"), Intent::Status);
        assert_eq!(i("dọn dẹp máy giúp mình"), Intent::Clean);
        assert_eq!(i("Có task nào đang chậm không"), Intent::Overdue);
        assert_eq!(i("việc nào quá hạn"), Intent::Overdue);
        assert_eq!(i("hôm nay có gì"), Intent::Today);
        assert_eq!(i("còn bao nhiêu dung lượng"), Intent::Disk);
        assert_eq!(i("pin còn bao nhiêu"), Intent::Battery);
        assert_eq!(i("how is my mac"), Intent::Status);
        assert_eq!(i("clean up my computer"), Intent::Clean);
        assert_eq!(i("any overdue tasks?"), Intent::Overdue);
        assert_eq!(i("tập trung 25 phút"), Intent::Focus);
        assert_eq!(i("bạn làm được gì"), Intent::Help);
        assert_eq!(i("xin chào Asi"), Intent::Greet);
        let (a, rest) = classify("Nhắc tôi mai 9h gọi khách Hoiana");
        assert_eq!(a, Intent::AddTask);
        assert_eq!(rest, "mai 9h gọi khách Hoiana");
        let (a, rest) = classify("remind me to call John at 3pm");
        assert_eq!((a, rest.as_str()), (Intent::AddTask, "call John at 3pm"));
        let (a, rest) = classify("ghi chú: mật khẩu wifi tầng 3 là 123");
        assert_eq!((a, rest.as_str()), (Intent::AddNote, "mật khẩu wifi tầng 3 là 123"));
        assert_eq!(classify("thứ 6 nộp báo cáo").0, Intent::AddTask, "câu có giờ hẹn → thêm việc");
        assert_eq!(classify("Nhắc tôi, 3 giờ chiều gọi khách.").1, "3 giờ chiều gọi khách");
        assert_eq!(classify("Thêm công việc mua sữa cho con"), (Intent::AddTask, "mua sữa cho con".to_string()));
        assert_eq!(classify("tạo một task kiểm tra hợp đồng").1, "kiểm tra hợp đồng");
        assert_eq!(classify("con mèo").0, Intent::Unknown);
    }

    #[test]
    fn sysinfo_reads() {
        let s = sysinfo();
        assert!(s.cores >= 1);
    }
}
