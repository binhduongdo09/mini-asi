//! Trợ lý Asi: hiểu câu nói (assistant::classify) → trả lời, thẻ, nút hành động.

#[allow(unused_imports)]
use crate::prelude::*;

// ───────────────────────── Trợ lý Asi (chat + giọng nói) ─────────────────────────

pub(crate) fn looks_english(text: &str, voice_lang: &str) -> bool {
    let has_vi = text.chars().any(|c| !c.is_ascii() && c.is_alphabetic());
    if has_vi {
        return false;
    }
    let f = format!(" {} ", text.to_lowercase());
    // từ "mạnh" chắc chắn là tiếng Anh; từ "yếu" (the, my, is…) trùng với tiếng Việt không dấu nên cần ≥ 2
    let strong = [" how ", " what ", " clean", " remind ", " show ", " task", " today", " please ", " any ", " overdue", " battery", " storage", " status", " schedule", " what's "];
    let weak = [" the ", " my ", " is ", " are ", " me ", " i ", " you "];
    if text.is_ascii() && !voice_lang.starts_with("en") {
        return strong.iter().any(|w| f.contains(*w)) || weak.iter().filter(|w| f.contains(*w)).count() >= 2;
    }
    voice_lang.starts_with("en") || strong.iter().any(|w| f.contains(*w))
}

pub(crate) fn fmt_gb(b: u64) -> String {
    asi_clean::disk::human(b)
}

pub(crate) fn greet_word(en: bool) -> &'static str {
    let h = Local::now().hour();
    match (en, h) {
        (true, 5..=11) => "Good morning",
        (true, 12..=17) => "Good afternoon",
        (true, _) => "Good evening",
        (false, 5..=10) => "Chào buổi sáng",
        (false, 11..=13) => "Chào buổi trưa",
        (false, 14..=17) => "Chào buổi chiều",
        (false, _) => "Chào buổi tối",
    }
}

/// Kiểm tra trạng thái Apple Intelligence (FoundationModels, chạy trên máy)
#[tauri::command]
pub(crate) async fn apple_ai_status(st: S<'_>) -> Result<apple_ai::AppleAiStatus, String> {
    let dir = st.data_dir.clone();
    blocking(move || Ok(apple_ai::status(&dir))).await
}

/// Xoá hội thoại gần đây (nút "Cuộc trò chuyện mới")
#[tauri::command]
pub(crate) fn asi_reset(st: S) {
    st.chat_log.lock().unwrap().clear();
}

fn local_fmt(ms: i64, f: &str) -> String {
    chrono::DateTime::from_timestamp_millis(ms).map(|d| d.with_timezone(&Local).format(f).to_string()).unwrap_or_default()
}
fn clip_chars(s: &str, n: usize) -> String {
    let t: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.chars().count() > n { t.chars().take(n).collect::<String>() + "…" } else { t }
}

/// Ngữ cảnh thật trên máy (gọn, vì mô hình trên máy chỉ đọc được ~4.000 token)
pub(crate) fn build_ai_context(st: &AppState) -> String {
    use chrono::TimeZone;
    let now = now_ms();
    let lnow = Local::now();
    let wd = ["Chủ nhật", "Thứ 2", "Thứ 3", "Thứ 4", "Thứ 5", "Thứ 6", "Thứ 7"][lnow.weekday().num_days_from_sunday() as usize];
    let end_today = {
        let d = lnow.date_naive().and_hms_opt(23, 59, 59).unwrap();
        Local.from_local_datetime(&d).earliest().map(|x| x.timestamp_millis()).unwrap_or(now)
    };
    let mut out = vec![format!("Now: {wd}, {}", lnow.format("%d/%m/%Y %H:%M"))];
    {
        let s = st.store.lock().unwrap();
        let mut open: Vec<&store::Task> = s.tasks.iter().filter(|t| t.done_at.is_none()).collect();
        open.sort_by_key(|t| t.due.unwrap_or(i64::MAX));
        let lines: Vec<String> = open
            .iter()
            .take(10)
            .map(|t| match t.due {
                Some(d) if d < now => format!("- {} (OVERDUE since {})", clip_chars(&t.title, 80), local_fmt(d, "%H:%M %d/%m")),
                Some(d) if d <= end_today => format!("- {} (due today {})", clip_chars(&t.title, 80), local_fmt(d, "%H:%M")),
                Some(d) => format!("- {} (due {})", clip_chars(&t.title, 80), local_fmt(d, "%a %d/%m %H:%M")),
                None => format!("- {} (no due time)", clip_chars(&t.title, 80)),
            })
            .collect();
        out.push(format!("Open tasks ({} total):\n{}", open.len(), if lines.is_empty() { "- none".into() } else { lines.join("\n") }));
        let notes: Vec<String> = s.notes.iter().filter(|n| !n.text.trim().is_empty()).take(3).map(|n| format!("- {}", clip_chars(&n.text, 100))).collect();
        if !notes.is_empty() {
            out.push(format!("Recent notes:\n{}", notes.join("\n")));
        }
    }
    let evs: Vec<String> = st.events.lock().unwrap().iter().filter(|e| e.end > now && e.start <= end_today).take(6).map(|e| format!("- {} {}", if e.all_day { "all day".into() } else { local_fmt(e.start, "%H:%M") }, clip_chars(&e.title, 70))).collect();
    if !evs.is_empty() {
        out.push(format!("Calendar today:\n{}", evs.join("\n")));
    }
    if let Some(c) = st.clips.lock().ok().and_then(|c| c.items.first().cloned()) {
        out.push(format!("Last copied: {}", if c.text.trim().is_empty() { "an image or file".to_string() } else { clip_chars(&c.text, 400) }));
    }
    let sys = assistant::sysinfo();
    let mut m = vec![];
    if let Some(d) = asi_clean::disk::status(&home_dir()) {
        m.push(format!("disk {} free ({:.0}%)", fmt_gb(d.free_bytes), d.free_percent));
    }
    if let Some(b) = sys.battery_pct {
        m.push(format!("battery {b}%{}", if sys.charging.unwrap_or(false) { " charging" } else { "" }));
    }
    if let Some(r) = sys.mem_used_pct {
        m.push(format!("RAM {r:.0}% used"));
    }
    if !m.is_empty() {
        out.push(format!("Mac: {}", m.join(", ")));
    }
    out.join("\n")
}

