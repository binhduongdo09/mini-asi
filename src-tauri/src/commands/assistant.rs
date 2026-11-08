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

/// Kiểm tra trạng thái Apple Intelligence (FoundationModels On-Device) trên máy
#[tauri::command]
pub(crate) async fn apple_ai_status(st: S<'_>) -> Result<assistant::AppleAiStatus, String> {
    let dir = st.data_dir.clone();
    blocking(move || Ok(assistant::check_apple_ai(&dir))).await
}

/// Tạo bản tóm tắt ngữ cảnh thực tế trên máy để Apple Intelligence trả lời sát với dữ liệu của người dùng
pub(crate) fn build_ai_context(st: &AppState) -> String {
    let now = now_ms();
    let lnow = Local::now();
    let (tasks_summary, notes_summary) = {
        let s = st.store.lock().unwrap();
        let open: Vec<String> = s
            .tasks
            .iter()
            .filter(|t| t.done_at.is_none())
            .take(8)
            .map(|t| {
                let state = match t.due {
                    Some(d) if d < now => " [OVERDUE]",
                    Some(_) => " [scheduled]",
                    None => "",
                };
                format!("- {}{}", t.title, state)
            })
            .collect();
        let notes: Vec<String> = s
            .notes
            .iter()
            .filter(|n| !n.text.trim().is_empty())
            .take(3)
            .map(|n| {
                let snippet: String = n.text.chars().take(120).collect();
                format!("- {}", snippet.replace('\n', " "))
            })
            .collect();
        (
            if open.is_empty() { "None".to_string() } else { open.join("\n") },
            if notes.is_empty() { "None".to_string() } else { notes.join("\n") },
        )
    };
    let clip_snippet = st
        .clips
        .lock()
        .ok()
        .and_then(|c| c.items.first().cloned())
        .map(|c| {
            if c.text.trim().is_empty() {
                "Image / File in clipboard".to_string()
            } else {
                c.text.chars().take(420).collect::<String>()
            }
        })
        .unwrap_or_else(|| "Empty".to_string());
    let disk_info = asi_clean::disk::status(&home_dir())
        .map(|d| format!("{} free ({:.0}%)", fmt_gb(d.free_bytes), d.free_percent))
        .unwrap_or_else(|| "Unknown".into());
    format!(
        "Current local time: {}\nDisk space: {}\nOpen tasks:\n{}\nRecent notes:\n{}\nLatest clipboard content:\n{}",
        lnow.format("%Y-%m-%d %H:%M (%A)"),
        disk_info,
        tasks_summary,
        notes_summary,
        clip_snippet
    )
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

#[tauri::command]
pub(crate) async fn asi_ask(app: AppHandle, st: S<'_>, text: String, offset: Option<i32>) -> Result<assistant::Reply, String> {
    use assistant::{Card, Intent, Reply, TaskBrief};
    let st2 = st.inner().clone();
    let app2 = app.clone();
    blocking(move || {
        let st = &st2;
        let lang = st.store.lock().unwrap().settings.voice_lang.clone();
        let en = looks_english(&text, &lang);
        let (intent, rest) = assistant::classify(&text);
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
                let ai_on = st.store.lock().unwrap().settings.apple_ai_enabled;
                if ai_on && !text.trim().is_empty() {
                    let ctx = build_ai_context(st);
                    if let Ok(ai_ans) = assistant::ask_apple_ai(&st.data_dir, &ctx, &text, en) {
                        let mut r = Reply::new(Intent::Unknown, ai_ans.clone());
                        // Chỉ đọc to câu đầu tiên nếu bật đọc giọng nói để không đọc quá dài
                        let first_sent = ai_ans
                            .split(['.', '!', '?', '\n'])
                            .find(|s| !s.trim().is_empty())
                            .unwrap_or(&ai_ans)
                            .trim()
                            .replace('*', "");
                        r.speak = first_sent;
                        r.mood = "happy".into();
                        r = r
                            .act(if en { "📋 Copy reply" } else { "📋 Chép câu trả lời" }, "copy_text", &ai_ans)
                            .act(if en { "✎ Save to Notes" } else { "✎ Lưu vào Ghi chú" }, "add_note", &ai_ans);
                        if rest.chars().count() <= 90 {
                            r = r.act(if en { "➕ Add as task" } else { "➕ Thêm thành việc" }, "add_task", &rest);
                        }
                        return Ok(r);
                    }
                }
                let mut r = assistant::help(en);
                r.intent = Intent::Unknown;
                r.mood = "idle".into();
                r.text = if en { "Hmm, I didn't get that. Try one of these:".into() } else { "Hmm, Asi chưa hiểu ý bạn. Thử một trong mấy câu này nhé:".into() };
                r.speak = if en { "Sorry, I didn't get that.".into() } else { "Xin lỗi, Asi chưa hiểu ý bạn.".into() };
                if !rest.trim().is_empty() {
                    // Nói một câu Asi không hiểu → cho bấm để biến ngay thành việc / ghi chú
                    r.text = if en { "Hmm, I didn't get that. Save it as a task or a note?".into() } else { "Hmm, Asi chưa hiểu lắm. Lưu câu này thành việc hay ghi chú?".into() };
                    r.actions.insert(0, assistant::Action { label: if en { "➕ Add as task".into() } else { "➕ Thêm thành việc".into() }, cmd: "add_task".into(), arg: rest.clone() });
                    r.actions.insert(1, assistant::Action { label: if en { "✎ Save note".into() } else { "✎ Lưu ghi chú".into() }, cmd: "add_note".into(), arg: rest.clone() });
                }
                r
            }
        };
        Ok(r)
    })
    .await
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
