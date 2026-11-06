//! Nhật ký buổi tối (3 câu hỏi cảm xúc).

#[allow(unused_imports)]
use crate::prelude::*;

// ───────────────────────── Nhật ký ─────────────────────────

pub(crate) fn day_range(date: &str, offset: Option<i32>) -> Option<(i64, i64)> {
    let (_, off) = local_now(offset);
    let d = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()?;
    let start = to_ms(d.and_hms_opt(0, 0, 0)?, off);
    Some((start, start + 86_400_000))
}

pub(crate) fn day_facts(st: &AppState, date: &str, offset: Option<i32>) -> journal::DayFacts {
    let Some((a, b)) = day_range(date, offset) else { return Default::default() };
    let (_, off) = local_now(offset);
    let inr = |x: i64| x >= a && x < b;
    let now = now_ms();
    let s = st.store.lock().unwrap();
    let hm = |ms: i64| chrono::DateTime::from_timestamp_millis(ms + off).map(|d| d.format("%H:%M").to_string()).unwrap_or_default();
    let today = inr(now);
    journal::DayFacts {
        tasks_done: s.tasks.iter().filter(|t| t.done_at.is_some_and(inr)).map(|t| t.title.clone()).collect(),
        tasks_open: s.tasks.iter().filter(|t| t.done_at.is_none()).count(),
        tasks_late: s.tasks.iter().filter(|t| t.done_at.is_none() && t.due.is_some_and(|d| d < now.min(b))).count(),
        events: st.events.lock().unwrap().iter().filter(|e| inr(e.start)).map(|e| if e.all_day { e.title.clone() } else { format!("{} {}", hm(e.start), e.title) }).collect(),
        screen_minutes: if today { st.health.lock().unwrap().today_minutes as u32 } else { 0 },
        notes_written: s.notes.iter().filter(|n| inr(n.updated)).count(),
        clips: st.clips.lock().unwrap().items.iter().filter(|c| inr(c.created)).count(),
    }
}

#[derive(Serialize)]
pub(crate) struct HistoryItem {
    pub(crate) date: String,
    pub(crate) mood: Option<u8>,
    pub(crate) preview: String,
}

#[derive(Serialize)]
pub(crate) struct JournalView {
    pub(crate) entry: journal::Entry,
    pub(crate) questions: Vec<String>,
    pub(crate) hints: Vec<String>,
    /// câu Asi nói khi trả lời xong
    pub(crate) closing: String,
    pub(crate) streak: u32,
    pub(crate) history: Vec<HistoryItem>,
}

#[tauri::command]
pub(crate) fn journal_get(st: S, date: String, offset: Option<i32>) -> JournalView {
    let lang = st.store.lock().unwrap().settings.voice_lang.clone();
    let qs = journal::questions(&lang);
    let questions: Vec<String> = qs.iter().map(|q| q.0.to_string()).collect();
    let hints: Vec<String> = qs.iter().map(|q| q.1.to_string()).collect();
    let facts = day_facts(&st, &date, offset);
    let j = st.journal.lock().unwrap();
    let mut entry = j.get(&date).cloned().unwrap_or_else(|| journal::Entry { date: date.clone(), ..Default::default() });
    let (now, _) = local_now(offset);
    if date == now.date().to_string() || entry.facts == journal::DayFacts::default() {
        entry.facts = facts;
    }
    let history = j
        .entries
        .iter()
        .filter(|e| j.has_written(&e.date))
        .take(60)
        .map(|e| HistoryItem {
            date: e.date.clone(),
            mood: e.mood,
            preview: e.answers.iter().map(|a| a.a.trim()).find(|a| !a.is_empty()).unwrap_or(e.text.trim()).chars().take(90).collect(),
        })
        .collect();
    let answered = entry.answers.iter().any(|a| !a.a.trim().is_empty());
    let closing = journal::closing(&lang, now.hour(), answered);
    JournalView { entry, questions, hints, closing, streak: j.streak(now.date()), history }
}

#[tauri::command]
pub(crate) fn journal_save(st: S, entry: journal::Entry, offset: Option<i32>) -> Result<journal::Entry, String> {
    let mut e = entry;
    e.facts = day_facts(&st, &e.date, offset);
    st.journal.lock().unwrap().upsert(e, now_ms())
}

#[tauri::command]
pub(crate) fn journal_delete(st: S, date: String) -> Result<(), String> {
    st.journal.lock().unwrap().delete(&date)
}

#[tauri::command]
pub(crate) fn journal_snooze(st: S, minutes: i64) {
    let mut a = st.journal_ask.lock().unwrap();
    *a = (String::new(), now_ms() + minutes.clamp(5, 240) * 60_000);
}

#[tauri::command]
pub(crate) fn journal_skip_today(st: S) {
    let mut a = st.journal_ask.lock().unwrap();
    *a = (Local::now().date_naive().to_string(), 0);
}

/// Biến câu trả lời "Ngày mai muốn làm gì" thành việc (không ghi giờ → 9h sáng mai)
#[tauri::command]
pub(crate) fn journal_make_tasks(app: AppHandle, st: S, text: String, offset: Option<i32>) -> usize {
    let (now, off) = local_now(offset);
    let tomorrow9 = now.date().succ_opt().and_then(|d| d.and_hms_opt(9, 0, 0)).map(|d| to_ms(d, off));
    let mut n = 0;
    for line in journal::split_plans(&text) {
        let p = nlp::parse(&line, now);
        let due = p.due.map(|d| to_ms(d, off)).or(tomorrow9);
        st.store.lock().unwrap().add_task(p.title, due);
        n += 1;
    }
    push_status(&app);
    kick_sync(&app);
    n
}

#[tauri::command]
pub(crate) fn journal_export(app: AppHandle, st: S) -> Result<String, String> {
    let md = st.journal.lock().unwrap().to_markdown();
    let dir = app.path().document_dir().unwrap_or_else(|_| st.data_dir.clone());
    let p = files::unique(dir.join("Nhật ký Mini-Asi.md"));
    std::fs::write(&p, md).map_err(|e| e.to_string())?;
    files::reveal(&p);
    Ok(p.to_string_lossy().to_string())
}