/// Chỉ dẫn cho "bộ não" AI: vai trò, quy tắc, cách chọn hành động, ngữ cảnh máy (`with_context = false` → không gửi dữ liệu cá nhân)
pub(crate) fn ai_instructions(st: &AppState, en: bool, with_context: bool) -> String {
    let lang = if en { "English" } else { "Vietnamese (always with full diacritics, natural and friendly, like a capable personal assistant; address the user as \"bạn\", refer to yourself as \"Asi\" or \"mình\")" };
    format!(
        "You are Asi, the user's personal assistant living in the MacBook notch (Mini-Asi app). The user is Vietnamese.\n\
         Help with anything a smart assistant can: answer general questions, explain, advise, write and rewrite (emails, messages, posts), translate, summarize, plan the day, brainstorm, calculate, and answer questions about the user's own tasks, calendar, notes and Mac using the context below.\n\
         Rules:\n\
         - Reply in {lang}.\n\
         - Be direct and useful. Usually 1-4 sentences; use short '- ' bullet lines for steps or options; for writing tasks give the full text (up to about 12 lines).\n\
         - Use the conversation so far to understand follow-ups (\"nó\", \"cái đó\", \"dịch lại\", \"ngắn hơn\").\n\
         - Only state facts about the user's tasks, events, notes or Mac that appear in the context. Never invent them. If you are not sure about something, say so briefly.\n\
         - No greetings unless greeted, no disclaimers about being an AI.\n\
         Most messages are questions or requests for text: answer them fully in reply and set action to none.\n\
         Only choose another action when the user explicitly asks the app to DO it (\"nhắc tôi\", \"thêm việc\", \"ghi chú lại\", \"bật tập trung\"…):\n\
         - add_task: wants a reminder or to-do. argument = the task in the user's words including time words (e.g. \"mai 9h gọi khách\").\n\
         - add_note: wants to save something as a note. argument = the note text.\n\
         - start_focus: wants a focus / pomodoro timer. argument = minutes as digits (default 25).\n\
         - show_today: asks about today's plan or agenda. show_overdue: asks which tasks are late.\n\
         - check_mac: asks about the computer's health, speed, battery, memory or storage. clean_junk: wants to free disk space.\n\
         - open_tab: wants to open a screen. argument = one of tasks, notes, clips, journal, words, threads, clean.\n\
         When you choose an action, reply with one short sentence; the app shows the details.\n\
         Context right now:\n{}",
        if with_context { build_ai_context(st) } else { format!("Now: {}", Local::now().format("%A %d/%m/%Y %H:%M")) }
    )
}

/// Lịch sử gần đây (≤ 6 lượt, trong 30 phút) + câu hỏi hiện tại
pub(crate) fn ai_prompt(st: &AppState, text: &str) -> String {
    let now = now_ms();
    let log = st.chat_log.lock().unwrap();
    let recent: Vec<&(String, String, i64)> = log.iter().rev().take_while(|(_, _, t)| now - t < 30 * 60_000).take(6).collect();
    let mut p = String::new();
    if !recent.is_empty() {
        p.push_str("Conversation so far:\n");
        for (q, a, _) in recent.into_iter().rev() {
            p.push_str(&format!("User: {}\nAsi: {}\n", clip_chars(q, 400), clip_chars(a, 500)));
        }
        p.push('\n');
    }
    p.push_str(&format!("User: {text}"));
    p
}

fn remember(st: &AppState, q: &str, a: &str) {
    let mut log = st.chat_log.lock().unwrap();
    log.push((q.to_string(), a.to_string(), now_ms()));
    let n = log.len();
    if n > 12 {
        log.drain(..n - 12);
    }
}

/// Câu ngắn để đọc to: 1–2 câu đầu, bỏ gạch đầu dòng
pub(crate) fn speak_summary(text: &str) -> String {
    let flat = text.lines().map(|l| l.trim().trim_start_matches(['-', '•', '*']).trim()).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ");
    let mut out = String::new();
    for (i, sent) in flat.split_inclusive(['.', '!', '?']).enumerate() {
        if i >= 2 || out.chars().count() + sent.chars().count() > 240 {
            break;
        }
        out.push_str(sent);
    }
    if out.trim().is_empty() { flat.chars().take(200).collect() } else { out.trim().to_string() }
}

