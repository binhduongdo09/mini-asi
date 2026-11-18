//! Trạng thái tổng hợp đẩy lên UI + vòng lặp nền (nhắc việc, sức khoẻ, ổ đĩa) + theo dõi chuột.

#[allow(unused_imports)]
use crate::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};

// ───────────────────────── Trạng thái tổng hợp ─────────────────────────

static SYS_CACHE: OnceLock<Mutex<(i64, assistant::SysInfo)>> = OnceLock::new();
static NEXT_NOTIF_ID: AtomicU64 = AtomicU64::new(1);

/// Đọc thông tin CPU / RAM / Nhiệt độ có cache 10 giây để tránh gọi `sysctl`/`ioreg` liên tục
pub(crate) fn cached_sysinfo(force: bool) -> assistant::SysInfo {
    let cell = SYS_CACHE.get_or_init(|| Mutex::new((0, assistant::SysInfo::default())));
    let now = now_ms();
    if !force {
        if let Ok(g) = cell.lock() {
            if g.0 > 0 && now - g.0 < 10_000 {
                return g.1.clone();
            }
        }
    }
    let info = assistant::sysinfo();
    if let Ok(mut g) = cell.lock() {
        *g = (now_ms(), info.clone());
    }
    info
}

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
    pub(crate) sys: assistant::SysInfo,
    pub(crate) notifs: Vec<NotifItem>,
    pub(crate) session_minutes: u32,
    pub(crate) today_minutes: u32,
    pub(crate) break_minutes: u32,
    pub(crate) focus_until: Option<i64>,
    pub(crate) sleepy: bool,
    pub(crate) focus_mode: bool,
    pub(crate) pro: bool,
    pub(crate) next_event: Option<EventView>,
    pub(crate) threads_unread: u32,
    pub(crate) update_version: Option<String>,
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
    let notifs = st.notifs.lock().unwrap().clone();
    Status {
        lateness,
        next,
        open_count,
        note_count,
        clip_count,
        disk: asi_clean::disk::status(&asi_clean::config::Paths::detect().home),
        sys: cached_sysinfo(false),
        notifs,
        session_minutes: h.session_minutes(ts),
        today_minutes: h.today_minutes as u32,
        break_minutes,
        focus_until: h.focus_until,
        sleepy: health::is_sleep_time(Local::now().hour(), sleep_hour),
        focus_mode,
        pro: tier(st) == asi_clean::rules::Tier::Pro,
        next_event: st.events.lock().unwrap().iter().filter(|e| !e.all_day && e.end > now && e.start < now + 86_400_000).min_by_key(|e| e.start).cloned(),
        threads_unread: st.threads.lock().unwrap().unread,
        update_version: {
            let u = st.update.lock().unwrap();
            if u.phase == "available" {
                u.release.as_ref().filter(|r| r.has_update).map(|r| r.latest_version.clone())
            } else {
                None
            }
        },
        now,
    }
}

#[tauri::command]
pub(crate) fn get_status(st: S) -> Status {
    build_status(&st)
}

#[tauri::command]
pub(crate) fn clear_notifs(app: AppHandle, st: S, id: Option<u64>) {
    {
        let mut list = st.notifs.lock().unwrap();
        match id {
            Some(i) => list.retain(|n| n.id != i),
            None => list.clear(),
        }
    }
    push_status(&app);
}

pub(crate) fn push_status(app: &AppHandle) {
    if let Some(st) = app.try_state::<Arc<AppState>>() {
        let _ = app.emit("status", build_status(&st));
    }
}

/// Lưu thông báo vào danh sách gần đây để hiển thị khi rê chuột vào tai thỏ
pub(crate) fn push_notif(app: &AppHandle, kind: &str, title: &str, body: &str, tab: &str) {
    let Some(st) = app.try_state::<Arc<AppState>>() else { return };
    let now = now_ms();
    let mut list = st.notifs.lock().unwrap();
    if list.first().is_some_and(|x| x.title == title && x.body == body && now - x.at < 60_000) {
        return;
    }
    list.insert(
        0,
        NotifItem {
            id: NEXT_NOTIF_ID.fetch_add(1, Ordering::Relaxed),
            at: now,
            kind: kind.into(),
            title: title.into(),
            body: body.into(),
            tab: tab.into(),
        },
    );
    list.truncate(15);
}

/// Giao diện đang dùng tiếng Anh không
pub(crate) fn ui_en(app: &AppHandle) -> bool {
    app.try_state::<Arc<AppState>>().map(|st| st.store.lock().unwrap().settings.ui_lang == "en").unwrap_or(false)
}

/// Chọn câu theo ngôn ngữ giao diện (thông báo, menu khay… — những chỗ không qua giao diện web)
pub(crate) fn tl(app: &AppHandle, vi: &str, en: &str) -> String {
    if ui_en(app) { en.to_string() } else { vi.to_string() }
}

pub(crate) fn notify(app: &AppHandle, title: &str, body: &str) {
    use tauri_plugin_notification::NotificationExt;
    let _ = app.notification().builder().title(title).body(body).show();
}

