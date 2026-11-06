//! Cửa sổ tai thỏ: vị trí, kích thước, tầng cửa sổ, 3 kiểu hiển thị (tai thỏ / viên thuốc / bong bóng), bộ gõ khi mở bảng.

#[allow(unused_imports)]
use crate::prelude::*;

// ───────────────────────── Hình học & cửa sổ ─────────────────────────

pub(crate) fn detect_geometry(win: &WebviewWindow) -> Geometry {
    let platform = std::env::consts::OS.to_string();
    let mons = win.available_monitors().unwrap_or_default();
    let primary = win.primary_monitor().ok().flatten().or_else(|| mons.first().cloned());
    let native = notch_native::notch_info();

    // Chọn màn hình: màn có tai thỏ (khớp theo toạ độ x & chiều rộng), không thì màn chính
    let mut chosen = primary.clone();
    if let Some(n) = native {
        for m in &mons {
            let s = m.scale_factor();
            let x = m.position().x as f64 / s;
            let w = m.size().width as f64 / s;
            if (x - n.screen_x).abs() < 2.0 && (w - n.screen_width).abs() < 2.0 {
                chosen = Some(m.clone());
            }
        }
    }
    let (sx, sy, sw, sh) = chosen
        .map(|m| {
            let s = m.scale_factor();
            (m.position().x as f64 / s, m.position().y as f64 / s, m.size().width as f64 / s, m.size().height as f64 / s)
        })
        .unwrap_or((0.0, 0.0, 1440.0, 900.0));
    let base = Geometry { has_notch: false, notch_width: 0.0, notch_height: 30.0, screen_x: sx, screen_y: sy, screen_width: sw, screen_height: sh, platform };
    match native {
        Some(n) if n.has_notch => Geometry { has_notch: true, notch_width: n.notch_width, notch_height: n.notch_height, ..base },
        Some(n) => Geometry { notch_height: n.notch_height.max(24.0), ..base },
        None => base,
    }
}

/// Kiểu hiển thị: "notch" (ôm tai thỏ) | "pill" (viên thuốc nổi dưới thanh menu) | "bubble" (bong bóng tròn kéo đi đâu cũng được)
#[derive(Clone)]
pub struct Disp {
    pub mode: String,
    pub bx: f64,
    pub by: f64,
}

pub(crate) fn disp_of(st: &AppState) -> Disp {
    let s = st.store.lock().unwrap();
    Disp { mode: s.settings.display_mode.clone(), bx: s.settings.bubble_x, by: s.settings.bubble_y }
}

pub(crate) fn place(win: &WebviewWindow, geo: &Geometry, w: f64, h: f64) {
    let d = win.app_handle().try_state::<Arc<AppState>>().map(|st| disp_of(&st)).unwrap_or(Disp { mode: "notch".into(), bx: 0.0, by: 0.0 });
    let (sx, sy, sw, sh) = (geo.screen_x, geo.screen_y, geo.screen_width, geo.screen_height.max(400.0));
    let (x, y) = if d.bx > 0.0 || d.by > 0.0 {
        // Người dùng đã kéo & ghim ở vị trí tuỳ chọn trên màn hình (áp dụng cho cả tai thỏ, viên thuốc, bong bóng & popup)
        (d.bx.clamp(sx, (sx + sw - w).max(sx)), d.by.clamp(sy, (sy + sh - h).max(sy)))
    } else {
        match d.mode.as_str() {
            "bubble" => {
                // mặc định: góc phải phía trên
                let bx = sx + sw - 140.0;
                let by = sy + 90.0;
                (bx.clamp(sx, (sx + sw - w).max(sx)), by.clamp(sy + geo.notch_height, (sy + sh - h).max(sy)))
            }
            "pill" => (sx + (sw - w) / 2.0, sy + geo.notch_height + 6.0),
            _ => (sx + (sw - w) / 2.0, sy),
        }
    };
    let _ = win.set_size(LogicalSize::new(w, h));
    let _ = win.set_position(LogicalPosition::new(x, y));
}