pub(crate) fn reply_status(st: &AppState, en: bool) -> assistant::Reply {
    use assistant::{Card, Intent, Metric, Reply};
    let sys = assistant::sysinfo();
    let disk = asi_clean::disk::status(&home_dir());
    let stat = build_status(st);
    let mut items = vec![];
    let mut problems: Vec<String> = vec![];
    let mut r = Reply::new(Intent::Status, "");
    if let Some(d) = &disk {
        let lvl = match d.level {
            asi_clean::disk::DiskLevel::Ok => 0,
            asi_clean::disk::DiskLevel::Warn => 1,
            asi_clean::disk::DiskLevel::Critical => 2,
        };
        items.push(Metric { label: if en { "Disk free" } else { "Ổ đĩa trống" }.into(), value: format!("{} ({:.0}%)", fmt_gb(d.free_bytes), d.free_percent), level: lvl, ratio: Some(1.0 - d.free_percent / 100.0) });
        if lvl > 0 {
            problems.push(if en { format!("disk is almost full ({:.0}% free)", d.free_percent) } else { format!("ổ đĩa sắp đầy (còn {:.0}%)", d.free_percent) });
            r = r.act(if en { "Clean junk" } else { "Dọn rác ngay" }, "ask", "dọn rác");
        }
    }
    if let Some(l) = sys.cpu_load {
        let lvl = if l > 1.2 { 2 } else if l > 0.85 { 1 } else { 0 };
        items.push(Metric { label: "CPU".into(), value: format!("{:.0}%", (l * 100.0).min(400.0)), level: lvl, ratio: Some(l.min(1.0)) });
        if lvl > 0 {
            problems.push(if en { "CPU is working hard — some app may be heavy".into() } else { "CPU đang chạy nặng — có app ngốn tài nguyên".into() });
        }
    }
    if let (Some(m), Some(t)) = (sys.mem_used_pct, sys.mem_total_gb) {
        let lvl = if m > 92.0 { 2 } else if m > 82.0 { 1 } else { 0 };
        items.push(Metric { label: "RAM".into(), value: format!("{:.0}% / {:.0} GB", m, t), level: lvl, ratio: Some(m / 100.0) });
        if lvl > 0 {
            problems.push(if en { "memory is nearly full — close unused apps".into() } else { "RAM gần đầy — nên đóng bớt app không dùng".into() });
        }
    }
    if let Some(b) = sys.battery_pct {
        let ch = sys.charging.unwrap_or(false);
        let lvl = if b < 10 && !ch { 2 } else if b < 20 && !ch { 1 } else { 0 };
        items.push(Metric { label: if en { "Battery" } else { "Pin" }.into(), value: format!("{b}%{}", if ch { " ⚡" } else { "" }), level: lvl, ratio: Some(b as f64 / 100.0) });
        if lvl > 0 {
            problems.push(if en { format!("battery low ({b}%)") } else { format!("pin yếu ({b}%) — cắm sạc nhé") });
        }
    }
    if let Some(u) = sys.uptime_days {
        let lvl = if u > 14.0 { 1 } else { 0 };
        items.push(Metric { label: if en { "Uptime" } else { "Đã bật" }.into(), value: if en { format!("{u:.1} days") } else { format!("{u:.1} ngày") }, level: lvl, ratio: None });
        if lvl > 0 {
            problems.push(if en { "it's been over 2 weeks without a restart".into() } else { "đã hơn 2 tuần chưa khởi động lại máy".into() });
        }
    }
    if stat.lateness.overdue > 0 {
        problems.push(if en { format!("{} overdue task(s)", stat.lateness.overdue) } else { format!("{} việc đang trễ", stat.lateness.overdue) });
        r = r.act(if en { "Show overdue" } else { "Xem việc trễ" }, "ask", "việc nào trễ");
    }
    if stat.session_minutes >= stat.break_minutes {
        problems.push(if en { format!("you've been on the computer {} min straight", stat.session_minutes) } else { format!("bạn đã dùng máy liên tục {} phút", stat.session_minutes) });
        r = r.act(if en { "Take a break" } else { "Nghỉ mắt 20 giây" }, "rest", "");
    }
    r.cards.push(Card::Metrics { items });
    if problems.is_empty() {
        r.text = if en { "Your Mac looks healthy ✨ Nothing to worry about.".into() } else { "Máy đang khoẻ re ✨ Không có gì đáng lo.".into() };
        r.speak = if en { "Your Mac looks healthy.".into() } else { "Máy đang khoẻ, không có gì đáng lo.".into() };
    } else {
        r.mood = "worried".into();
        r.text = if en { format!("I noticed {} thing(s): {}.", problems.len(), problems.join("; ")) } else { format!("Asi thấy {} điều cần để ý: {}.", problems.len(), problems.join("; ")) };
        r.speak = r.text.clone();
    }
    r
}

