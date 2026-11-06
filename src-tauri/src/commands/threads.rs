//! Threads: API chính thức (thống kê, trả lời) + cửa sổ lướt bản mobile + khung nhúng thử nghiệm.

#[allow(unused_imports)]
use crate::prelude::*;

// ───────────────────────── Threads ─────────────────────────

pub(crate) const TOKEN_KEY: &str = "threads_token";
pub(crate) const SECRET_KEY: &str = "threads_app_secret";

pub(crate) fn threads_token(st: &AppState) -> Option<String> {
    threads::secret::get(TOKEN_KEY, &st.data_dir)
}

pub(crate) fn open_url(url: &str) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

pub(crate) fn load_seen(st: &AppState) -> HashSet<String> {
    std::fs::read_to_string(st.data_dir.join("threads_seen.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub(crate) fn save_seen(st: &AppState, seen: &HashSet<String>) {
    // giữ gọn: tối đa 5000 id gần nhất
    let v: Vec<&String> = seen.iter().take(5000).collect();
    if let Ok(s) = serde_json::to_string(&v) {
        let _ = std::fs::write(st.data_dir.join("threads_seen.json"), s);
    }
}

/// Tải lại dữ liệu Threads. Trả về các trả lời / lượt nhắc MỚI để báo.
pub(crate) fn fetch_threads(st: &AppState) -> Result<Vec<(String, String)>, String> {
    let token = threads_token(st).ok_or("Chưa kết nối Threads")?;
    // Gia hạn token khi còn < 10 ngày
    let exp = st.store.lock().unwrap().settings.threads_expires;
    let now = now_ms();
    if exp > 0 && exp - now < 10 * 86_400_000 {
        if let Ok((t, secs)) = threads::refresh(&token) {
            let _ = threads::secret::set(TOKEN_KEY, &t, &st.data_dir);
            let mut s = st.store.lock().unwrap();
            s.settings.threads_expires = now + secs as i64 * 1000;
            let _ = s.save_settings();
        }
    }
    let token = threads_token(st).unwrap_or(token);
    let profile = threads::me(&token)?;
    let overview = threads::overview(&token).ok();
    let mut posts = threads::posts(&token, 12)?;
    for p in posts.iter_mut() {
        p.metrics = threads::post_metrics(&token, &p.id).ok();
    }
    let mentions = threads::mentions(&token).unwrap_or_default();

    let mut c = st.threads.lock().unwrap();
    if c.seen.is_empty() {
        c.seen = load_seen(st);
    }
    let first = !c.primed && c.seen.is_empty();
    let mut fresh = vec![];
    for p in posts.iter_mut().take(8) {
        if p.metrics.as_ref().is_none_or(|m| m.replies == 0) {
            continue;
        }
        if let Ok(rs) = threads::replies(&token, &p.id) {
            for r in rs.into_iter().filter(|r| r.is_reply_owned_by_me != Some(true)) {
                if c.seen.insert(r.id.clone()) && !first {
                    p.unread += 1;
                    fresh.push((format!("@{}", r.username.clone().unwrap_or_default()), r.text.clone().unwrap_or_default()));
                }
            }
        }
    }
    let mut mention_new = 0;
    for m in &mentions {
        if c.seen.insert(m.id.clone()) && !first {
            mention_new += 1;
            fresh.push((format!("@{} nhắc đến bạn", m.username.clone().unwrap_or_default()), m.text.clone().unwrap_or_default()));
        }
    }
    // Cộng dồn "chưa đọc" của các lần trước cho tới khi người dùng mở xem
    let prev: HashMap<String, u32> = c.posts.iter().map(|p| (p.id.clone(), p.unread)).collect();
    for p in posts.iter_mut() {
        p.unread += *prev.get(&p.id).unwrap_or(&0);
    }
    c.mention_unread += mention_new;
    c.primed = true;
    c.profile = Some(profile.clone());
    c.overview = overview;
    c.posts = posts;
    c.mentions = mentions;
    c.unread = c.posts.iter().map(|p| p.unread).sum::<u32>() + c.mention_unread;
    c.fetched = now;
    c.error = None;
    let seen = c.seen.clone();
    drop(c);
    save_seen(st, &seen);
    let mut s = st.store.lock().unwrap();
    if s.settings.threads_user != profile.username {
        s.settings.threads_user = profile.username.clone();
        let _ = s.save_settings();
    }
    Ok(fresh)
}

#[derive(Serialize)]
pub(crate) struct ThreadsView {
    pub(crate) connected: bool,
    pub(crate) has_app: bool,
    pub(crate) app_id: String,
    pub(crate) redirect: String,
    pub(crate) notify: bool,
    pub(crate) expires: i64,
    pub(crate) cache: ThreadsCache,
}

pub(crate) fn threads_view(st: &AppState) -> ThreadsView {
    let s = st.store.lock().unwrap();
    ThreadsView {
        connected: threads_token(st).is_some(),
        has_app: !s.settings.threads_app_id.is_empty() && threads::secret::get(SECRET_KEY, &st.data_dir).is_some(),
        app_id: s.settings.threads_app_id.clone(),
        redirect: s.settings.threads_redirect.clone(),
        notify: s.settings.threads_notify,
        expires: s.settings.threads_expires,
        cache: st.threads.lock().unwrap().clone(),
    }
}

#[tauri::command]
pub(crate) async fn threads_get(st: S<'_>, refresh: bool) -> Result<ThreadsView, String> {
    let st = st.inner().clone();
    blocking(move || {
        let stale = now_ms() - st.threads.lock().unwrap().fetched > 3 * 60_000;
        if threads_token(&st).is_some() && (refresh || stale) {
            if let Err(e) = fetch_threads(&st) {
                st.threads.lock().unwrap().error = Some(e);
            }
        }
        Ok(threads_view(&st))
    })
    .await
}

#[tauri::command]
pub(crate) fn threads_save_app(st: S, app_id: String, secret: String, redirect: String) -> Result<(), String> {
    if !secret.trim().is_empty() {
        threads::secret::set(SECRET_KEY, secret.trim(), &st.data_dir)?;
    }
    let mut s = st.store.lock().unwrap();
    s.settings.threads_app_id = app_id.trim().to_string();
    if !redirect.trim().is_empty() {
        s.settings.threads_redirect = redirect.trim().to_string();
    }
    s.save_settings()
}

/// Mở trang đăng nhập Threads trong trình duyệt
#[tauri::command]
pub(crate) fn threads_login(st: S) -> Result<String, String> {
    let (id, redirect) = {
        let s = st.store.lock().unwrap();
        (s.settings.threads_app_id.clone(), s.settings.threads_redirect.clone())
    };
    if id.is_empty() {
        return Err("Nhập Threads App ID trước".into());
    }
    let url = threads::auth_url(&id, &redirect, &format!("asi{}", now_ms() % 100000));
    open_url(&url);
    Ok(url)
}

/// Người dùng dán địa chỉ trình duyệt chuyển tới (có ?code=...) sau khi bấm "Cho phép"
#[tauri::command]
pub(crate) async fn threads_finish(st: S<'_>, pasted: String) -> Result<ThreadsView, String> {
    let st = st.inner().clone();
    blocking(move || {
        let code = threads::extract_code(&pasted).ok_or("Không thấy mã đăng nhập trong địa chỉ đã dán")?;
        let (id, redirect) = {
            let s = st.store.lock().unwrap();
            (s.settings.threads_app_id.clone(), s.settings.threads_redirect.clone())
        };
        let secret = threads::secret::get(SECRET_KEY, &st.data_dir).ok_or("Thiếu App Secret")?;
        let (token, secs) = threads::login_with_code(&id, &secret, &redirect, &code)?;
        threads::secret::set(TOKEN_KEY, &token, &st.data_dir)?;
        {
            let mut s = st.store.lock().unwrap();
            s.settings.threads_expires = now_ms() + secs as i64 * 1000;
            let _ = s.save_settings();
        }
        *st.threads.lock().unwrap() = ThreadsCache::default();
        fetch_threads(&st)?;
        Ok(threads_view(&st))
    })
    .await
}

/// Dán access token (lấy từ Meta App Dashboard → Threads API → User Token Generator)
#[tauri::command]
pub(crate) async fn threads_set_token(st: S<'_>, token: String) -> Result<ThreadsView, String> {
    let st = st.inner().clone();
    blocking(move || {
        let mut token = token.trim().to_string();
        threads::me(&token)?; // kiểm tra token dùng được
        let mut secs = 60 * 86_400u64;
        if let Some(secret) = threads::secret::get(SECRET_KEY, &st.data_dir) {
            if let Ok((t, s2)) = threads::long_lived(&secret, &token) {
                token = t;
                secs = s2;
            }
        }
        threads::secret::set(TOKEN_KEY, &token, &st.data_dir)?;
        {
            let mut s = st.store.lock().unwrap();
            s.settings.threads_expires = now_ms() + secs as i64 * 1000;
            let _ = s.save_settings();
        }
        *st.threads.lock().unwrap() = ThreadsCache::default();
        fetch_threads(&st)?;
        Ok(threads_view(&st))
    })
    .await
}

#[tauri::command]
pub(crate) fn threads_logout(st: S) {
    threads::secret::delete(TOKEN_KEY, &st.data_dir);
    *st.threads.lock().unwrap() = ThreadsCache::default();
    let mut s = st.store.lock().unwrap();
    s.settings.threads_user.clear();
    s.settings.threads_expires = 0;
    let _ = s.save_settings();
}

#[tauri::command]
pub(crate) async fn threads_replies(st: S<'_>, id: String) -> Result<Vec<threads::Reply>, String> {
    let st = st.inner().clone();
    blocking(move || {
        let token = threads_token(&st).ok_or("Chưa kết nối Threads")?;
        let rs = threads::replies(&token, &id)?;
        let mut c = st.threads.lock().unwrap();
        for r in &rs {
            c.seen.insert(r.id.clone());
        }
        if let Some(p) = c.posts.iter_mut().find(|p| p.id == id) {
            p.unread = 0;
        }
        c.unread = c.posts.iter().map(|p| p.unread).sum::<u32>() + c.mention_unread;
        let seen = c.seen.clone();
        drop(c);
        save_seen(&st, &seen);
        Ok(rs)
    })
    .await
}

/// Đăng bài / trả lời — chỉ chạy khi người dùng bấm "Đăng" và xác nhận trong giao diện
#[tauri::command]
pub(crate) async fn threads_post(st: S<'_>, text: String, reply_to: Option<String>) -> Result<String, String> {
    let st = st.inner().clone();
    blocking(move || {
        let token = threads_token(&st).ok_or("Chưa kết nối Threads")?;
        let id = threads::publish_text(&token, &text, reply_to.as_deref())?;
        let _ = fetch_threads(&st);
        Ok(id)
    })
    .await
}

#[tauri::command]
pub(crate) fn threads_mark_read(st: S) {
    let mut c = st.threads.lock().unwrap();
    for p in c.posts.iter_mut() {
        p.unread = 0;
    }
    c.mention_unread = 0;
    c.unread = 0;
    let seen = c.seen.clone();
    drop(c);
    save_seen(&st, &seen);
}

/// Cửa sổ lướt Threads kiểu web/điện thoại ngay dưới tai thỏ (Threads API không có bảng tin,
/// nên mở chính trang threads.com — đăng nhập 1 lần, lần sau tự nhớ).
/// Cửa sổ này KHÔNG được cấp quyền gọi lệnh của Mini-Asi (capabilities chỉ dành cho cửa sổ "notch").
#[tauri::command]
pub(crate) async fn threads_web(app: AppHandle, st: S<'_>, url: Option<String>) -> Result<(), String> {
    let target = url.filter(|u| u.starts_with("https://www.threads.") || u.starts_with("https://threads.")).unwrap_or_else(|| "https://www.threads.com/".into());
    // Đưa cửa sổ Threads lên trước SAU khi tai thỏ thu gọn xong (thu gọn trả focus cho app trước đó,
    // nếu mở ngay thì Threads bị app khác che mất — lỗi hay gặp ở chế độ bong bóng/viên thuốc)
    let bring_front = |app: &AppHandle| {
        let h = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(550));
            let h2 = h.clone();
            let _ = h.run_on_main_thread(move || {
                notch_native::activate_app();
                if let Some(w) = h2.get_webview_window("threads-web") {
                    let _ = w.unminimize();
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            });
        });
    };
    if let Some(w) = app.get_webview_window("threads-web") {
        // đang ở trước mặt → bấm lần nữa để ẩn; bị che/thu nhỏ → đưa lên trước
        if w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false) && !w.is_minimized().unwrap_or(false) && target.ends_with("threads.com/") {
            let _ = w.hide();
            return Ok(());
        }
        if !target.ends_with("threads.com/") {
            if let Ok(u) = target.parse() {
                let _ = w.navigate(u);
            }
        }
        bring_front(&app);
        return Ok(());
    }
    let (geo, d) = (st.geo.lock().unwrap().clone(), disp_of(&st));
    let sh = geo.screen_height.max(600.0);
    let top = geo.screen_y + geo.notch_height.max(24.0) + 14.0;
    let w = 440.0;
    let h = (sh - (top - geo.screen_y) - 40.0).clamp(480.0, 860.0);
    let x = match d.mode.as_str() {
        // bong bóng: mở cạnh bong bóng, không đè lên nó
        "bubble" if d.bx > 0.0 => {
            let left = d.bx - w - 16.0;
            if left >= geo.screen_x + 8.0 { left } else { (d.bx + 90.0).min(geo.screen_x + geo.screen_width - w - 8.0) }
        }
        _ => geo.screen_x + (geo.screen_width - w) / 2.0,
    };
    tauri::WebviewWindowBuilder::new(&app, "threads-web", tauri::WebviewUrl::External(target.parse().map_err(|e| format!("{e}"))?))
        .title("Threads · Mini-Asi")
        .inner_size(w, h)
        .min_inner_size(360.0, 480.0)
        .position(x.max(geo.screen_x), top)
        .user_agent(MOBILE_UA)
        .always_on_top(false)
        .resizable(true)
        .closable(true)
        .minimizable(true)
        .focused(true)
        .on_navigation(move |u| {
            if u.scheme() == "https" && threads_host_ok(u.host_str().unwrap_or("")) {
                true
            } else {
                // link ra ngoài (bài báo, web khác) → mở bằng trình duyệt của máy
                open_url(u.as_str());
                false
            }
        })
        .build()
        .map_err(|e| format!("Không mở được cửa sổ Threads: {e}"))?;
    bring_front(&app);
    Ok(())
}

