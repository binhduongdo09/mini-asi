//! Lệnh kiểm tra và tự động cập nhật Mini-Asi từ GitHub Releases (logic ở `updater.rs`).

#[allow(unused_imports)]
use crate::prelude::*;

#[derive(Serialize, Clone)]
pub(crate) struct UpdateView {
    pub(crate) current_version: String,
    pub(crate) repo: String,
    pub(crate) auto_check: bool,
    pub(crate) status: updater::UpdateProgress,
}

pub(crate) fn update_view(st: &AppState) -> UpdateView {
    let s = st.store.lock().unwrap();
    let repo = if s.settings.update_repo.trim().is_empty() {
        updater::DEFAULT_REPO.to_string()
    } else {
        s.settings.update_repo.clone()
    };
    UpdateView {
        current_version: updater::CURRENT_VERSION.to_string(),
        repo,
        auto_check: s.settings.auto_update_check,
        status: st.update.lock().unwrap().clone(),
    }
}

#[tauri::command]
pub(crate) fn update_info(st: S) -> UpdateView {
    update_view(&st)
}

pub(crate) fn check_update_inner(app: &AppHandle, st: &AppState, silent: bool) -> updater::UpdateProgress {
    let repo = {
        let s = st.store.lock().unwrap();
        if s.settings.update_repo.trim().is_empty() {
            updater::DEFAULT_REPO.to_string()
        } else {
            s.settings.update_repo.clone()
        }
    };
    {
        let mut u = st.update.lock().unwrap();
        if u.phase == "downloading" || u.phase == "installing" {
            return u.clone();
        }
        if !silent {
            u.phase = "checking".into();
            u.message = "Đang kiểm tra phiên bản mới trên GitHub…".into();
            u.error = None;
            let _ = app.emit("update-progress", u.clone());
        }
    }
    let next = match updater::check_latest(&repo, &st.data_dir) {
        Ok(rel) => {
            if rel.has_update {
                updater::UpdateProgress {
                    phase: "available".into(),
                    percent: 0,
                    downloaded_bytes: 0,
                    total_bytes: rel.asset.as_ref().map(|a| a.size).unwrap_or(0),
                    message: format!("Đã có phiên bản mới v{}", rel.latest_version),
                    error: None,
                    release: Some(rel),
                }
            } else {
                updater::UpdateProgress {
                    phase: "up_to_date".into(),
                    percent: 100,
                    downloaded_bytes: 0,
                    total_bytes: 0,
                    message: format!("Bạn đang dùng bản mới nhất (v{})", updater::CURRENT_VERSION),
                    error: None,
                    release: Some(rel),
                }
            }
        }
        Err(e) => updater::UpdateProgress {
            phase: if silent { "idle".into() } else { "error".into() },
            percent: 0,
            downloaded_bytes: 0,
            total_bytes: 0,
            message: String::new(),
            error: if silent { None } else { Some(e) },
            release: None,
        },
    };
    *st.update.lock().unwrap() = next.clone();
    let _ = app.emit("update-progress", &next);
    push_status(app);
    next
}

#[tauri::command]
pub(crate) async fn update_check(app: AppHandle, st: S<'_>) -> Result<UpdateView, String> {
    let st2 = st.inner().clone();
    blocking(move || {
        check_update_inner(&app, &st2, false);
        Ok(update_view(&st2))
    })
    .await
}