/// Vòng lặp nền: nhắc việc, sức khoẻ, hẹn giờ tập trung, cảnh báo ổ đĩa đầy.
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
            let title = tl(&app, "⏰ Đến giờ rồi!", "⏰ It's time!");
            notify(&app, &title, &t.title);
            push_notif(&app, "reminder", &title, &t.title, "tasks");
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
                    let title = tl(&app, "👀 Nghỉ mắt chút nhé", "👀 Rest your eyes");
                    let body = tl(&app, &format!("Bạn đã dùng máy liên tục {minutes} phút. Nhìn xa 20 giây nào!"), &format!("You've been on the computer for {minutes} minutes. Look far away for 20 seconds!"));
                    notify(&app, &title, &body);
                    push_notif(&app, "break", &title, &body, "home");
                    let _ = app.emit("break", minutes);
                }
                health::Nudge::Break { .. } => {
                    let title = tl(&app, "🎯 Hết giờ tập trung", "🎯 Focus session done");
                    let body = tl(&app, "Làm tốt lắm! Nghỉ 5 phút rồi tiếp tục nhé.", "Great job! Take a 5-minute break, then keep going.");
                    notify(&app, &title, &body);
                    push_notif(&app, "focus", &title, &body, "home");
                }
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
                let title = tl(&app, &format!("📅 {} phút nữa: {}", mins, e.title), &format!("📅 In {} min: {}", mins, e.title));
                notify(&app, &title, &e.location);
                push_notif(&app, "event", &title, &e.location, "tasks");
                let _ = app.emit("event-soon", &e);
            }
        }
        // 4. Hỏi chuyện nhật ký buổi tối
        {
            let (on, hour) = {
                let s = st.store.lock().unwrap();
                (s.settings.journal_enabled && s.settings.module_on("journal"), s.settings.journal_hour)
            };
            let lt = Local::now();
            let today = lt.date_naive().to_string();
            if on && lt.hour() >= hour {
                let written = st.journal.lock().unwrap().has_written(&today);
                let mut ask = st.journal_ask.lock().unwrap();
                if !written && ask.0 != today && now >= ask.1 {
                    *ask = (today, 0);
                    drop(ask);
                    let title = tl(&app, "📔 Hôm nay của bạn thế nào?", "📔 How was your day?");
                    let body = tl(&app, "Kể Asi nghe vài dòng để lưu vào nhật ký nhé", "Tell Asi a few lines for your journal");
                    notify(&app, &title, &body);
                    push_notif(&app, "journal", &title, &body, "journal");
                    let _ = app.emit("journal-ask", ());
                }
            }
        }
        // 5. Cảnh báo ổ đĩa sắp đầy (chỉ báo một lần khi chuyển sang ngưỡng cảnh báo)
        if let Some(d) = asi_clean::disk::status(&asi_clean::config::Paths::detect().home) {
            if matches!(d.level, asi_clean::disk::DiskLevel::Warn | asi_clean::disk::DiskLevel::Critical) {
                if st.alerted.lock().unwrap().insert("disk-low".into()) {
                    let free = asi_clean::disk::human(d.free_bytes);
                    let pct = d.free_percent.round() as u32;
                    let title = tl(&app, "💾 Ổ đĩa sắp đầy!", "💾 Disk space is low!");
                    let body = tl(
                        &app,
                        &format!("Chỉ còn {free} trống ({pct}%). Mở Dọn máy để giải phóng dung lượng."),
                        &format!("Only {free} free ({pct}%). Open Clean to free up space."),
                    );
                    notify(&app, &title, &body);
                    push_notif(&app, "disk", &title, &body, "clean");
                    let _ = app.emit("disk-alert", &d);
                }
            } else {
                st.alerted.lock().unwrap().remove("disk-low");
            }
        }
        push_status(&app);
        std::thread::sleep(Duration::from_secs(10));
    }
}

/// Theo dõi chuột: báo UI khi con trỏ đi vào/ra khỏi cửa sổ (kể cả khi app không được focus).
pub(crate) fn hover_loop(app: AppHandle) {
    let mut inside = false;
    let mut last_at: Option<(i32, i32)> = None;
    let mut sleep_ms = 80u64;
    loop {
        std::thread::sleep(Duration::from_millis(sleep_ms));
        if !inside && notch_native::idle_seconds().is_some_and(|s| s > 30.0) {
            sleep_ms = 350;
            continue;
        }
        let Some(win) = app.get_webview_window("notch") else { continue };
        let (Ok(c), Ok(p), Ok(sz)) = (app.cursor_position(), win.outer_position(), win.outer_size()) else { continue };
        let sf = win.scale_factor().unwrap_or(1.0);
        // Bỏ vùng đệm trong suốt quanh hòn đảo (MARGIN_X=18, MARGIN_B=20) để không "ló ra" nhầm
        let pad = if inside { 2.0 * sf } else { 16.0 * sf };
        let x0 = p.x as f64 + pad;
        let y0 = p.y as f64 - 3.0 * sf;
        let x1 = (p.x + sz.width as i32) as f64 - pad;
        let y1 = (p.y + sz.height as i32) as f64 - pad;
        let now_in = c.x >= x0 && c.x <= x1.max(x0 + 20.0) && c.y >= y0 && c.y <= y1.max(y0 + 12.0);
        // Vị trí chuột trong cửa sổ (px CSS) để UI biết đang rê lên nút micro — gửi trước "hover" và chỉ khi chuột nhích
        if now_in {
            let at = (((c.x - p.x as f64) / sf).round() as i32, ((c.y - p.y as f64) / sf).round() as i32);
            if last_at.map_or(true, |(lx, ly): (i32, i32)| (lx - at.0).abs() + (ly - at.1).abs() >= 2) {
                last_at = Some(at);
                let _ = app.emit("hover-at", [at.0, at.1]);
            }
        } else {
            last_at = None;
        }
        if now_in != inside {
            inside = now_in;
            let _ = app.emit("hover", inside);
        }
        // Gần tai thỏ → quét nhạy; ở xa → giãn ra cho đỡ tốn pin
        let near = now_in || (c.x >= x0 - 220.0 && c.x <= x1 + 220.0 && c.y >= y0 - 120.0 && c.y <= y1 + 220.0);
        sleep_ms = if near { 65 } else { 180 };
    }
}