pub(crate) const MOBILE_UA: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1";

pub(crate) fn threads_host_ok(host: &str) -> bool {
    ["threads.com", "threads.net", "instagram.com", "facebook.com", "fbcdn.net", "cdninstagram.com", "meta.com", "fbsbx.com"]
        .iter()
        .any(|d| host == *d || host.ends_with(&format!(".{d}")))
}

/// Threads (bản mobile) nằm NGAY TRONG bảng tai thỏ: một webview con đặt đúng vào khung
/// mà giao diện chừa sẵn. Không có quyền gọi lệnh Mini-Asi. Có kèm bộ gõ tiếng Việt của Asi.
#[tauri::command]
pub(crate) fn threads_embed(app: AppHandle, win: WebviewWindow, show: bool, x: f64, y: f64, w: f64, h: f64, ime: Option<String>, url: Option<String>) -> Result<(), String> {
    let mode_js = match ime.as_deref() {
        Some("telex") => "\"telex\"",
        Some("vni") => "\"vni\"",
        _ => "null",
    };
    if let Some(wv) = app.get_webview("threads-embed") {
        if show {
            let _ = wv.set_position(LogicalPosition::new(x, y));
            let _ = wv.set_size(LogicalSize::new(w.max(200.0), h.max(200.0)));
            let _ = wv.show();
            let _ = wv.eval(format!("window.VNKey && (VNKey.mode = {mode_js}, VNKey.suspended = false)"));
            if let Some(u) = url.and_then(|u| u.parse().ok()) {
                let _ = wv.navigate(u);
            }
        } else {
            let _ = wv.hide();
        }
        return Ok(());
    }
    if !show {
        return Ok(());
    }
    let target = url.unwrap_or_else(|| "https://www.threads.com/".into());
    let init = format!("{}\n;window.VNKey && (VNKey.mode = {mode_js});", include_str!("../../../ui/vnkey.js"));
    let builder = tauri::webview::WebviewBuilder::new("threads-embed", tauri::WebviewUrl::External(target.parse().map_err(|e| format!("{e}"))?))
        .user_agent(MOBILE_UA)
        .initialization_script(&init)
        .on_navigation(|u| {
            if u.scheme() == "https" && threads_host_ok(u.host_str().unwrap_or("")) {
                true
            } else {
                open_url(u.as_str());
                false
            }
        });
    win.as_ref().window().add_child(builder, LogicalPosition::new(x, y), LogicalSize::new(w.max(200.0), h.max(200.0))).map_err(|e| format!("Không nhúng được Threads: {e}"))?;
    Ok(())
}

