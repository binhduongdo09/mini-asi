//! Trợ lý Asi — phần hiểu câu lệnh bằng luật (offline, tức thì): tiếng Việt / tiếng Anh, gõ hoặc nói.
//! Câu dài / hỏi đáp tự do do Apple Intelligence xử lý (xem apple_ai.rs và commands/assistant.rs).

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

/// `s` đã được bọc dấu cách hai đầu; khoá chỉ khớp khi đứng thành từ trọn vẹn
/// (để "tre" không khớp "trên", "rest" không khớp "restaurant").
pub(crate) fn has_any(s: &str, keys: &[&str]) -> bool {
    keys.iter().any(|k| {
        let k = k.trim();
        let mut from = 0;
        while let Some(i) = s[from..].find(k) {
            let at = from + i;
            let before = s[..at].chars().next_back().map_or(true, |c| !c.is_alphanumeric());
            let after = s[at + k.len()..].chars().next().map_or(true, |c| !c.is_alphanumeric());
            if before && after {
                return true;
            }
            from = at + k.len().max(1);
        }
        false
    })
}

/// Câu bắt đầu bằng lệnh rõ ràng ("nhắc tôi…", "thêm việc…", "ghi chú…") → (ý định, phần nội dung)
pub fn explicit_command(input: &str) -> Option<(Intent, String)> {
    use unicode_normalization::UnicodeNormalization;
    let raw: String = input.trim().trim_end_matches(['.', '?', '!']).nfc().map(|c| if c == ':' { ' ' } else { c }).collect();
    let f = format!(" {} ", fold(&raw).replace(',', " "));
    for (keys, intent) in [
        (&[
            "them cong viec ", "tao cong viec ", "them mot cong viec ", "tao mot cong viec ", "them mot viec ", "tao mot viec ", "them mot task ", "tao mot task ",
            "them nhiem vu ", "tao nhiem vu ", "them viec ", "tao viec ", "them task ", "tao task ", "them lich ", "nhac nho toi ", "nhac nho minh ",
            "nhac toi ", "nhac minh ", "nhac em ", "nhac anh ", "add a task ", "create a task ", "add task ", "remind me to ", "remind me ", "new task ",
        ][..], Intent::AddTask),
        (&["ghi chu ", "ghi lai ", "luu ghi chu ", "take a note ", "write down ", "note "][..], Intent::AddNote),
    ] {
        for k in keys {
            // chỉ tính khi lệnh nằm ở đầu câu (tối đa 2 từ đệm phía trước: "Asi ơi nhắc tôi…")
            if let Some(pos) = f.find(&format!(" {k}")).or_else(|| f.find(k)) {
                if f[..pos].split_whitespace().count() > 2 {
                    continue;
                }
                let kk = if f[pos..].starts_with(' ') { pos + 1 } else { pos };
                let start_chars = f[..kk + k.len()].chars().count().saturating_sub(1);
                let rest: String = raw.chars().skip(start_chars).collect::<String>().trim_start_matches([':', ',', '-', ' ']).to_string();
                if !rest.trim().is_empty() || intent == Intent::AddNote {
                    return Some((intent, rest.trim().to_string()));
                }
            }
        }
    }
    None
}

