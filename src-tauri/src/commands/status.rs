//! Trạng thái tổng hợp đẩy lên UI + vòng lặp nền (nhắc việc, sức khoẻ, ổ đĩa) + theo dõi chuột.

#[allow(unused_imports)]
use crate::prelude::*;

// ───────────────────────── Trạng thái tổng hợp ─────────────────────────

#[derive(Serialize, Clone)]
pub(crate) struct NextTask {
    pub(crate) id: u64,
    pub(crate) title: String,
    pub(crate) due: i64,
}

#[derive(Serialize, Clone)]
pub(crate) struct Status {
    pub(crate) lateness: store::Lateness,
    pub(crate) next: Option<NextTask>,
    pub(crate) open_count: usize,
    pub(crate) note_count: usize,
    pub(crate) clip_count: usize,
    pub(crate) disk: Option<asi_clean::disk::DiskStatus>,
    pub(crate) session_minutes: u32,
    pub(crate) today_minutes: u32,
    pub(crate) break_minutes: u32,
    pub(crate) focus_until: Option<i64>,
    pub(crate) sleepy: bool,
    pub(crate) focus_mode: bool,
    pub(crate) pro: bool,
    pub(crate) next_event: Option<EventView>,
    pub(crate) threads_unread: u32,
    pub(crate) now: i64,
}

pub(crate) fn build_status(st: &AppState) -> Status {
    let now = now_ms();
    let clip_count = st.clips.lock().map(|c| c.items.len()).unwrap_or(0);
    let (lateness, next, open_count, note_count, sleep_hour, break_minutes, focus_mode) = {
        let s = st.store.lock().unwrap();
        let open: Vec<&Task> = s.tasks.iter().filter(|t| t.done_at.is_none()).collect();
        let next = open
            .iter()
            .filter(|t| t.due.is_some_and(|d| d >= now))
            .min_by_key(|t| t.due)
            .map(|t| NextTask { id: t.id, title: t.title.clone(), due: t.due.unwrap() });
        let notes = s.notes.iter().filter(|n| !n.text.trim().is_empty()).count();
        (store::lateness(&s.tasks, now), next, open.len(), notes, s.settings.sleep_hour, s.settings.break_minutes, s.settings.focus_mode)
    };
    let h = st.health.lock().unwrap();
    let ts = Local::now().timestamp();
    Status {
        lateness,
        next,
        open_count,
        note_count,
        clip_count,
        disk: asi_clean::disk::status(&asi_clean::config::Paths::detect().home),
        session_minutes: h.session_minutes(ts),
        today_minutes: h.today_minutes as u32,
        break_minutes,
        focus_until: h.focus_until,
        sleepy: health::is_sleep_time(Local::now().hour(), sleep_hour),
        focus_mode,
        pro: tier(st) == asi_clean::rules::Tier::Pro,
        next_event: st.events.lock().unwrap().iter().filter(|e| !e.all_day && e.end > now && e.start < now + 86_400_000).min_by_key(|e| e.start).cloned(),
        threads_unread: st.threads.lock().unwrap().unread,
        now,
    }
}

#[tauri::command]
pub(crate) fn get_status(st: S) -> Status {
    build_status(&st)
}

pub(crate) fn push_status(app: &AppHandle) {
    if let Some(st) = app.try_state::<Arc<AppState>>() {
        let _ = app.emit("status", build_status(&st));
    }
}

pub(crate) fn notify(app: &AppHandle, title: &str, body: &str) {
    use tauri_plugin_notification::NotificationExt;
    let _ = app.notification().builder().title(title).body(body).show();
}