pub(crate) fn reply_clean(st: &AppState, en: bool) -> Result<assistant::Reply, String> {
    use assistant::{Card, Intent, Reply};
    let e = engine(st)?;
    let rep = e.scan(vec![]);
    let safe = rep.safe_bytes;
    let top: Vec<String> = rep.rules.iter().filter(|r| !r.locked && r.level == asi_clean::rules::Level::Safe && r.bytes > 0).take(4).map(|r| format!("{} · {}", r.name, fmt_gb(r.bytes))).collect();
    *st.last_scan.lock().unwrap() = Some(rep.clone());
    let mut r = if safe == 0 {
        Reply::new(Intent::Clean, if en { "Your Mac is already clean 🧼" } else { "Máy sạch bong rồi 🧼 Không có rác an toàn để dọn." })
    } else {
        let mut r = Reply::new(
            Intent::Clean,
            if en { format!("Found {} of safe junk (caches, logs…). Clean it now?", fmt_gb(safe)) } else { format!("Asi tìm thấy {} rác an toàn (cache, log…). Dọn luôn nhé?", fmt_gb(safe)) },
        );
        r.cards.push(Card::Text { text: top.join("\n") });
        r.mood = "focus".into();
        r = r.act(if en { "Clean now" } else { "Dọn ngay" }, "clean_safe", "");
        r
    };
    if rep.locked_bytes > 0 {
        r.cards.push(Card::Text { text: if en { format!("✨ Pro can free another {}", fmt_gb(rep.locked_bytes)) } else { format!("✨ Bản Pro dọn thêm được {}", fmt_gb(rep.locked_bytes)) } });
    }
    Ok(r.act(if en { "See details" } else { "Xem chi tiết" }, "open_tab", "clean"))
}

