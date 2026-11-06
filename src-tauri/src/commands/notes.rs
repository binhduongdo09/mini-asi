//! Lệnh cho Ghi chú nhanh.

#[allow(unused_imports)]
use crate::prelude::*;

// ───────────────────────── Ghi chú ─────────────────────────

#[tauri::command]
pub(crate) fn list_notes(st: S) -> Vec<store::Note> {
    let mut v = st.store.lock().unwrap().notes.clone();
    v.sort_by(|a, b| b.pinned.cmp(&a.pinned).then(b.updated.cmp(&a.updated)));
    v
}

#[tauri::command]
pub(crate) fn save_note(app: AppHandle, st: S, id: Option<u64>, text: String) -> store::Note {
    let n = st.store.lock().unwrap().upsert_note(id, text);
    kick_sync(&app);
    n
}

#[tauri::command]
pub(crate) fn pin_note(st: S, id: u64) {
    let mut s = st.store.lock().unwrap();
    if let Some(n) = s.notes.iter_mut().find(|n| n.id == id) {
        n.pinned = !n.pinned;
    }
    let _ = s.save_notes();
}

#[tauri::command]
pub(crate) fn delete_note(app: AppHandle, st: S, id: u64) {
    st.store.lock().unwrap().delete_note(id);
    kick_sync(&app);
}
