//! Lệnh cho Việc (thêm bằng câu tự nhiên, xong, hoãn, sửa, xoá).

#[allow(unused_imports)]
use crate::prelude::*;

// ───────────────────────── Việc ─────────────────────────

/// Giờ địa phương theo đồng hồ của giao diện (UI gửi lệch múi giờ, tính bằng phút).
/// Nhờ vậy hiển thị và tính toán luôn khớp nhau, kể cả khi hệ thống báo múi giờ khác.
pub(crate) fn local_now(offset: Option<i32>) -> (chrono::NaiveDateTime, i64) {
    match offset {
        Some(o) => {
            let off = o as i64 * 60_000;
            let n = chrono::DateTime::from_timestamp_millis(now_ms() + off).unwrap_or_default().naive_utc();
            (n, off)
        }
        None => {
            let n = Local::now();
            (n.naive_local(), n.offset().local_minus_utc() as i64 * 1000)
        }
    }
}

pub(crate) fn to_ms(dt: chrono::NaiveDateTime, off: i64) -> i64 {
    dt.and_utc().timestamp_millis() - off
}

#[derive(Serialize)]
pub(crate) struct ParsePreview {
    pub(crate) title: String,
    pub(crate) due: Option<i64>,
}

#[tauri::command]
pub(crate) fn parse_task(text: String, offset: Option<i32>) -> ParsePreview {
    let (now, off) = local_now(offset);
    let p = nlp::parse(&text, now);
    ParsePreview { title: p.title, due: p.due.map(|d| to_ms(d, off)) }
}

#[tauri::command]
pub(crate) fn add_task(app: AppHandle, st: S, text: String, offset: Option<i32>) -> Result<Task, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("Chưa nhập nội dung".into());
    }
    let (now, off) = local_now(offset);
    let p = nlp::parse(text, now);
    let t = st.store.lock().unwrap().add_task(p.title, p.due.map(|d| to_ms(d, off)));
    push_status(&app);
    kick_sync(&app);
    Ok(t)
}

#[tauri::command]
pub(crate) fn list_tasks(st: S) -> Vec<Task> {
    st.store.lock().unwrap().tasks.clone()
}

#[tauri::command]
pub(crate) fn toggle_task(app: AppHandle, st: S, id: u64) -> Option<Task> {
    let mut s = st.store.lock().unwrap();
    let t = s.task_mut(id)?;
    t.done_at = if t.done_at.is_some() { None } else { Some(now_ms()) };
    t.updated = now_ms();
    let out = t.clone();
    let _ = s.save_tasks();
    drop(s);
    push_status(&app);
    kick_sync(&app);
    Some(out)
}

/// minutes > 0: hoãn N phút. minutes = -1: sáng mai 9h.
#[tauri::command]
pub(crate) fn snooze_task(app: AppHandle, st: S, id: u64, minutes: i64, offset: Option<i32>) -> Option<Task> {
    let mut s = st.store.lock().unwrap();
    let t = s.task_mut(id)?;
    let due = if minutes < 0 {
        let (now, off) = local_now(offset);
        to_ms(now.date().succ_opt()?.and_hms_opt(9, 0, 0)?, off)
    } else {
        now_ms() + minutes * 60_000
    };
    t.due = Some(due);
    t.notified = false;
    t.done_at = None;
    t.updated = now_ms();
    let out = t.clone();
    let _ = s.save_tasks();
    drop(s);
    push_status(&app);
    kick_sync(&app);
    Some(out)
}

#[tauri::command]
pub(crate) fn rename_task(app: AppHandle, st: S, id: u64, title: String) -> Option<Task> {
    let mut s = st.store.lock().unwrap();
    let t = s.task_mut(id)?;
    t.title = title;
    t.updated = now_ms();
    let out = t.clone();
    let _ = s.save_tasks();
    drop(s);
    kick_sync(&app);
    Some(out)
}

#[tauri::command]
pub(crate) fn delete_task(app: AppHandle, st: S, id: u64) {
    st.store.lock().unwrap().delete_task(id);
    push_status(&app);
    kick_sync(&app);
}

#[tauri::command]
pub(crate) fn clear_done(st: S) {
    let mut s = st.store.lock().unwrap();
    let ids: Vec<u64> = s.tasks.iter().filter(|t| t.done_at.is_some()).map(|t| t.id).collect();
    for id in ids {
        s.delete_task(id);
    }
}