/// Trả lời bằng luật (offline, tức thì) theo ý định đã nhận ra
fn rule_reply(app2: &AppHandle, st: &AppState, intent: assistant::Intent, rest: String, text: &str, en: bool, offset: Option<i32>) -> Result<assistant::Reply, String> {
    use assistant::{Card, Intent, Reply, TaskBrief};
    let now = now_ms();
    let (lnow, off) = local_now(offset);
    let r = match intent {
        Intent::Status => reply_status(st, en),
        Intent::Clean => reply_clean(st, en)?,
        Intent::Overdue => {
            let mut items: Vec<TaskBrief> = st.store.lock().unwrap().tasks.iter().filter(|t| t.done_at.is_none() && t.due.is_some_and(|d| d < now)).map(|t| TaskBrief { id: t.id, title: t.title.clone(), due: t.due }).collect();
            items.sort_by_key(|t| t.due);
            let n = items.len();
            let mut r = if n == 0 {
                Reply::new(intent, if en { "No overdue tasks 🎉 Great job!" } else { "Không có việc nào trễ cả 🎉 Giỏi quá!" })
            } else {
                let mut r = Reply::new(intent, if en { format!("{n} task(s) are overdue. Oldest: “{}”.", items[0].title) } else { format!("Có {n} việc đang trễ. Lâu nhất là “{}”.", items[0].title) });
                r.mood = "angry".into();
                r
            };
            if n > 0 {
                r.cards.push(Card::Tasks { items });
            }
            r
        }
        Intent::Today => {
            let start = to_ms(lnow.date().and_hms_opt(0, 0, 0).unwrap(), off);
            let end = start + 86_400_000;
            let mut items: Vec<TaskBrief> = st.store.lock().unwrap().tasks.iter().filter(|t| t.done_at.is_none() && t.due.is_some_and(|d| d < end)).map(|t| TaskBrief { id: t.id, title: t.title.clone(), due: t.due }).collect();
            items.sort_by_key(|t| t.due);
            let evs: Vec<(String, i64, bool)> = st.events.lock().unwrap().iter().filter(|e| e.start < end && e.end > now).map(|e| (e.title.clone(), e.start, e.all_day)).collect();
            let mut r = Reply::new(
                intent,
                if en { format!("Today: {} task(s) and {} event(s).", items.len(), evs.len()) } else { format!("Hôm nay bạn có {} việc và {} lịch hẹn.", items.len(), evs.len()) },
            );
            if !evs.is_empty() {
                r.cards.push(Card::Events { items: evs });
            }
            if !items.is_empty() {
                r.cards.push(Card::Tasks { items });
            }
            if r.cards.is_empty() {
                r.text = if en { "Nothing scheduled today — enjoy! 🌿".into() } else { "Hôm nay thảnh thơi, không có lịch gì 🌿".into() };
                r.speak = r.text.clone();
            }
            r
        }
        Intent::AddTask => {
            let t = add_task(app2.clone(), app2.state::<Arc<AppState>>(), rest.clone(), offset)?;
            let when = t.due.map(|d| chrono::DateTime::from_timestamp_millis(d + off).map(|x| x.format("%H:%M %d/%m").to_string()).unwrap_or_default());
            let mut r = Reply::new(intent, match (&when, en) {
                (Some(w), true) => format!("Added “{}” — I'll remind you at {w}.", t.title),
                (Some(w), false) => format!("Đã thêm “{}” — Asi sẽ nhắc lúc {w}.", t.title),
                (None, true) => format!("Added “{}”.", t.title),
                (None, false) => format!("Đã thêm việc “{}”.", t.title),
            });
            r.cards.push(Card::Tasks { items: vec![TaskBrief { id: t.id, title: t.title, due: t.due }] });
            r.act(if en { "Open tasks" } else { "Xem danh sách việc" }, "open_tab", "tasks").act(if en { "Undo" } else { "Hoàn tác" }, "undo_task", &t.id.to_string())
        }
        Intent::AddNote => {
            if rest.trim().is_empty() {
                Reply::new(intent, if en { "What should I note down?" } else { "Bạn muốn Asi ghi lại điều gì?" }).act(if en { "Open notes" } else { "Mở Ghi chú" }, "open_tab", "notes")
            } else {
                st.store.lock().unwrap().upsert_note(None, rest.clone());
                kick_sync(&app2);
                Reply::new(intent, if en { "Noted ✍️" } else { "Asi ghi lại rồi ✍️" }).act(if en { "Open notes" } else { "Mở Ghi chú" }, "open_tab", "notes")
            }
        }
        Intent::Focus => {
            let m: i64 = text.split(|c: char| !c.is_ascii_digit()).find_map(|x| x.parse().ok()).unwrap_or(25).clamp(5, 180);
            st.health.lock().unwrap().focus_until = Some(Local::now().timestamp() + m * 60);
            let mut r = Reply::new(intent, if en { format!("Focus mode for {m} minutes. I'll keep quiet 🤫") } else { format!("Bắt đầu tập trung {m} phút. Asi sẽ giữ im lặng 🤫") });
            r.mood = "focus".into();
            r
        }
        Intent::Break => Reply::new(intent, if en { "Let's rest your eyes for 20 seconds 👀" } else { "Cùng nghỉ mắt 20 giây nhé 👀" }).act(if en { "Start" } else { "Bắt đầu nghỉ" }, "rest", ""),
        Intent::Disk => match asi_clean::disk::status(&home_dir()) {
            Some(d) => Reply::new(intent, if en { format!("You have {} free of {} ({:.0}%).", fmt_gb(d.free_bytes), fmt_gb(d.total_bytes), d.free_percent) } else { format!("Ổ đĩa còn trống {} / {} ({:.0}%).", fmt_gb(d.free_bytes), fmt_gb(d.total_bytes), d.free_percent) }).act(if en { "Clean junk" } else { "Dọn rác" }, "ask", "dọn rác"),
            None => Reply::new(intent, "Asi chưa đọc được ổ đĩa"),
        },
        Intent::Battery => match assistant::sysinfo().battery_pct {
            Some(b) => Reply::new(intent, if en { format!("Battery at {b}%.") } else { format!("Pin còn {b}%.") }),
            None => Reply::new(intent, if en { "This computer has no battery info." } else { "Máy này không có thông tin pin." }),
        },
        Intent::LastClip => {
            let c = st.clips.lock().unwrap().items.first().cloned();
            match c {
                Some(c) => {
                    let preview = if c.text.is_empty() { format!("{} file / ảnh", c.files.len().max(1)) } else { c.text.chars().take(160).collect() };
                    let mut r = Reply::new(intent, if en { "Your latest copy:" } else { "Mục bạn vừa chép:" });
                    r.speak = if en { "Here's what you copied last.".into() } else { "Đây là mục bạn vừa chép.".into() };
                    r.cards.push(Card::Text { text: preview });
                    r.act(if en { "Copy again" } else { "Chép lại" }, "clip_copy", &c.id.to_string()).act("Clipboard", "open_tab", "clips")
                }
                None => Reply::new(intent, if en { "Clipboard history is empty." } else { "Chưa có gì trong lịch sử clipboard." }),
            }
        }
        Intent::Journal => Reply::new(intent, if en { "Let's write today's journal 📔" } else { "Cùng viết nhật ký hôm nay nhé 📔" }).act(if en { "Open journal" } else { "Mở Nhật ký" }, "open_tab", "journal"),
        Intent::Threads => Reply::new(intent, if en { "Opening Threads…" } else { "Mở Threads cho bạn nè…" }).act("Threads", "open_tab", "threads"),
        Intent::Time => {
            let n = Local::now();
            let wd = ["Chủ nhật", "Thứ 2", "Thứ 3", "Thứ 4", "Thứ 5", "Thứ 6", "Thứ 7"][n.weekday().num_days_from_sunday() as usize];
            Reply::new(intent, if en { format!("It's {}.", n.format("%H:%M, %A %d %B")) } else { format!("Bây giờ là {}, {} ngày {}.", n.format("%H:%M"), wd, n.format("%d/%m/%Y")) })
        }
        Intent::Help => assistant::help(en),
        Intent::Greet => {
            let s = build_status(st);
            let extra = if s.lateness.overdue > 0 {
                if en { format!(" You have {} overdue task(s).", s.lateness.overdue) } else { format!(" Bạn đang có {} việc trễ đó nha.", s.lateness.overdue) }
            } else {
                String::new()
            };
            Reply::new(intent, format!("{} 👋 {}{}", greet_word(en), if en { "How can I help?" } else { "Asi giúp gì được cho bạn?" }, extra))
        }
        Intent::Thanks => Reply::new(intent, if en { "Anytime 💖" } else { "Không có gì đâu 💖" }),
        Intent::Vocab => {
            let words = st.vocab.lock().unwrap().batch(st.store.lock().unwrap().settings.vocab_count.clamp(1, 5) as usize, false, now);
            Reply::new(intent, if en { format!("Today's words: {}.", words.join(", ")) } else { format!("Từ mới hôm nay: {}. Mở tab Từ mới để nghe & đọc thử nhé!", words.join(", ")) })
                .act(if en { "Practice" } else { "Luyện phát âm" }, "open_tab", "words")
        }
        Intent::Unknown => {
            let mut r = assistant::help(en);
            r.intent = Intent::Unknown;
            r.mood = "idle".into();
            r.text = if en { "Hmm, I didn't get that. Try one of these:".into() } else { "Hmm, Asi chưa hiểu ý bạn. Thử một trong mấy câu này nhé:".into() };
            r.speak = if en { "Sorry, I didn't get that.".into() } else { "Xin lỗi, Asi chưa hiểu ý bạn.".into() };
            if !rest.trim().is_empty() {
                // Không hiểu → cho bấm để biến ngay thành việc / ghi chú
                r.text = if en { "Hmm, I didn't get that. Save it as a task or a note?".into() } else { "Hmm, Asi chưa hiểu lắm. Lưu câu này thành việc hay ghi chú?".into() };
                r.actions.insert(0, assistant::Action { label: if en { "Add as task".into() } else { "Thêm thành việc".into() }, cmd: "add_task".into(), arg: rest.clone() });
                r.actions.insert(1, assistant::Action { label: if en { "Save note".into() } else { "Lưu ghi chú".into() }, cmd: "add_note".into(), arg: rest.clone() });
            }
            r
        }
    };
    Ok(r)
}

