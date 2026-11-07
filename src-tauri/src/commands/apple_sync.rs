//! Đồng bộ 2 chiều với Lời nhắc / Ghi chú của Apple, đọc Lịch.

#[allow(unused_imports)]
use crate::prelude::*;

// ───────────────────────── Lịch & đồng bộ Apple ─────────────────────────

#[derive(Serialize, Clone, Default)]
pub(crate) struct SyncReport {
    pub(crate) tasks_pushed: usize,
    pub(crate) tasks_pulled: usize,
    pub(crate) notes_pushed: usize,
    pub(crate) notes_pulled: usize,
    pub(crate) events: usize,
    pub(crate) errors: Vec<String>,
    pub(crate) at: i64,
}

pub(crate) fn sync_tasks(st: &AppState) -> Result<(usize, usize), String> {
    use sync::Plan;
    let started = now_ms();
    let (list, last, tomb, locals) = {
        let s = st.store.lock().unwrap();
        let seen: HashSet<&str> = s.sync.seen_reminders.iter().map(|x| x.as_str()).collect();
        let locals: Vec<sync::Local> = s
            .tasks
            .iter()
            .map(|t| sync::Local {
                id: t.id,
                ext_id: t.ext_id.clone(),
                updated: t.updated.max(t.created),
                seen: t.ext_id.as_deref().is_some_and(|e| seen.contains(e)),
                title: t.title.clone(),
            })
            .collect();
        (s.settings.reminders_list.clone(), s.sync.last_tasks, s.sync.deleted_reminders.clone(), locals)
    };
    let remotes = apple::reminders_pull(&list)?;
    let rs: Vec<sync::Remote> = remotes.iter().map(|r| sync::Remote { id: r.id.clone(), modified: r.modified, stale_done: r.done && started - r.modified > 7 * 86_400_000, title: r.title.clone() }).collect();
    let rmap: HashMap<&str, &apple::RemoteReminder> = remotes.iter().map(|r| (r.id.as_str(), r)).collect();
    let plan = sync::plan(&locals, &rs, last, &tomb);
    let mut ops = vec![];
    let mut pulled = 0;
    {
        let mut s = st.store.lock().unwrap();
        for p in &plan {
            match p {
                Plan::PushNew(id) | Plan::PushUpdate(id, _) => {
                    if let Some(t) = s.tasks.iter().find(|t| t.id == *id) {
                        let mut o = apple::Op::new(if matches!(p, Plan::PushNew(_)) { "create" } else { "update" });
                        o.key = Some(id.to_string());
                        if let Plan::PushUpdate(_, ext) = p {
                            o.id = Some(ext.clone());
                        }
                        o.title = Some(t.title.clone());
                        o.due = t.due;
                        o.done = Some(t.done_at.is_some());
                        ops.push(o);
                    }
                }
                Plan::DeleteRemote(ext) => {
                    let mut o = apple::Op::new("delete");
                    o.id = Some(ext.clone());
                    ops.push(o);
                }
                Plan::PullNew(ext) => {
                    if let Some(r) = rmap.get(ext.as_str()) {
                        let id = s.new_id();
                        s.tasks.push(Task {
                            id,
                            title: r.title.clone(),
                            due: r.due,
                            created: started,
                            done_at: if r.done { Some(r.modified) } else { None },
                            notified: r.due.is_some_and(|d| d <= started),
                            updated: r.modified,
                            ext_id: Some(ext.clone()),
                        });
                        pulled += 1;
                    }
                }
                Plan::PullUpdate(id, ext) => {
                    if let (Some(r), Some(t)) = (rmap.get(ext.as_str()), s.tasks.iter_mut().find(|t| t.id == *id)) {
                        if t.due != r.due {
                            t.notified = r.due.is_some_and(|d| d <= started);
                        }
                        t.title = r.title.clone();
                        t.due = r.due;
                        t.done_at = if r.done { t.done_at.or(Some(r.modified)) } else { None };
                        t.updated = r.modified;
                        pulled += 1;
                    }
                }
                Plan::DeleteLocal(id) => s.tasks.retain(|t| t.id != *id),
                Plan::Relink(id, ext) => {
                    if let Some(t) = s.task_mut(*id) {
                        t.ext_id = Some(ext.clone());
                    }
                }
            }
        }
        s.sync.seen_reminders = remotes.iter().map(|r| r.id.clone()).collect();
        let _ = s.save_tasks();
    }
    let pushed = ops.len();
    let res = apple::reminders_apply(&list, &ops)?;
    let mut s = st.store.lock().unwrap();
    for (k, v) in res {
        if let Ok(id) = k.parse::<u64>() {
            if let Some(t) = s.task_mut(id) {
                t.ext_id = Some(v);
            }
        }
    }
    s.sync.deleted_reminders.clear();
    s.sync.last_tasks = started;
    let _ = s.save_tasks();
    let _ = s.save_sync();
    Ok((pushed, pulled))
}