/// Nhờ thêm việc / ghi chú nằm GIỮA câu, nói tự nhiên ("bạn thêm giúp mình task gửi báo giá trước thứ 6 nhé",
/// "lưu vào ghi chú: wifi là abc", "please add a task to call John"). `explicit_command` chỉ bắt lệnh ở đầu câu; câu
/// kiểu này trước đây rơi vào AI → mô hình trả lời "đã thêm" nhưng không thật sự thêm. Trả về (ý định, nội dung).
pub fn write_intent(input: &str) -> Option<(Intent, String)> {
    use unicode_normalization::UnicodeNormalization;
    let raw: String = input.trim().nfc().collect();
    let f_all = format!(" {} ", fold(&raw).replace([',', '.', '!', '?', ':', ';'], " "));
    // hỏi cách làm ≠ nhờ làm
    if has_any(&f_all, &["lam sao", "cach nao", "cach de", "huong dan", "how to", "how do", "tai sao", "why", "bang cach nao"]) {
        return None;
    }
    let polite = has_any(&f_all, &["giup", "gium", "ho", "dum", "nhe", "nha", "please", "voi", "di", "cho minh", "cho toi", "cho em", "cho anh"]);
    if looks_like_question(&raw) && !polite {
        return None;
    }
    // tách từ, giữ dấu câu riêng (":" giữa câu)
    let words: Vec<String> = raw.replace(':', " : ").split_whitespace().map(str::to_string).collect();
    let fw: Vec<String> = words.iter().map(|w| fold(w).trim_matches(|c: char| !c.is_alphanumeric()).to_string()).collect();
    let task_verbs = ["them", "tao", "lap", "dat", "len", "add", "create", "make", "new", "ghi", "luu"];
    let task_objs = ["viec", "task", "tasks", "todo", "nhiem", "reminder", "lich"];
    let note_pairs: &[(&str, &[&str])] = &[("ghi", &["chu", "lai", "nho"]), ("luu", &["lai", "chu", "note"]), ("note", &["lai", "nay", "down"]), ("take", &["note"]), ("save", &["note", "notes"]), ("jot", &["down"])];
    let pronouns = ["toi", "minh", "em", "anh", "t", "me", "chi", "tao"];
    let mut hit: Option<(Intent, usize)> = None;
    for i in 0..fw.len() {
        // nhắc + đại từ ở bất kỳ đâu: "nhớ nhắc mình …", "bạn nhắc t …", "remind me …"
        if (fw[i] == "nhac" || fw[i] == "remind") && fw.get(i + 1).is_some_and(|p| pronouns.contains(&p.as_str()) || p == "nho") {
            let j = if fw.get(i + 1).map(String::as_str) == Some("nho") { i + 2 } else { i + 1 };
            let j = if fw.get(j).is_some_and(|p| pronouns.contains(&p.as_str())) { j } else { j - 1 };
            hit = Some((Intent::AddTask, j + 1));
            break;
        }
        // ghi chú / lưu lại / take a note …
        if let Some((_, objs)) = note_pairs.iter().find(|(v, _)| *v == fw[i]) {
            if let Some(k) = (i + 1..(i + 4).min(fw.len())).find(|&k| objs.contains(&fw[k].as_str())) {
                // "ghi lại vào việc" / "lưu thành việc" → là việc
                let is_task = fw[k + 1..(k + 4).min(fw.len())].iter().any(|w| w == "viec" || w == "task");
                let end = if is_task { (k + 1..(k + 4).min(fw.len())).find(|&m| fw[m] == "viec" || fw[m] == "task").unwrap_or(k) } else { k };
                // "lưu vào ghi chú" → bỏ luôn chữ "chú"
                let end = if fw.get(end + 1).map(String::as_str) == Some("chu") && fw[end] == "ghi" { end + 1 } else { end };
                hit = Some((if is_task { Intent::AddTask } else { Intent::AddNote }, end + 1));
                break;
            }
            // "lưu vào ghi chú"
            if fw[i] == "luu" {
                if let Some(k) = (i + 1..(i + 4).min(fw.len())).find(|&k| fw[k] == "ghi" && fw.get(k + 1).map(String::as_str) == Some("chu")) {
                    hit = Some((Intent::AddNote, k + 2));
                    break;
                }
            }
        }
        if task_verbs.contains(&fw[i].as_str()) && fw[i] != "ghi" {
            if let Some(k) = (i + 1..(i + 6).min(fw.len())).find(|&k| task_objs.contains(&fw[k].as_str())) {
                // "lên lịch" chỉ khi đứng liền; "nhiệm vụ" → bỏ cả chữ "vụ"
                if fw[k] == "lich" && k != i + 1 {
                    continue;
                }
                let end = if fw[k] == "nhiem" && fw.get(k + 1).map(String::as_str) == Some("vu") { k + 1 } else { k };
                // "thêm vào ghi chú" → ghi chú
                hit = Some((Intent::AddTask, end + 1));
                break;
            }
            if let Some(k) = (i + 1..(i + 5).min(fw.len())).find(|&k| fw[k] == "ghi" && fw.get(k + 1).map(String::as_str) == Some("chu")) {
                hit = Some((Intent::AddNote, k + 2));
                break;
            }
        }
    }
    let (intent, start) = hit?;
    // bỏ từ đệm đầu nội dung
    const LEAD: &[&str] = &["", "giup", "gium", "ho", "dum", "vao", "danh", "sach", "moi", "la", "rang", "sau", "nhu", "1", "mot", "cai", "noi", "dung", "voi", "ve", "to", "a", "an", "that", "list", "my", "lai", "giup"];
    let mut s = start;
    while s < fw.len() {
        let w = fw[s].as_str();
        if LEAD.contains(&w) || words[s] == ":" {
            s += 1;
        } else if (w == "cho" || w == "giup") && fw.get(s + 1).is_some_and(|p| pronouns.contains(&p.as_str())) {
            s += 2;
        } else if pronouns.contains(&w) && s > start && matches!(fw[s - 1].as_str(), "giup" | "gium" | "ho" | "dum" | "cho") {
            s += 1;
        } else if (w == "viec" || w == "task" || w == "note") && s == start {
            s += 1;
        } else {
            break;
        }
    }
    // "note lại ý này: …", "ghi chú cái sau: …" → lấy phần sau dấu ":" nếu nó nằm gần đầu
    if let Some(c) = (s..(s + 5).min(words.len())).find(|&k| words[k] == ":") {
        s = c + 1;
    }
    // bỏ từ đệm cuối câu
    const TRAIL: &[&str] = &["nhe", "nha", "voi", "di", "please", "thanks", "giup", "gium", "duoc", "khong", "ko", "k", "dc", "nhi", "a", "ah", "nhen"];
    let mut e = fw.len();
    loop {
        if e > s && (TRAIL.contains(&fw[e - 1].as_str()) || words[e - 1] == ":") {
            e -= 1;
        } else if e > s + 1 && pronouns.contains(&fw[e - 1].as_str()) && matches!(fw[e - 2].as_str(), "giup" | "gium" | "cho" | "ho" | "dum") {
            // "… giúp mình" / "… cho tôi" ở cuối (không bỏ "tối" đứng một mình — có thể là "8h tối")
            e -= 2;
        } else {
            break;
        }
    }
    let content = words[s..e.max(s)].join(" ").replace(" : ", ": ").trim_matches(|c: char| c == ':' || c == ',' || c == '-' || c.is_whitespace()).trim_end_matches(['.', '!', '?']).to_string();
    if content.is_empty() && intent == Intent::AddTask {
        return None;
    }
    Some((intent, content))
}