/// Bộ não đang chọn: (nhà cung cấp, nhãn hiển thị)
fn brain(st: &AppState) -> (Option<llm::Provider>, String) {
    let s = st.store.lock().unwrap();
    match llm::Provider::from_id(&s.settings.ai_provider) {
        Some(p) => {
            let m = model_of(&s.settings, p);
            (Some(p), format!("{} · {m}", p.name()))
        }
        None => (None, "Apple Intelligence".into()),
    }
}

pub(crate) fn model_of(set: &store::Settings, p: llm::Provider) -> String {
    let m = match p {
        llm::Provider::OpenAi => &set.ai_model_openai,
        llm::Provider::Gemini => &set.ai_model_gemini,
        llm::Provider::Claude => &set.ai_model_claude,
    };
    if m.trim().is_empty() { p.default_model().to_string() } else { m.trim().to_string() }
}

/// Hỏi bộ não đã chọn (Apple Intelligence trên máy hoặc API đám mây). `allow_actions = false` → chỉ trả lời.
fn ask_brain(st: &AppState, text: &str, en: bool, allow_actions: bool) -> Result<apple_ai::AiTurn, String> {
    let (prov, _) = brain(st);
    match prov {
        None => apple_ai::ask(&st.data_dir, &ai_instructions(st, en, true), &ai_prompt(st, text), allow_actions),
        Some(p) => {
            let (model, ctx) = {
                let s = st.store.lock().unwrap();
                (model_of(&s.settings, p), s.settings.ai_cloud_context)
            };
            let key = threads::secret::get(&p.key_name(), &st.data_dir).filter(|k| !k.trim().is_empty()).ok_or_else(|| format!("Chưa nhập API key {} — vào Cài đặt › Asi & giọng nói", p.name()))?;
            let system = ai_instructions(st, en, ctx) + &llm::json_rules(allow_actions);
            let raw = llm::ask(p, key.trim(), &model, &system, &ai_prompt(st, text))?;
            let mut t = llm::parse_turn(&raw);
            if !allow_actions {
                t.action = "none".into();
            }
            Ok(t)
        }
    }
}

/// Dựng câu trả lời từ AI: chữ của mô hình + thẻ/nút thật của app theo hành động mô hình chọn
fn reply_from_ai(app2: &AppHandle, st: &AppState, t: apple_ai::AiTurn, text: &str, en: bool, offset: Option<i32>) -> Result<assistant::Reply, String> {
    use assistant::{Intent, Reply};
    let arg = if t.argument.is_empty() { text.to_string() } else { t.argument.clone() };
    let with_ai_text = |mut r: Reply| {
        if !t.reply.is_empty() {
            r.text = t.reply.clone();
            r.speak = speak_summary(&t.reply);
        }
        r
    };
    let r = match t.action.as_str() {
        // Việc làm thay đổi dữ liệu: dùng đúng đường của lệnh (có nút Hoàn tác), câu xác nhận do app viết
        "add_task" => rule_reply(app2, st, Intent::AddTask, arg, text, en, offset)?,
        "add_note" => rule_reply(app2, st, Intent::AddNote, arg, text, en, offset)?,
        "start_focus" => rule_reply(app2, st, Intent::Focus, String::new(), &format!("{arg} {text}"), en, offset)?,
        // Dọn rác: con số phải chính xác → dùng câu của app (vẫn cần bấm "Dọn ngay")
        "clean_junk" => rule_reply(app2, st, Intent::Clean, String::new(), text, en, offset)?,
        // Xem thông tin: lời của mô hình (đã có ngữ cảnh) + thẻ số liệu thật của app
        "show_today" => with_ai_text(rule_reply(app2, st, Intent::Today, String::new(), text, en, offset)?),
        "show_overdue" => with_ai_text(rule_reply(app2, st, Intent::Overdue, String::new(), text, en, offset)?),
        "check_mac" => with_ai_text(reply_status(st, en)),
        "open_tab" => {
            let tab = ["tasks", "notes", "clips", "journal", "words", "threads", "clean"].into_iter().find(|x| arg.to_lowercase().contains(x)).unwrap_or("tasks");
            let mut r = Reply::new(Intent::Help, if t.reply.is_empty() { (if en { "Here you go." } else { "Mở cho bạn nè." }).to_string() } else { t.reply.clone() });
            r.speak = speak_summary(&r.text);
            r.act(if en { "Open" } else { "Mở" }, "open_tab", tab)
        }
        _ => {
            let mut r = Reply::new(Intent::Unknown, t.reply.clone());
            r.speak = speak_summary(&t.reply);
            r.mood = "happy".into();
            if t.reply.chars().count() > 160 {
                r = r.act(if en { "Copy" } else { "Chép" }, "copy_text", &t.reply).act(if en { "Save as note" } else { "Lưu ghi chú" }, "add_note", &t.reply);
            }
            r
        }
    };
    Ok(r)
}