pub(crate) fn sync_notes(st: &AppState) -> Result<(usize, usize), String> {
    use sync::Plan;
    let started = now_ms();
    let (folder, last, tomb, locals) = {
        let s = st.store.lock().unwrap();
        let locals: Vec<sync::Local> = s.notes.iter().filter(|n| !n.text.trim().is_empty()).map(|n| sync::Local { id: n.id, ext_id: n.ext_id.clone(), updated: n.updated, seen: true, title: String::new() }).collect();
        (s.settings.notes_folder.clone(), s.sync.last_notes, s.sync.deleted_notes.clone(), locals)
    };
    let remotes = apple::notes_pull(&folder)?;
    let rs: Vec<sync::Remote> = remotes.iter().map(|r| sync::Remote { id: r.id.clone(), modified: r.modified, stale_done: false, title: String::new() }).collect();
    let rmap: HashMap<&str, &apple::RemoteNote> = remotes.iter().map(|r| (r.id.as_str(), r)).collect();
    let plan = sync::plan(&locals, &rs, last, &tomb);
    let mut ops = vec![];
    let mut pulled = 0;
    {
        let mut s = st.store.lock().unwrap();
        for p in &plan {
            match p {
                Plan::PushNew(id) | Plan::PushUpdate(id, _) => {
                    if let Some(n) = s.notes.iter().find(|n| n.id == *id) {
                        let mut o = apple::Op::new(if matches!(p, Plan::PushNew(_)) { "create" } else { "update" });
                        o.key = Some(id.to_string());
                        if let Plan::PushUpdate(_, ext) = p {
                            o.id = Some(ext.clone());
                        }
                        o.html = Some(apple::note_html(&n.text));
                        ops.push(o);
                    }
                }
                Plan::DeleteRemote(ext) => {
                    let mut o = apple::Op::new("delete");
                    o.id = Some(ext.clone());
                    ops.push(o);
                }
                Plan::PullNew(ext) => {
                    if let Some(r) = rmap.get(ext.as_str()) {
                        let id = s.new_id();
                        s.notes.push(store::Note { id, text: r.text.clone(), updated: r.modified, pinned: false, ext_id: Some(ext.clone()) });
                        pulled += 1;
                    }
                }
                Plan::PullUpdate(id, ext) => {
                    if let (Some(r), Some(n)) = (rmap.get(ext.as_str()), s.notes.iter_mut().find(|n| n.id == *id)) {
                        n.text = r.text.clone();
                        n.updated = r.modified;
                        pulled += 1;
                    }
                }
                Plan::DeleteLocal(id) => s.notes.retain(|n| n.id != *id),
                Plan::Relink(..) => {}
            }
        }
        let _ = s.save_notes();
    }
    let pushed = ops.len();
    let res = apple::notes_apply(&folder, &ops)?;
    let mut s = st.store.lock().unwrap();
    for (k, v) in res {
        if let Ok(id) = k.parse::<u64>() {
            if let Some(n) = s.notes.iter_mut().find(|n| n.id == id) {
                n.ext_id = Some(v);
            }
        }
    }
    s.sync.deleted_notes.clear();
    s.sync.last_notes = started;
    let _ = s.save_notes();
    let _ = s.save_sync();
    Ok((pushed, pulled))
}

pub(crate) fn refresh_events(st: &AppState) -> Result<usize, String> {
    let now = now_ms();
    let start = now - 12 * 3_600_000;
    let evs = notch_native::calendar::events(start, now + 8 * 86_400_000)?;
    let v: Vec<EventView> = evs
        .into_iter()
        .map(|e| EventView { id: e.id, title: e.title, start: e.start_ms, end: e.end_ms, all_day: e.all_day, location: e.location, calendar: e.calendar })
        .collect();
    let n = v.len();
    *st.events.lock().unwrap() = v;
    Ok(n)
}