/// Đổi bộ gõ của Asi bên trong khung Threads khi người dùng bấm nút VI/EN
/// Đóng mọi thứ của Threads (cửa sổ riêng + khung nhúng) — dùng khi bị kẹt
#[tauri::command]
pub(crate) fn threads_close(app: AppHandle) {
    if let Some(w) = app.get_webview_window("threads-web") {
        let _ = w.close();
    }
    if let Some(wv) = app.get_webview("threads-embed") {
        let _ = wv.close();
    }
}

#[tauri::command]
pub(crate) fn threads_embed_ime(app: AppHandle, ime: Option<String>) {
    if let Some(wv) = app.get_webview("threads-embed") {
        let m = match ime.as_deref() {
            Some("telex") => "\"telex\"",
            Some("vni") => "\"vni\"",
            _ => "null",
        };
        let _ = wv.eval(format!("window.VNKey && (VNKey.mode = {m}, VNKey.suspended = false)"));
    }
}

#[tauri::command]
pub(crate) fn threads_embed_nav(app: AppHandle, action: String) {
    if let Some(wv) = app.get_webview("threads-embed") {
        let js = match action.as_str() {
            "back" => "history.back()",
            "home" => "location.href='https://www.threads.com/'",
            "reload" => "location.reload()",
            _ => return,
        };
        let _ = wv.eval(js);
    }
}

#[tauri::command]
pub(crate) fn open_link(url: String) {
    if url.starts_with("https://") {
        open_url(&url);
    }
}

/// Kiểm tra Threads mỗi 10 phút, báo khi có trả lời / lượt nhắc mới
pub(crate) fn threads_loop(app: AppHandle, st: Arc<AppState>) {
    std::thread::sleep(Duration::from_secs(20));
    loop {
        if threads_token(&st).is_some() {
            match fetch_threads(&st) {
                Ok(fresh) => {
                    let on = st.store.lock().unwrap().settings.threads_notify;
                    if on {
                        for (who, text) in fresh.iter().take(3) {
                            notify(&app, &format!("💬 {who}"), &text.chars().take(120).collect::<String>());
                        }
                        if fresh.len() > 3 {
                            notify(&app, "💬 Threads", &format!("và {} phản hồi mới khác", fresh.len() - 3));
                        }
                    }
                    let _ = app.emit("threads", st.threads.lock().unwrap().unread);
                }
                Err(e) => st.threads.lock().unwrap().error = Some(e),
            }
        }
        std::thread::sleep(Duration::from_secs(600));
    }
}