/// Mô hình nói đã làm ("đã thêm", "đã ghi lại", "added"…) — dùng để bắt trường hợp nói mà không làm
pub fn claims_done(reply: &str) -> bool {
    let f = format!(" {} ", fold(reply));
    has_any(&f, &["da them", "da tao", "da ghi", "da luu", "da dat", "se nhac", "da len lich", "added", "noted", "saved", "i will remind", "i ll remind", "created"])
}

/// Câu hỏi (không phải mệnh lệnh): có "?" hoặc từ để hỏi tiếng Việt / tiếng Anh
pub fn looks_like_question(input: &str) -> bool {
    if input.trim_end().ends_with('?') {
        return true;
    }
    let f = format!(" {} ", fold(input.trim()).replace([',', '.', '!'], " "));
    has_any(&f, &[
        "khong", "chua", "gi", "sao", "nao", "bao nhieu", "bao gio", "khi nao", "the nao", "lam sao", "o dau", "la ai", "co phai", "duoc khong",
        "what", "why", "how", "when", "where", "who", "which", "is it", "can you", "could you", "should i",
    ]) && !f.trim_start().starts_with("nhac ")
}

/// Người dùng có thật sự nhờ app LÀM việc này không (chặn mô hình tự thêm việc / ghi chú khi người dùng chỉ hỏi)
pub fn user_asks_for(action: &str, input: &str) -> bool {
    let f = format!(" {} ", fold(input.trim()).replace([',', '.', '!', '?', ':'], " "));
    let keys: &[&str] = match action {
        "add_task" => &["nhac", "them viec", "them cong viec", "tao viec", "tao cong viec", "them task", "tao task", "them nhiem vu", "viec can lam", "len lich", "dat lich", "hen gio", "dat hen", "to do", "todo", "remind", "add task", "add a task", "create a task", "schedule"],
        "add_note" => &["ghi chu", "ghi lai", "luu lai", "luu vao", "luu thanh", "note", "save", "write down"],
        "start_focus" => &["tap trung", "focus", "pomodoro"],
        _ => return true,
    };
    has_any(&f, keys)
        || match action {
            "add_task" => matches!(write_intent(input), Some((Intent::AddTask, _))),
            "add_note" => matches!(write_intent(input), Some((Intent::AddNote, _))),
            _ => false,
        }
}