pub(crate) fn run_sync(st: &AppState) -> SyncReport {
    let (rem, notes, cal) = {
        let s = st.store.lock().unwrap();
        (s.settings.sync_reminders, s.settings.sync_notes, s.settings.show_calendar)
    };
    let mut rep = SyncReport { at: now_ms(), ..Default::default() };
    if rem {
        match sync_tasks(st) {
            Ok((a, b)) => (rep.tasks_pushed, rep.tasks_pulled) = (a, b),
            Err(e) => rep.errors.push(format!("Lời nhắc: {e}")),
        }
    }
    if notes {
        match sync_notes(st) {
            Ok((a, b)) => (rep.notes_pushed, rep.notes_pulled) = (a, b),
            Err(e) => rep.errors.push(format!("Ghi chú: {e}")),
        }
    }
    if cal {
        match refresh_events(st) {
            Ok(n) => rep.events = n,
            Err(e) => rep.errors.push(format!("Lịch: {e}")),
        }
    } else {
        st.events.lock().unwrap().clear();
    }
    let mut s = st.store.lock().unwrap();
    s.sync.last_run = rep.at;
    s.sync.last_error = if rep.errors.is_empty() { None } else { Some(rep.errors.join(" · ")) };
    let _ = s.save_sync();
    rep
}

/// Đồng bộ mỗi 5 phút, hoặc ~3 giây sau mỗi thay đổi
pub(crate) fn sync_loop(app: AppHandle, st: Arc<AppState>, rx: mpsc::Receiver<()>) {
    std::thread::sleep(Duration::from_secs(4));
    loop {
        let rep = run_sync(&st);
        if rep.tasks_pulled + rep.notes_pulled + rep.events > 0 || !rep.errors.is_empty() {
            let _ = app.emit("synced", &rep);
            push_status(&app);
        }
        match rx.recv_timeout(Duration::from_secs(300)) {
            Ok(()) => {
                std::thread::sleep(Duration::from_secs(3));
                while rx.try_recv().is_ok() {}
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => return,
        }
    }
}

#[tauri::command]
pub(crate) async fn sync_now(app: AppHandle, st: S<'_>) -> Result<SyncReport, String> {
    let st = st.inner().clone();
    let may_prompt = needs_prompt(notch_native::perm::calendar()) || st.store.lock().unwrap().sync.last_run == 0;
    if may_prompt {
        window_low(&app, true);
    }
    let rep = blocking(move || Ok(run_sync(&st))).await;
    if may_prompt {
        window_low(&app, false);
    }
    let rep = rep?;
    let _ = app.emit("synced", &rep);
    push_status(&app);
    Ok(rep)
}

#[derive(Serialize)]
pub(crate) struct SyncInfo {
    pub(crate) last_run: i64,
    pub(crate) last_error: Option<String>,
    pub(crate) linked_tasks: usize,
    pub(crate) linked_notes: usize,
}

#[tauri::command]
pub(crate) fn sync_info(st: S) -> SyncInfo {
    let s = st.store.lock().unwrap();
    SyncInfo {
        last_run: s.sync.last_run,
        last_error: s.sync.last_error.clone(),
        linked_tasks: s.tasks.iter().filter(|t| t.ext_id.is_some()).count(),
        linked_notes: s.notes.iter().filter(|n| n.ext_id.is_some()).count(),
    }
}

#[tauri::command]
pub(crate) fn get_events(st: S) -> Vec<EventView> {
    st.events.lock().unwrap().clone()
}

#[tauri::command]
pub(crate) async fn add_to_calendar(st: S<'_>, id: u64) -> Result<(), String> {
    let (title, due) = {
        let s = st.store.lock().unwrap();
        let t = s.tasks.iter().find(|t| t.id == id).ok_or("Không tìm thấy việc")?;
        (t.title.clone(), t.due.ok_or("Việc này chưa có giờ — hãy hẹn giờ trước")?)
    };
    let st2 = st.inner().clone();
    blocking(move || {
        notch_native::calendar::create_event(&title, due, due + 30 * 60_000)?;
        let _ = refresh_events(&st2);
        Ok(())
    })
    .await
}