/// Tầng cửa sổ theo kiểu hiển thị: tai thỏ nằm trên thanh menu; viên thuốc & bong bóng nổi trên mọi cửa sổ
pub(crate) fn normal_level(app: &AppHandle) -> isize {
    let mode = app.try_state::<Arc<AppState>>().map(|st| st.store.lock().unwrap().settings.display_mode.clone()).unwrap_or_default();
    if mode == "notch" || mode.is_empty() { notch_native::NOTCH_LEVEL } else { 3 }
}

#[allow(unused_variables)]
pub(crate) fn apply_level(app: &AppHandle, level: isize) {
    #[cfg(target_os = "macos")]
    if let Some(w) = app.get_webview_window("notch") {
        if let Ok(p) = w.ns_window() {
            let p = p as usize;
            let _ = app.run_on_main_thread(move || unsafe { notch_native::set_window_level(p as *mut std::ffi::c_void, level) });
        }
    }
}

/// Đổi kiểu hiển thị (giao diện tự tải lại sau đó)
#[tauri::command]
pub(crate) fn set_display(app: AppHandle, st: S, mode: String) -> Result<(), String> {
    let m = if ["notch", "pill", "bubble"].contains(&mode.as_str()) { mode } else { "notch".into() };
    {
        let mut s = st.store.lock().unwrap();
        s.settings.display_mode = m;
        s.save_settings()?;
    }
    apply_level(&app, normal_level(&app));
    Ok(())
}

/// Kéo & ghim cửa sổ (tai thỏ / viên thuốc / bong bóng / popup): di chuyển tới (x, y) — toạ độ màn hình (logical).
/// `save = true` khi thả tay. Truyền `x <= 0.0 && y <= 0.0 && save = true` để đặt lại vị trí mặc định.
#[tauri::command]
pub(crate) fn move_bubble(win: WebviewWindow, st: S, x: f64, y: f64, save: bool) {
    if save && x <= 0.0 && y <= 0.0 {
        {
            let mut s = st.store.lock().unwrap();
            s.settings.bubble_x = 0.0;
            s.settings.bubble_y = 0.0;
            let _ = s.save_settings();
        }
        let geo = st.geo.lock().unwrap().clone();
        if let Ok(sz) = win.inner_size() {
            let sf = win.scale_factor().unwrap_or(1.0);
            place(&win, &geo, sz.width as f64 / sf, sz.height as f64 / sf);
        }
        return;
    }
    let _ = win.set_position(LogicalPosition::new(x, y));
    if save {
        let mut s = st.store.lock().unwrap();
        s.settings.bubble_x = x;
        s.settings.bubble_y = y;
        let _ = s.save_settings();
    }
}

#[tauri::command]
pub(crate) fn get_geometry(st: S) -> Geometry {
    st.geo.lock().unwrap().clone()
}

#[tauri::command]
pub(crate) fn set_frame(win: WebviewWindow, st: S, width: f64, height: f64) {
    let geo = st.geo.lock().unwrap().clone();
    let max_h = (geo.screen_height - 20.0).max(400.0);
    place(&win, &geo, width.clamp(120.0, (geo.screen_width - 20.0).max(600.0)), height.clamp(20.0, max_h));
}

/// Chạy trên main thread và chờ kết quả (API bộ gõ của macOS bắt buộc main thread)
pub(crate) fn on_main<T: Send + 'static>(app: &AppHandle, f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    let (tx, rx) = mpsc::channel();
    app.run_on_main_thread(move || {
        let _ = tx.send(f());
    })
    .ok()?;
    rx.recv_timeout(Duration::from_secs(3)).ok()
}

