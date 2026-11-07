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

#[derive(Serialize, Clone)]
struct WindowDragPayload {
    phase: &'static str, // "move" | "leave" | "drop"
    x: f64,
    y: f64,
    pid: i32,
    app_name: String,
    prev_pid: i32,
    prev_name: String,
}

/// Theo dõi chuột: báo UI khi con trỏ đi vào/ra khỏi cửa sổ (kể cả khi app không được focus)
/// và phát hiện khi người dùng đang kéo một cửa sổ ứng dụng khác đưa lên tai thỏ để chia màn hình.
pub(crate) fn hover_loop(app: AppHandle) {
    let mut inside = false;
    let mut sleep_ms = 80u64;
    let mut was_pressed = false;
    let mut drag_origin: Option<(f64, f64)> = None;
    let mut drag_win_rect: Option<(f64, f64, f64, f64)> = None;
    let mut drag_confirmed = false;
    let mut drag_active_in_notch = false;

    loop {
        std::thread::sleep(Duration::from_millis(sleep_ms));
        if !inside && !was_pressed && notch_native::idle_seconds().is_some_and(|s| s > 30.0) {
            sleep_ms = 350;
            continue;
        }
        let Some(win) = app.get_webview_window("notch") else { continue };
        let (Ok(c), Ok(p), Ok(sz)) = (app.cursor_position(), win.outer_position(), win.outer_size()) else { continue };
        let sf = win.scale_factor().unwrap_or(1.0);
        let (cur_app, prev_app) = current_and_prev_ext_apps();

        // Tránh vùng đệm bóng đổ trong suốt (MARGIN_X=18, MARGIN_B=20) khi xét điểm vào để không bị nhấp nháy
        let pad_x = if inside || drag_active_in_notch { 2.0 * sf } else { 16.0 * sf };
        let pad_b = if inside || drag_active_in_notch { 2.0 * sf } else { 16.0 * sf };
        let x0 = p.x as f64 + pad_x;
        let y0 = p.y as f64 - 3.0 * sf;
        let x1 = (p.x + sz.width as i32) as f64 - pad_x;
        let y1 = (p.y + sz.height as i32) as f64 - pad_b;
        let now_in = c.x >= x0 && c.x <= x1.max(x0 + 20.0) && c.y >= y0 && c.y <= y1.max(y0 + 12.0);
        let rel_x = (c.x - p.x as f64) / sf;
        let rel_y = (c.y - p.y as f64) / sf;

        let pressed = notch_native::mouse_left_pressed();
        if pressed && !was_pressed {
            // Bắt đầu nhấn chuột trái: nếu nhấn ở ngoài tai thỏ thì ghi nhận điểm xuất phát
            if !now_in && cur_app.0 > 0 {
                drag_origin = Some((c.x, c.y));
                drag_win_rect = notch_native::window_rect_for_pid(cur_app.0);
                drag_confirmed = false;
                drag_active_in_notch = false;
            } else {
                drag_origin = None;
                drag_win_rect = None;
                drag_confirmed = false;
                drag_active_in_notch = false;
            }
        } else if pressed && was_pressed {
            if let Some((ox, oy)) = drag_origin {
                let moved = (c.x - ox).hypot(c.y - oy) >= 18.0 * sf;
                if moved && !drag_confirmed {
                    if let Some((wx0, wy0, ww0, _)) = drag_win_rect {
                        let cur_rect = notch_native::window_rect_for_pid(cur_app.0);
                        let win_moved = cur_rect
                            .map(|(wx1, wy1, _, _)| (wx1 - wx0).hypot(wy1 - wy0) >= 4.0)
                            .unwrap_or(true);
                        let lx = ox / sf;
                        let ly = oy / sf;
                        let on_titlebar = ly >= wy0 - 10.0 && ly <= wy0 + 60.0 && lx >= wx0 && lx <= wx0 + ww0;
                        if win_moved || on_titlebar {
                            drag_confirmed = true;
                        }
                    } else {
                        drag_confirmed = true;
                    }
                }
                if drag_confirmed {
                    let drag_enter_hit = now_in
                        || (!drag_active_in_notch
                            && c.y >= y0 - 8.0 * sf
                            && c.y <= y1 + 22.0 * sf
                            && c.x >= p.x as f64 - 65.0 * sf
                            && c.x <= (p.x + sz.width as i32) as f64 + 65.0 * sf);
                    if drag_enter_hit {
                        drag_active_in_notch = true;
                        let _ = app.emit(
                            "window-drag",
                            WindowDragPayload {
                                phase: "move",
                                x: rel_x,
                                y: rel_y,
                                pid: cur_app.0,
                                app_name: cur_app.1.clone(),
                                prev_pid: prev_app.0,
                                prev_name: prev_app.1.clone(),
                            },
                        );
                    } else if drag_active_in_notch
                        && (c.y > y1 + 40.0 * sf || c.x < x0 - 50.0 * sf || c.x > x1 + 50.0 * sf)
                    {
                        drag_active_in_notch = false;
                        let _ = app.emit(
                            "window-drag",
                            WindowDragPayload {
                                phase: "leave",
                                x: rel_x,
                                y: rel_y,
                                pid: cur_app.0,
                                app_name: cur_app.1.clone(),
                                prev_pid: prev_app.0,
                                prev_name: prev_app.1.clone(),
                            },
                        );
                    }
                }
            }
        } else if !pressed && was_pressed {
            if drag_active_in_notch {
                let _ = app.emit(
                    "window-drag",
                    WindowDragPayload {
                        phase: "drop",
                        x: rel_x,
                        y: rel_y,
                        pid: cur_app.0,
                        app_name: cur_app.1.clone(),
                        prev_pid: prev_app.0,
                        prev_name: prev_app.1.clone(),
                    },
                );
            }
            drag_origin = None;
            drag_win_rect = None;
            drag_confirmed = false;
            drag_active_in_notch = false;
        }
        was_pressed = pressed;

        if now_in != inside {
            inside = now_in;
            if !drag_active_in_notch {
                let _ = app.emit("hover", inside);
            }
        }
        // Chuột đang ở trong, đang kéo cửa sổ, hoặc tiến lại gần (< 220px) → quét nhạy 50-65ms; ở xa → giãn lên 180ms
        let near = now_in
            || drag_active_in_notch
            || (pressed && drag_origin.is_some())
            || (c.x >= x0 - 220.0 && c.x <= x1 + 220.0 && c.y >= y0 - 120.0 && c.y <= y1 + 220.0);
        sleep_ms = if drag_active_in_notch { 45 } else if near { 65 } else { 180 };
    }
}