/// Hỏi Asi. Lệnh ngắn, rõ ràng → luật (tức thì, offline). Còn lại → bộ não AI đã chọn (Apple Intelligence / ChatGPT /
/// Gemini / Claude) để hiểu rộng và trả lời như trợ lý. Mô hình chỉ được thêm việc / ghi chú / bật tập trung khi người
/// dùng thật sự nhờ (`assistant::user_asks_for`); chọn nhầm → hỏi lại ở chế độ chỉ trả lời.
/// AI lỗi → báo rõ lỗi, KHÔNG tự thêm việc / ghi chú.
#[tauri::command]
pub(crate) async fn asi_ask(app: AppHandle, st: S<'_>, text: String, offset: Option<i32>) -> Result<assistant::Reply, String> {
    use assistant::{Intent, Reply};
    let st2 = st.inner().clone();
    let app2 = app.clone();
    blocking(move || {
        let st = &st2;
        let text = text.trim().to_string();
        let (lang, ai_on) = {
            let s = st.store.lock().unwrap();
            (s.settings.voice_lang.clone(), s.settings.apple_ai_enabled)
        };
        let en = looks_english(&text, &lang);
        let explicit = assistant::explicit_command(&text);
        let (intent, rest) = assistant::classify(&text);
        let words = text.split_whitespace().count();
        let quick = explicit.is_some() || (!matches!(intent, Intent::Unknown | Intent::AddTask | Intent::Help) && words <= 4 && !assistant::looks_like_question(&text));
        let rules_label = if en { "Basic commands" } else { "Lệnh cơ bản" };
        if ai_on && !quick {
            let (_, label) = brain(st);
            let mut res = ask_brain(st, &text, en, true);
            if let Ok(t) = &res {
                if matches!(t.action.as_str(), "add_task" | "add_note" | "start_focus") && !assistant::user_asks_for(&t.action, &text) {
                    // mô hình định tự thêm việc / ghi chú trong khi người dùng chỉ hỏi → hỏi lại, chỉ trả lời
                    res = ask_brain(st, &text, en, false).or_else(|_| Ok(apple_ai::AiTurn { reply: t.reply.clone(), action: "none".into(), argument: String::new() }));
                }
            }
            match res {
                Ok(turn) if !(turn.reply.is_empty() && turn.action == "none") => {
                    let mut r = reply_from_ai(&app2, st, turn, &text, en, offset)?;
                    r.engine = label;
                    remember(st, &text, &r.text);
                    return Ok(r);
                }
                Ok(_) => {}
                Err(e) => {
                    eprintln!("{label}: {e}");
                    // Câu không phải lệnh rõ ràng → báo lỗi AI, để người dùng tự chọn lưu thành việc / ghi chú
                    if !matches!(intent, Intent::Status | Intent::Clean | Intent::Overdue | Intent::Today | Intent::Disk | Intent::Battery | Intent::Time | Intent::LastClip) {
                        let mut r = Reply::new(Intent::Unknown, if en { format!("I couldn't reach {label}: {e}") } else { format!("Asi chưa hỏi được {label}: {e}") });
                        r.mood = "worried".into();
                        r.speak = if en { "Sorry, the AI isn't available right now.".into() } else { "Xin lỗi, trợ lý AI chưa trả lời được.".into() };
                        r.engine = rules_label.into();
                        let r = r
                            .act(if en { "AI settings" } else { "Cài đặt AI" }, "open_tab", "ai")
                            .act(if en { "Save as task" } else { "Lưu thành việc" }, "add_task", &text)
                            .act(if en { "Save note" } else { "Lưu ghi chú" }, "add_note", &text);
                        return Ok(r);
                    }
                }
            }
        }
        let mut r = rule_reply(&app2, st, intent, rest, &text, en, offset)?;
        r.engine = rules_label.into();
        remember(st, &text, &r.text);
        Ok(r)
    })
    .await
}

#[derive(Serialize)]
pub(crate) struct AiProviderInfo {
    id: String,
    name: String,
    has_key: bool,
    model: String,
    default_model: String,
    key_url: String,
}

/// Thông tin bộ não AI cho Cài đặt (không bao giờ trả key về giao diện)
#[tauri::command]
pub(crate) fn ai_info(st: S) -> Vec<AiProviderInfo> {
    let set = st.store.lock().unwrap().settings.clone();
    llm::ALL
        .iter()
        .map(|&p| AiProviderInfo {
            id: p.id().into(),
            name: p.name().into(),
            has_key: threads::secret::get(&p.key_name(), &st.data_dir).is_some_and(|k| !k.trim().is_empty()),
            model: model_of(&set, p),
            default_model: p.default_model().into(),
            key_url: p.key_url().into(),
        })
        .collect()
}

/// Lưu API key vào Keychain (rỗng = xoá)
#[tauri::command]
pub(crate) fn ai_set_key(st: S, provider: String, key: String) -> Result<(), String> {
    let p = llm::Provider::from_id(&provider).ok_or("Nhà cung cấp lạ")?;
    let k = key.trim();
    if k.is_empty() {
        threads::secret::delete(&p.key_name(), &st.data_dir);
        Ok(())
    } else {
        threads::secret::set(&p.key_name(), k, &st.data_dir)
    }
}