/// Phân loại câu nói. Trả về ý định + phần nội dung còn lại (vd. nội dung việc cần thêm).
pub fn classify(input: &str) -> (Intent, String) {
    use unicode_normalization::UnicodeNormalization;
    let raw: String = input.trim().trim_end_matches(['.', '?', '!']).nfc().map(|c| if c == ':' { ' ' } else { c }).collect();
    // dấu phẩy sau từ khoá (vd. "Nhắc tôi, 3h chiều…" do nhận dạng giọng nói thêm vào) → coi như dấu cách
    let f = format!(" {} ", fold(&raw).replace(',', " "));
    if let Some(hit) = explicit_command(input) {
        return hit;
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
    // Câu có giờ hẹn rõ ràng (và không phải câu hỏi) → hiểu là thêm việc
    if !looks_like_question(input) && crate::nlp::parse(&raw, chrono::Local::now().naive_local()).due.is_some() {
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
    #[allow(unused_mut)]
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
    /// Danh sách link (tin tức…): bấm để mở trong trình duyệt
    Links { items: Vec<LinkItem> },
}

#[derive(Debug, Clone, Serialize)]
pub struct LinkItem {
    pub title: String,
    pub url: String,
    /// dòng phụ: "VnExpress · 2 giờ trước"
    pub meta: String,
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
    /// ai đã trả lời: "Apple Intelligence" | "ChatGPT · gpt-5-mini" | "Lệnh cơ bản" … (hiện nhỏ dưới câu trả lời)
    pub engine: String,
}

impl Reply {
    pub fn new(intent: Intent, text: impl Into<String>) -> Self {
        let t = text.into();
        Self { intent, speak: t.clone(), text: t, mood: "happy".into(), cards: vec![], actions: vec![], engine: String::new() }
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


// ───────── chống lặp câu trả lời (mô hình nhỏ trên máy hay chép lại câu trả lời trước) ─────────

/// Chuẩn hoá để so sánh: bỏ dấu, chữ thường, chỉ giữ chữ & số
pub fn norm(s: &str) -> String {
    fold(s)
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .map(|w| match w {
            // viết tắt hay gặp khi chat
            "ko" | "k" | "khg" | "hok" | "hong" | "kh" | "kg" => "khong",
            "dc" | "đc" => "duoc",
            "j" => "gi",
            "ntn" => "nhu the nao",
            _ => w,
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Câu nối tiếp cần ngữ cảnh hội thoại ("còn mai thì sao", "ngắn hơn", "dịch sang tiếng Anh", "nó là gì")
pub fn is_follow_up(input: &str) -> bool {
    let f = format!(" {} ", norm(input));
    let words = f.split_whitespace().count();
    let starts = ["con ", "the con ", "vay con ", "vay ", "the ", "tai sao ", "sao ", "and ", "what about ", "how about ", "why "];
    starts.iter().any(|p| f.trim_start().starts_with(p))
        || has_any(&f, &["no", "cai do", "cai nay", "dieu do", "dieu nay", "y ban", "ngan hon", "ngan gon hon", "dai hon", "chi tiet hon", "cu the hon", "giai thich them", "noi them",
            "viet lai", "sua lai", "lam lai", "dich", "dich sang", "viet tiep", "tiep tuc", "them nua", "vi du", "con gi nua", "cau tren", "doan tren", "ban vua", "luc nay",
            "it", "that", "this", "shorter", "longer", "translate", "rewrite", "more", "again", "above", "continue"])
            && words <= 12
}

/// Câu trả lời có phải chỉ nhắc lại câu hỏi, hoặc chép nguyên một câu trả lời trước cho câu hỏi KHÁC không
pub fn is_repeat(question: &str, reply: &str, recent: &[(String, String)]) -> bool {
    let (q, a) = (norm(question), norm(reply));
    if a.is_empty() {
        return false;
    }
    // nhại lại câu hỏi ("Tin công nghệ hôm nay có gì không?")
    let (qw, aw): (Vec<&str>, Vec<&str>) = (q.split_whitespace().collect(), a.split_whitespace().collect());
    let shared = aw.iter().filter(|w| qw.contains(w)).count();
    let echo = a == q
        || (q.len() >= 8 && a.contains(&q) && a.len() <= q.len() + 12)
        || (a.len() >= 8 && q.contains(&a))
        || (aw.len() >= 3 && aw.len() <= qw.len() + 2 && shared * 10 >= aw.len() * 9);
    echo || recent.iter().any(|(q0, a0)| norm(a0) == a && norm(q0) != q)
}

#[cfg(test)]
mod tests {
    #[test]
    fn repeat_and_follow_up_detection() {
        let recent = vec![
            ("nhiệt độ hôm nay thế nào".to_string(), "Thời tiết hôm nay có mưa rào, bạn nên mặc áo mưa.".to_string()),
            ("giá vàng hôm nay thế nào".to_string(), "Giá vàng hôm nay là 1.650.000 VND/ounce.".to_string()),
        ];
        // đúng các lỗi trong ảnh chụp
        assert!(is_repeat("thị trường hôm nay thế nào", "Thời tiết hôm nay có mưa rào, bạn nên mặc áo mưa.", &recent));
        assert!(is_repeat("thị trường hôm nay thế nào", "Giá vàng hôm nay là 1.650.000 VND/ounce.", &recent));
        assert!(is_repeat("Tin công nghệ hôm nay có gì ko", "Tin công nghệ hôm nay có gì không?", &recent));
        assert!(!is_repeat("Hà Nội có mấy quận", "Hà Nội có 12 quận và 17 huyện, 1 thị xã.", &recent));
        assert!(is_repeat("Tin công nghệ hôm nay có gì không", "Tin công nghệ hôm nay có gì không?", &recent));
        // hỏi lại đúng câu cũ → được phép trả lời giống
        assert!(!is_repeat("nhiệt độ hôm nay thế nào", "Thời tiết hôm nay có mưa rào, bạn nên mặc áo mưa.", &recent));
        assert!(!is_repeat("viết lời chúc sinh nhật", "Chúc bạn tuổi mới thật nhiều niềm vui!", &recent));
        assert!(!is_repeat("x", "", &recent));
        assert!(is_follow_up("còn ngày mai thì sao"));
        assert!(is_follow_up("viết ngắn hơn"));
        assert!(is_follow_up("dịch sang tiếng Anh"));
        assert!(is_follow_up("make it shorter"));
        assert!(!is_follow_up("thị trường hôm nay thế nào"));
        assert!(!is_follow_up("Công việc hôm nay thế nào"));
        assert!(!is_follow_up("giải thích lạm phát là gì"));
    }

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
    fn keywords_match_whole_words_only() {
        // "tre" (trễ) không được khớp "trên"; "rest" không khớp "restaurant"
        assert_ne!(classify("kể chuyện con mèo trên mái nhà").0, Intent::Overdue);
        assert_ne!(classify("restaurant nào ngon ở Hà Nội").0, Intent::Break);
        assert_eq!(classify("việc nào trễ").0, Intent::Overdue);
    }

    #[test]
    fn questions_are_not_tasks() {
        assert!(looks_like_question("Hà Nội mai có mưa không"));
        assert!(looks_like_question("Thứ 6 này nên mặc gì đi đám cưới?"));
        assert!(!looks_like_question("mai 9h gọi khách"));
        assert_ne!(classify("Thứ 6 này nên mặc gì đi đám cưới?").0, Intent::AddTask);
        assert_eq!(classify("thứ 6 nộp báo cáo").0, Intent::AddTask);
        // nói tự nhiên, lệnh giữa câu
        let w = |t: &str| write_intent(t).map(|(i, c)| (i, c));
        assert_eq!(w("bạn thêm giúp mình task gửi báo giá cho anh Tuấn trước thứ 6 nhé"), Some((Intent::AddTask, "gửi báo giá cho anh Tuấn trước thứ 6".into())));
        assert_eq!(w("thêm vào danh sách việc: mai 9h họp agency"), Some((Intent::AddTask, "mai 9h họp agency".into())));
        assert_eq!(w("tạo cho mình 1 task gọi lại khách lúc 3h chiều"), Some((Intent::AddTask, "gọi lại khách lúc 3h chiều".into())));
        assert_eq!(w("nhớ nhắc mình uống thuốc 8h tối nha"), Some((Intent::AddTask, "uống thuốc 8h tối".into())));
        assert_eq!(w("Asi ơi lưu vào ghi chú: wifi nhà là abc123"), Some((Intent::AddNote, "wifi nhà là abc123".into())));
        assert_eq!(w("ghi nhớ giúp mình số tài khoản công ty 0123"), Some((Intent::AddNote, "số tài khoản công ty 0123".into())));
        assert_eq!(w("note lại ý này: chạy thử TikTok Ads 2 tuần"), Some((Intent::AddNote, "chạy thử TikTok Ads 2 tuần".into())));
        assert_eq!(w("please add a task to call John tomorrow 3pm"), Some((Intent::AddTask, "call John tomorrow 3pm".into())));
        assert_eq!(w("bạn thêm việc đi chợ chiều nay được không?"), Some((Intent::AddTask, "đi chợ chiều nay".into())));
        assert_eq!(w("lưu thành việc: gửi hợp đồng"), Some((Intent::AddTask, "gửi hợp đồng".into())));
        assert_eq!(w("hôm nay có việc gì không"), None);
        assert_eq!(w("làm sao để ghi chú nhanh trên mac"), None);
        assert_eq!(w("việc gì quan trọng nhất tuần này?"), None);
        assert_eq!(w("viết giúp mình email xin nghỉ phép"), None);
        assert!(claims_done("Mình đã thêm việc “Gửi báo giá” cho bạn rồi nhé!"));
        assert!(!claims_done("Bạn có 3 việc hôm nay."));
        assert!(explicit_command("Lưu ý gì khi đi Đà Lạt").is_none(), "\"lưu ý\" không phải lệnh ghi chú");
    }

    #[test]
    fn write_actions_need_explicit_request() {
        assert!(user_asks_for("add_task", "nhắc mình mai 8h nộp hồ sơ"));
        assert!(user_asks_for("add_task", "Thêm việc mua sữa"));
        assert!(!user_asks_for("add_task", "Mai thời tiết Hà Nội thế nào?"));
        assert!(user_asks_for("add_note", "lưu lại đoạn này vào ghi chú"));
        assert!(!user_asks_for("add_note", "Viết giúp mình email xin nghỉ phép"));
        assert!(user_asks_for("show_today", "bất kỳ"));
    }

    #[test]
    fn explicit_commands_only_at_start() {
        assert_eq!(explicit_command("Asi ơi nhắc tôi mai 9h họp").map(|x| x.1), Some("mai 9h họp".to_string()));
        assert!(explicit_command("thứ 6 nộp báo cáo").is_none(), "có giờ hẹn nhưng không có từ lệnh");
        assert!(explicit_command("bạn có thể giải thích vì sao người ta hay nói nhắc tôi đi không").is_none());
        assert_eq!(explicit_command("ghi chú: wifi 123").map(|x| x.0), Some(Intent::AddNote));
    }

    #[test]
    fn sysinfo_reads() {
        let s = sysinfo();
        assert!(s.cores >= 1);
    }
}
