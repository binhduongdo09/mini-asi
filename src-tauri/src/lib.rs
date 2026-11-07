//! Mini-Asi — backend Tauri.
//!
//! Luồng chính:
//!   • 1 cửa sổ trong suốt ở mép trên giữa màn hình (ôm tai thỏ / tai thỏ ảo)
//!   • UI tự quyết kích thước theo trạng thái → gọi `set_frame`
//!   • Vòng lặp nền 10 giây: nhắc việc đến hạn, điểm trễ (màu đỏ), sức khoẻ, ổ đĩa → sự kiện "status"
//!   • Luồng 80ms: theo dõi chuột để mở "ló ra" khi rê vào tai thỏ

pub mod apple;
pub mod assistant;
pub mod backup;
pub mod clipboard;
pub mod files;
pub mod health;
pub mod journal;
pub mod sync;
pub mod threads;
pub mod tts;
pub mod updater;
pub mod vocab;
pub mod voice;
pub mod license;
pub mod nlp;
pub mod store;
pub mod commands;
pub mod prelude;

#[allow(unused_imports)]
use prelude::*;

fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
    use tauri::tray::TrayIconBuilder;
    let open = MenuItem::with_id(app, "open", "Mở Mini-Asi", true, Some("CmdOrCtrl+Shift+Space"))?;
    let clips = MenuItem::with_id(app, "clips", "Clipboard…", true, None::<&str>)?;
    let journal = MenuItem::with_id(app, "journal", "Viết nhật ký…", true, None::<&str>)?;
    let thweb = MenuItem::with_id(app, "threads-web", "Lướt Threads", true, None::<&str>)?;
    let thclose = MenuItem::with_id(app, "threads-close", "Đóng Threads", true, None::<&str>)?;
    let clean = MenuItem::with_id(app, "clean", "Dọn máy…", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Cài đặt…", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Thoát Mini-Asi", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &clips, &journal, &thweb, &thclose, &clean, &settings, &sep, &quit])?;
    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?;
    TrayIconBuilder::with_id("main")
        .icon(icon)
        .icon_as_template(true)
        .tooltip("Mini-Asi")
        .menu(&menu)
        .on_menu_event(|app, ev| match ev.id.as_ref() {
            "quit" => app.exit(0),
            "threads-close" => threads_close(app.clone()),
            "threads-web" => {
                let h = app.clone();
                tauri::async_runtime::spawn(async move {
                    let st = h.state::<Arc<AppState>>();
                    let _ = threads_web(h.clone(), st, None).await;
                });
            }
            id => {
                let _ = app.emit("open-tab", id.to_string());
            }
        })
        .build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, sc, ev| {
                    if ev.state == ShortcutState::Pressed {
                        if CLIP_SHORTCUT.get() == Some(&sc.id()) {
                            let _ = app.emit("open-tab", "clips");
                        } else {
                            let _ = app.emit("toggle", ());
                        }
                    }
                })
                .build(),
        )
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let data_dir = app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from(".mini-asi"));
            let store = Store::open(data_dir.clone());
            let shortcut = store.settings.shortcut.clone();
            let clip_shortcut = store.settings.clip_shortcut.clone();
            let clips = clipboard::ClipStore::open(&data_dir, store.settings.clip_max);
            let journal = journal::Journal::open(&data_dir);
            let (sync_tx, sync_rx) = mpsc::channel();
            let ts = Local::now().timestamp();
            let win = app.get_webview_window("notch").expect("thiếu cửa sổ notch");
            let geo = detect_geometry(&win);
            let st = Arc::new(AppState {
                store: Mutex::new(store),
                health: Mutex::new(health::HealthState::new(ts, Local::now().ordinal())),
                geo: Mutex::new(geo.clone()),
                last_scan: Mutex::new(None),
                clips: Mutex::new(clips),
                journal: Mutex::new(journal),
                events: Mutex::new(vec![]),
                recorder: Mutex::new(None),
                sync_tx: Mutex::new(Some(sync_tx)),
                alerted: Mutex::new(HashSet::new()),
                journal_ask: Mutex::new((String::new(), 0)),
                threads: Mutex::new(ThreadsCache::default()),
                ime_prev: Mutex::new(None),
                ime_last: Mutex::new(String::new()),
                data_dir: data_dir.clone(),
                tts: tts::Tts::new(&data_dir, tts::Engine::VieNeu),
                nghi: tts::Tts::new(&data_dir, tts::Engine::Nghi),
                vocab: Mutex::new(vocab::Vocab::open(&data_dir)),
                update: Mutex::new(updater::UpdateProgress::default()),
            });
            app.manage(st.clone());

            #[cfg(target_os = "macos")]
            if let Ok(ptr) = win.ns_window() {
                unsafe { notch_native::configure_window(ptr) };
            }
            // Bấm ra ngoài (kể cả khi đang gõ trong khung Threads) → báo giao diện để thu gọn
            {
                let h = app.handle().clone();
                win.on_window_event(move |e| {
                    if let tauri::WindowEvent::Focused(false) = e {
                        let _ = h.emit("window-blur", ());
                    }
                });
            }
            let (init_w, init_h) = {
                let s = st.store.lock().unwrap();
                let disp = s.settings.display_mode.as_str();
                if disp == "bubble" {
                    (58.0 + 36.0, 58.0 + 20.0)
                } else {
                    let wing = match s.settings.wing_size.as_str() {
                        "hidden" => 0.0,
                        "xs" => 28.0,
                        "s" => 56.0,
                        "m" => 88.0,
                        "l" => 128.0,
                        "custom" => s.settings.notch_w.clamp(0.0, 180.0),
                        _ => if s.settings.notch_w > 0.0 { s.settings.notch_w.clamp(0.0, 180.0) } else { 56.0 },
                    };
                    let on_notch = disp == "notch" && geo.has_notch;
                    let base_w = if on_notch { geo.notch_width + wing * 2.0 } else { (130.0 + wing * 1.4).max(90.0) };
                    let bar_h = if s.settings.notch_h >= 20.0 { s.settings.notch_h.clamp(20.0, 54.0) } else if on_notch { geo.notch_height } else { 32.0 };
                    (base_w + 36.0, bar_h + 20.0)
                }
            };
            place(&win, &geo, init_w, init_h);
            apply_level(app.handle(), normal_level(app.handle()));
            let _ = win.show();

            if let Err(e) = app.global_shortcut().register(shortcut.as_str()) {
                eprintln!("Không đăng ký được phím tắt {shortcut}: {e}");
            }
            match clip_shortcut.parse::<tauri_plugin_global_shortcut::Shortcut>() {
                Ok(sc) => {
                    let _ = CLIP_SHORTCUT.set(sc.id());
                    if let Err(e) = app.global_shortcut().register(sc) {
                        eprintln!("Không đăng ký được phím tắt clipboard {clip_shortcut}: {e}");
                    }
                }
                Err(e) => eprintln!("Phím tắt clipboard không hợp lệ {clip_shortcut}: {e}"),
            }
            if let Err(e) = setup_tray(app.handle()) {
                eprintln!("Không tạo được biểu tượng khay: {e}");
            }

            let (h, s2) = (app.handle().clone(), st.clone());
            std::thread::spawn(move || clip_loop(h, s2));
            let (h, s2) = (app.handle().clone(), st.clone());
            std::thread::spawn(move || threads_loop(h, s2));
            let (h, s2) = (app.handle().clone(), st.clone());
            std::thread::spawn(move || backup_loop(h, s2));
            let (h, s2) = (app.handle().clone(), st.clone());
            std::thread::spawn(move || sync_loop(h, s2, sync_rx));
            let (h, s2) = (app.handle().clone(), st.clone());
            std::thread::spawn(move || update_loop(h, s2));
            let h = app.handle().clone();
            std::thread::spawn(move || background_loop(h, st));
            let h = app.handle().clone();
            std::thread::spawn(move || hover_loop(h));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_geometry, set_frame, set_interactive, set_display, move_bubble, window_snap, vocab_batch, vocab_learned, vocab_check, vocab_play, vocab_play_mine, tts_info, tts_install, tts_uninstall, tts_use,
            parse_task, add_task, list_tasks, toggle_task, snooze_task, rename_task, delete_task, clear_done,
            list_notes, save_note, pin_note, delete_note,
            file_info, pick_files, file_zip, file_extract, file_compress_pdf, file_merge_pdf, reveal,
            file_airdrop, cursor_side, media_status, media_control, toggle_mic_mute, ensure_camera_perm,
            clean_scan, clean_run, clean_history, open_disk_settings,
            health_break_done, health_snooze, focus_start, focus_stop,
            get_settings, save_settings, license_info, activate_license, deactivate_license, quit,
            get_status, asi_ask, asi_action, asi_speak, asi_stop_speaking,
            clip_list, clip_copy, clip_merge, clip_copy_files, clip_copy_image, clip_pin, clip_delete, clip_clear, clip_image, clip_to_note, clip_to_task,
            accessibility_status, open_privacy,
            voice_start, voice_stop, voice_cancel,
            sync_now, sync_info, get_events, add_to_calendar,
            journal_get, journal_save, journal_delete, journal_snooze, journal_skip_today, journal_make_tasks, journal_export,
            threads_get, threads_save_app, threads_login, threads_finish, threads_set_token, threads_logout, threads_replies, threads_post, threads_mark_read, threads_web, open_link, threads_embed, threads_embed_ime, threads_embed_nav, threads_close, ime_info,
            permissions_status, request_permissions,
            backup_info, backup_pick_dir, backup_set, backup_now, device_sync_now, backup_restore,
            update_info, update_check, update_install
        ])
        .run(tauri::generate_context!())
        .expect("Mini-Asi không khởi động được");
}

#[cfg(test)]
mod lang_tests {
    #[test]
    fn english_detection() {
        assert!(!super::looks_english("may minh the nao", "vi-VN"));
        assert!(!super::looks_english("máy mình thế nào", "vi-VN"));
        assert!(super::looks_english("how is my mac", "vi-VN"));
        assert!(super::looks_english("clean up please", "vi-VN"));
        assert!(!super::looks_english("don rac giup minh", "vi-VN"));
    }
}