/// Kiểm tra key + chọn mô hình còn dùng được (tự đổi sang tên mới nhất nếu tên đang lưu đã cũ). Trả về tên mô hình.
#[tauri::command]
pub(crate) async fn ai_test(app: AppHandle, st: S<'_>, provider: String) -> Result<String, String> {
    let p = llm::Provider::from_id(&provider).ok_or("Nhà cung cấp lạ")?;
    let st2 = st.inner().clone();
    let model = blocking(move || {
        let key = threads::secret::get(&p.key_name(), &st2.data_dir).ok_or("Chưa nhập API key")?;
        let current = model_of(&st2.store.lock().unwrap().settings, p);
        let list = llm::list_models(p, key.trim())?;
        let model = llm::pick_model(p, &current, &list).unwrap_or(current);
        let raw = llm::ask(p, key.trim(), &model, &format!("Reply in Vietnamese.{}", llm::json_rules(false)), "Chào Asi, trả lời đúng một câu ngắn.")?;
        if llm::parse_turn(&raw).reply.is_empty() {
            return Err("Mô hình không trả lời".into());
        }
        let snap = {
            let mut s = st2.store.lock().unwrap();
            match p {
                llm::Provider::OpenAi => s.settings.ai_model_openai = model.clone(),
                llm::Provider::Gemini => s.settings.ai_model_gemini = model.clone(),
                llm::Provider::Claude => s.settings.ai_model_claude = model.clone(),
            }
            let _ = s.save_settings();
            s.settings.clone()
        };
        Ok::<_, String>((model, snap))
    })
    .await?;
    let _ = app.emit("settings-changed", &model.1);
    Ok(model.0)
}

/// Thực hiện hành động do Asi đề xuất (chỉ khi người dùng bấm nút)
#[tauri::command]
pub(crate) async fn asi_action(app: AppHandle, st: S<'_>, cmd: String, arg: String) -> Result<assistant::Reply, String> {
    use assistant::{Intent, Reply};
    let st2 = st.inner().clone();
    let app_outer = app.clone();
    let r = blocking(move || {
        let lang = st2.store.lock().unwrap().settings.voice_lang.clone();
        let en = lang.starts_with("en");
        match cmd.as_str() {
            "clean_safe" => {
                let e = engine(&st2)?;
                let rep = e.scan(vec![]);
                let rules: Vec<String> = rep.rules.iter().filter(|r| r.level == asi_clean::rules::Level::Safe && !r.locked && r.id != "system-trash").map(|r| r.id.clone()).collect();
                let res = e.clean(&rep, &asi_clean::clean::Selection { rules, paths: vec![] }, false);
                let mut r = Reply::new(Intent::Clean, if en { format!("Done! Freed {} 🎉", fmt_gb(res.deleted_bytes)) } else { format!("Xong! Đã giải phóng {} 🎉", fmt_gb(res.deleted_bytes)) });
                r.mood = "love".into();
                Ok(r)
            }
            "add_task" => {
                let t = add_task(app.clone(), app.state::<Arc<AppState>>(), arg.clone(), None)?;
                let mut r = Reply::new(Intent::AddTask, if en { format!("Added “{}” ✓", t.title) } else { format!("Đã thêm việc “{}” ✓", t.title) });
                r.cards.push(assistant::Card::Tasks { items: vec![assistant::TaskBrief { id: t.id, title: t.title, due: t.due }] });
                Ok(r.act(if en { "Open tasks" } else { "Xem danh sách việc" }, "open_tab", "tasks").act(if en { "Undo" } else { "Hoàn tác" }, "undo_task", &t.id.to_string()))
            }
            "undo_task" => {
                let id: u64 = arg.parse().map_err(|_| "id lạ")?;
                st2.store.lock().unwrap().delete_task(id);
                kick_sync(&app);
                Ok(Reply::new(Intent::AddTask, if en { "Okay, removed it." } else { "Đã bỏ việc vừa thêm." }))
            }
            "add_note" => {
                st2.store.lock().unwrap().upsert_note(None, arg.clone());
                kick_sync(&app);
                Ok(Reply::new(Intent::AddNote, if en { "Noted ✍️" } else { "Asi ghi lại rồi ✍️" }).act(if en { "Open notes" } else { "Mở Ghi chú" }, "open_tab", "notes"))
            }
            "clip_copy" => {
                let id: u64 = arg.parse().map_err(|_| "id lạ")?;
                let d = st2.clips.lock().unwrap().data(id).ok_or("Mục này không còn")?;
                notch_native::clip::write(&d)?;
                Ok(Reply::new(Intent::LastClip, if en { "Copied — press ⌘V to paste." } else { "Đã chép — bấm ⌘V để dán." }))
            }
            "copy_text" => {
                notch_native::clip::write(&notch_native::clip::ClipData::Text(arg.clone()))?;
                Ok(Reply::new(Intent::LastClip, if en { "Copied reply to clipboard — press ⌘V to paste." } else { "Đã chép câu trả lời vào Clipboard — nhấn ⌘V để dán." }))
            }
            _ => Err("Lệnh không hỗ trợ".into()),
        }
    })
    .await?;
    push_status(&app_outer);
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speak_summary_short_and_clean() {
        let t = "Đây là 3 bước:\n- Mở Cài đặt.\n- Chọn Pin.\n- Bật Chế độ nguồn điện thấp.";
        assert_eq!(speak_summary(t), "Đây là 3 bước: Mở Cài đặt. Chọn Pin.");
        let long = "Câu một rất dài. ".repeat(40);
        assert!(speak_summary(&long).chars().count() <= 240);
    }
}