/// Mở rộng cần bàn phím → focus. Thu gọn → trả bàn phím cho app trước đó.
/// Trả về chế độ bộ gõ tiếng Việt của Asi ("telex" | "vni") nếu cần bật.
///
/// Vì sao: bộ gõ tiếng Việt có sẵn của macOS ghép chữ không ổn định trong cửa sổ nằm trên
/// thanh menu. Khi mở bảng, Asi tạm chuyển bàn phím sang ABC và tự gõ dấu (ui/vnkey.js);
/// khi đóng bảng thì trả lại đúng bộ gõ cũ.
#[tauri::command]
pub(crate) async fn set_interactive(app: AppHandle, win: WebviewWindow, st: S<'_>, on: bool) -> Result<Option<String>, String> {
    let (engine, tts_on) = {
        let g = st.store.lock().unwrap();
        (g.settings.viet_engine.clone(), if g.settings.voice_reply { tts::Engine::from_id(&g.settings.tts_engine) } else { None })
    };
    if on {
        let _ = win.set_focus();
        if let Some(e) = tts_on {
            tts_pack(&st, e).ensure_worker(); // nạp sẵn giọng khi mở tai thỏ
        }
        if engine == "off" {
            return Ok(None);
        }
        let cur = on_main(&app, notch_native::ime::current).flatten().unwrap_or_default();
        let sys_mode = notch_native::ime::vietnamese_mode(&cur);
        // bộ gõ ghép chữ bất kỳ (không phải bàn phím ABC/US…)
        let is_ime = !cur.is_empty() && !notch_native::ime::is_latin_layout(&cur);
        let (mode, switch) = match engine.as_str() {
            "telex" => (Some("telex"), is_ime),
            "vni" => (Some("vni"), is_ime),
            _ => (sys_mode, sys_mode.is_some()),
        };
        *st.ime_last.lock().unwrap() = cur.clone();
        if switch {
            let ok = on_main(&app, notch_native::ime::select_latin).unwrap_or(false);
            if ok {
                *st.ime_prev.lock().unwrap() = Some(cur);
            } else {
                // Không có bàn phím ABC/U.S. nào đang bật → không chuyển được.
                // Tắt bộ gõ của Asi để khỏi ghép chữ hai lần, để bộ gõ hệ thống tự lo.
                let _ = app.emit("ime-warn", cur);
                return Ok(None);
            }
        }
        Ok(mode.map(String::from))
    } else {
        if let Some(prev) = st.ime_prev.lock().unwrap().take() {
            on_main(&app, move || notch_native::ime::select(&prev));
        }
        // Đang dùng cửa sổ Threads của Mini-Asi thì không trả focus cho app khác
        let threads_front = app.get_webview_window("threads-web").is_some_and(|w| w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false));
        if !threads_front {
            let _ = app.run_on_main_thread(notch_native::deactivate_app);
        }
        Ok(None)
    }
}

#[derive(Serialize)]
pub(crate) struct ImeInfo {
    pub(crate) current: String,
    pub(crate) recognized: Option<String>,
    pub(crate) enabled: Vec<String>,
}

#[tauri::command]
pub(crate) async fn ime_info(app: AppHandle, st: S<'_>) -> Result<ImeInfo, String> {
    let prev = st.ime_prev.lock().unwrap().clone();
    let last = st.ime_last.lock().unwrap().clone();
    // nếu Asi đang tạm chuyển sang ABC thì bộ gõ "thật" của người dùng là bộ trước đó
    let current = prev.unwrap_or(if last.is_empty() { on_main(&app, notch_native::ime::current).flatten().unwrap_or_default() } else { last });
    let enabled = on_main(&app, notch_native::ime::enabled).unwrap_or_default();
    Ok(ImeInfo { recognized: notch_native::ime::vietnamese_mode(&current).map(String::from), current, enabled })
}

/// Hạ tai thỏ xuống tầng thường trong lúc macOS hiện hộp thoại xin quyền (để không che nút "Cho phép")
pub(crate) fn window_low(app: &AppHandle, low: bool) {
    let _ = app.emit("perm-prompt", low);
    apply_level(app, if low { 0 } else { normal_level(app) });
}

pub(crate) fn needs_prompt(s: notch_native::perm::State) -> bool {
    s == notch_native::perm::State::NotAsked
}
