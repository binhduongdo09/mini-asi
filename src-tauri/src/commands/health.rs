//! Sức khoẻ: nghỉ mắt, uống nước, hẹn giờ tập trung.

#[allow(unused_imports)]
use crate::prelude::*;

// ───────────────────────── Sức khoẻ ─────────────────────────

#[tauri::command]
pub(crate) fn health_break_done(app: AppHandle, st: S) {
    st.health.lock().unwrap().took_break(Local::now().timestamp());
    push_status(&app);
}

#[tauri::command]
pub(crate) fn health_snooze(st: S, minutes: i64) {
    st.health.lock().unwrap().snooze(Local::now().timestamp(), minutes.clamp(1, 120));
}

#[tauri::command]
pub(crate) fn focus_start(app: AppHandle, st: S, minutes: i64) {
    st.health.lock().unwrap().focus_until = Some(Local::now().timestamp() + minutes.clamp(1, 180) * 60);
    push_status(&app);
}

#[tauri::command]
pub(crate) fn focus_stop(app: AppHandle, st: S) {
    st.health.lock().unwrap().focus_until = None;
    push_status(&app);
}