/// Tải và cài đặt bản cập nhật mới ngay trong ứng dụng, xong tự khởi động lại.
#[tauri::command]
pub(crate) async fn update_install(app: AppHandle, st: S<'_>) -> Result<(), String> {
    let st2 = st.inner().clone();
    {
        let u = st2.update.lock().unwrap();
        if u.phase == "downloading" || u.phase == "installing" {
            return Ok(());
        }
    }
    std::thread::spawn(move || {
        let emit_step = |st: &AppState, app: &AppHandle, p: updater::UpdateProgress| {
            *st.update.lock().unwrap() = p.clone();
            let _ = app.emit("update-progress", p);
        };

        let rel = match st2.update.lock().unwrap().release.clone() {
            Some(r) if r.has_update => r,
            _ => {
                let p = check_update_inner(&app, &st2, false);
                match p.release.filter(|r| r.has_update) {
                    Some(r) => r,
                    None => return,
                }
            }
        };

        let Some(asset) = rel.asset.clone() else {
            emit_step(
                &st2,
                &app,
                updater::UpdateProgress {
                    phase: "error".into(),
                    error: Some("Bản phát hành này chưa có bộ cài đặt cho hệ điều hành của bạn".into()),
                    release: Some(rel),
                    ..Default::default()
                },
            );
            return;
        };

        let work_dir = std::env::temp_dir().join("mini-asi-update");
        let dest = work_dir.join(&asset.name);
        let token = updater::github_token(&st2.data_dir);

        emit_step(
            &st2,
            &app,
            updater::UpdateProgress {
                phase: "downloading".into(),
                percent: 0,
                downloaded_bytes: 0,
                total_bytes: asset.size,
                message: format!("Đang tải Mini-Asi v{}…", rel.latest_version),
                error: None,
                release: Some(rel.clone()),
            },
        );

        let dl = updater::download_asset(&asset, token.as_deref(), &dest, &|got, total| {
            let pct = if total > 0 { ((got * 100) / total).min(100) as u8 } else { 50 };
            emit_step(
                &st2,
                &app,
                updater::UpdateProgress {
                    phase: "downloading".into(),
                    percent: pct,
                    downloaded_bytes: got,
                    total_bytes: total,
                    message: format!("Đang tải v{}… {}%", rel.latest_version, pct),
                    error: None,
                    release: Some(rel.clone()),
                },
            );
        });

        if let Err(e) = dl {
            emit_step(
                &st2,
                &app,
                updater::UpdateProgress {
                    phase: "error".into(),
                    error: Some(e),
                    release: Some(rel),
                    ..Default::default()
                },
            );
            return;
        }

        emit_step(
            &st2,
            &app,
            updater::UpdateProgress {
                phase: "installing".into(),
                percent: 100,
                downloaded_bytes: asset.size,
                total_bytes: asset.size,
                message: "Đang nâng cấp ứng dụng…".into(),
                error: None,
                release: Some(rel.clone()),
            },
        );

        #[cfg(target_os = "macos")]
        {
            let target = updater::macos_target_app();
            if let Err(e) = updater::install_dmg_macos(&dest, &target) {
                emit_step(
                    &st2,
                    &app,
                    updater::UpdateProgress {
                        phase: "error".into(),
                        error: Some(e),
                        release: Some(rel),
                        ..Default::default()
                    },
                );
                return;
            }
            let _ = std::fs::remove_file(&dest);
        }

        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("cmd").args(["/C", "start", "", &dest.to_string_lossy()]).spawn();
        }

        #[cfg(target_os = "linux")]
        {
            if let Ok(appimage) = std::env::var("APPIMAGE") {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755));
                let _ = std::fs::rename(&dest, &appimage);
            } else {
                let _ = std::process::Command::new("xdg-open").arg(&dest).spawn();
            }
        }

        emit_step(
            &st2,
            &app,
            updater::UpdateProgress {
                phase: "restarting".into(),
                percent: 100,
                downloaded_bytes: asset.size,
                total_bytes: asset.size,
                message: "Đã nâng cấp xong! Đang khởi động lại Mini-Asi…".into(),
                error: None,
                release: Some(rel),
            },
        );
        std::thread::sleep(Duration::from_millis(700));
        updater::relaunch_after_exit();
        app.exit(0);
    });
    Ok(())
}

/// Kiểm tra phiên bản mới định kỳ trên GitHub (15 giây sau khi mở máy và mỗi 6 tiếng)
pub(crate) fn update_loop(app: AppHandle, st: Arc<AppState>) {
    std::thread::sleep(Duration::from_secs(15));
    let mut notified_ver = String::new();
    loop {
        let auto = st.store.lock().unwrap().settings.auto_update_check;
        if auto {
            let p = check_update_inner(&app, &st, true);
            if let Some(rel) = p.release.filter(|r| r.has_update) {
                if notified_ver != rel.latest_version {
                    notified_ver = rel.latest_version.clone();
                    notify(
                        &app,
                        &format!("✨ Đã có Mini-Asi v{}", rel.latest_version),
                        "Mở tai thỏ và bấm “Cập nhật ngay” để nâng cấp tự động nhé.",
                    );
                    let _ = app.emit("update-available", &rel);
                }
            }
        }
        std::thread::sleep(Duration::from_secs(6 * 3600));
    }
}