/// Vòng lặp nền: nhắc việc, sức khoẻ, hẹn giờ tập trung.
pub(crate) fn background_loop(app: AppHandle, st: Arc<AppState>) {
    loop {
        let now = now_ms();
        // 1. Việc đến hạn
        let due: Vec<Task> = {
            let mut s = st.store.lock().unwrap();
            let mut fired = vec![];
            for t in s.tasks.iter_mut() {
                if t.done_at.is_none() && !t.notified && t.due.is_some_and(|d| d <= now) {
                    t.notified = true;
                    fired.push(t.clone());
                }
            }
            if !fired.is_empty() {
                let _ = s.save_tasks();
            }
            fired
        };
        for t in due {
            notify(&app, "⏰ Đến giờ rồi!", &t.title);
            let _ = app.emit("reminder", &t);
        }
        // 2. Sức khoẻ
        let (bm, bo, wm, wo) = {
            let s = st.store.lock().unwrap();
            let x = &s.settings;
            (x.break_minutes, x.break_enabled && !x.focus_mode, x.water_minutes, x.water_enabled && !x.focus_mode)
        };
        let ts = Local::now().timestamp();
        let nudges = {
            let mut h = st.health.lock().unwrap();
            let mut n = h.tick(ts, Local::now().ordinal(), notch_native::idle_seconds(), bm, bo, wm, wo);
            if let Some(fu) = h.focus_until {
                if ts >= fu {
                    h.focus_until = None;
                    n.push(health::Nudge::Break { minutes: 0 });
                    let _ = app.emit("focus-done", ());
                }
            }
            n
        };
        for n in nudges {
            match n {
                health::Nudge::Break { minutes } if minutes > 0 => {
                    notify(&app, "👀 Nghỉ mắt chút nhé", &format!("Bạn đã dùng máy liên tục {minutes} phút. Nhìn xa 20 giây nào!"));
                    let _ = app.emit("break", minutes);
                }
                health::Nudge::Break { .. } => notify(&app, "🎯 Hết giờ tập trung", "Làm tốt lắm! Nghỉ 5 phút rồi tiếp tục nhé."),
                health::Nudge::Water => {
                    let _ = app.emit("water", ());
                }
            }
        }
        // 3. Sắp đến giờ họp (Lịch)
        let alert_min = st.store.lock().unwrap().settings.event_alert_minutes as i64;
        let soon: Vec<EventView> = st
            .events
            .lock()
            .unwrap()
            .iter()
            .filter(|e| !e.all_day && e.start > now && e.start - alert_min * 60_000 <= now)
            .cloned()
            .collect();
        for e in soon {
            let key = format!("{}@{}", e.id, e.start);
            if st.alerted.lock().unwrap().insert(key) {
                let mins = ((e.start - now) / 60_000).max(1);
                notify(&app, &format!("📅 {} phút nữa: {}", mins, e.title), &e.location);
                let _ = app.emit("event-soon", &e);
            }
        }
        // 4. Hỏi chuyện nhật ký buổi tối
        {
            let (on, hour) = {
                let s = st.store.lock().unwrap();
                (s.settings.journal_enabled, s.settings.journal_hour)
            };
            let lt = Local::now();
            let today = lt.date_naive().to_string();
            if on && lt.hour() >= hour {
                let written = st.journal.lock().unwrap().has_written(&today);
                let mut ask = st.journal_ask.lock().unwrap();
                if !written && ask.0 != today && now >= ask.1 {
                    *ask = (today, 0);
                    drop(ask);
                    notify(&app, "📔 Hôm nay của bạn thế nào?", "Kể Asi nghe vài dòng để lưu vào nhật ký nhé");
                    let _ = app.emit("journal-ask", ());
                }
            }
        }
        push_status(&app);
        std::thread::sleep(Duration::from_secs(10));
    }
}

/// Theo dõi chuột: báo UI khi con trỏ đi vào/ra khỏi cửa sổ (kể cả khi app không được focus).
pub(crate) fn hover_loop(app: AppHandle) {
    let mut inside = false;
    loop {
        std::thread::sleep(Duration::from_millis(80));
        let Some(win) = app.get_webview_window("notch") else { continue };
        let (Ok(c), Ok(p), Ok(sz)) = (app.cursor_position(), win.outer_position(), win.outer_size()) else { continue };
        let now_in = c.x >= p.x as f64 && c.x <= (p.x + sz.width as i32) as f64 && c.y >= p.y as f64 - 2.0 && c.y <= (p.y + sz.height as i32) as f64;
        if now_in != inside {
            inside = now_in;
            let _ = app.emit("hover", inside);
        }
    }
}
