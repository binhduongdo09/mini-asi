//! Lớp native của Mini-Asi.
//!
//! - `notch_info()`        : màn hình chính có tai thỏ không, kích thước tai thỏ (đơn vị point)
//! - `configure_window()`  : đưa cửa sổ lên trên thanh menu, hiện trên mọi Space (macOS)
//! - `idle_seconds()`      : số giây kể từ lần cuối người dùng chạm bàn phím/chuột

use std::ffi::c_void;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NotchInfo {
    /// true nếu màn hình có tai thỏ thật
    pub has_notch: bool,
    /// Chiều rộng tai thỏ (point). 0 nếu không có.
    pub notch_width: f64,
    /// Chiều cao vùng tai thỏ / thanh menu (point)
    pub notch_height: f64,
    /// Chiều rộng màn hình (point)
    pub screen_width: f64,
    /// Toạ độ x của mép trái màn hình (point, hệ toạ độ toàn cục)
    pub screen_x: f64,
    pub scale: f64,
}

// ─────────────────────────── macOS ───────────────────────────
#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSScreen, NSWindow, NSWindowCollectionBehavior};

    /// Phải gọi trên main thread.
    pub fn notch_info() -> Option<NotchInfo> {
        let mtm = MainThreadMarker::new()?;
        // Ưu tiên màn hình tích hợp có tai thỏ; nếu không có thì lấy màn hình chính.
        let screens = NSScreen::screens(mtm);
        let mut chosen = None;
        for s in screens.iter() {
            let insets = s.safeAreaInsets();
            if insets.top > 0.0 {
                chosen = Some(s.clone());
                break;
            }
        }
        let screen = match chosen {
            Some(s) => s,
            None => NSScreen::mainScreen(mtm)?,
        };
        let frame = screen.frame();
        let insets = screen.safeAreaInsets();
        let scale = screen.backingScaleFactor();
        if insets.top > 0.0 {
            let left = screen.auxiliaryTopLeftArea();
            let right = screen.auxiliaryTopRightArea();
            let notch_w = (frame.size.width - left.size.width - right.size.width).max(0.0);
            Some(NotchInfo {
                has_notch: notch_w > 0.0,
                notch_width: notch_w,
                notch_height: insets.top,
                screen_width: frame.size.width,
                screen_x: frame.origin.x,
                scale,
            })
        } else {
            // Không có tai thỏ: chiều cao thanh menu = frame - visibleFrame (phần trên)
            let visible = screen.visibleFrame();
            let menu_h = (frame.origin.y + frame.size.height) - (visible.origin.y + visible.size.height);
            Some(NotchInfo {
                has_notch: false,
                notch_width: 0.0,
                notch_height: if menu_h > 0.0 { menu_h } else { 24.0 },
                screen_width: frame.size.width,
                screen_x: frame.origin.x,
                scale,
            })
        }
    }

    /// Ngăn macOS AppKit tự động đẩy cửa sổ xuống dưới thanh Menu Bar (`Y = 33pt`)
    /// khi cửa sổ tạm hạ xuống `level = 0` (xin quyền / chọn file) hoặc `level = 3` (AirDrop).
    extern "C-unwind" fn unconstrained_frame_rect(
        _this: *mut objc2::runtime::AnyObject,
        _sel: objc2::runtime::Sel,
        frame_rect: objc2_foundation::NSRect,
        _screen: *mut objc2::runtime::AnyObject,
    ) -> objc2_foundation::NSRect {
        frame_rect
    }

    /// `ns_window` là con trỏ NSWindow (Tauri: `window.ns_window()`).
    ///
    /// # Safety
    /// Con trỏ phải là NSWindow hợp lệ và hàm phải chạy trên main thread.
    pub unsafe fn configure_window(ns_window: *mut c_void) {
        if ns_window.is_null() {
            return;
        }
        let cls = objc2::ffi::object_getClass(ns_window as *const objc2::runtime::AnyObject) as *mut objc2::runtime::AnyClass;
        if !cls.is_null() {
            let sel = objc2::sel!(constrainFrameRect:toScreen:);
            let imp: objc2::runtime::Imp = std::mem::transmute(
                unconstrained_frame_rect
                    as extern "C-unwind" fn(
                        *mut objc2::runtime::AnyObject,
                        objc2::runtime::Sel,
                        objc2_foundation::NSRect,
                        *mut objc2::runtime::AnyObject,
                    ) -> objc2_foundation::NSRect,
            );
            let _ = objc2::ffi::class_replaceMethod(
                cls,
                sel,
                imp,
                c"{CGRect={CGPoint=dd}{CGSize=dd}}@:{CGRect={CGPoint=dd}{CGSize=dd}}@".as_ptr(),
            );
        }
        let win: &NSWindow = &*(ns_window as *const NSWindow);
        // NSMainMenuWindowLevel = 24. Đặt cao hơn để nằm trên thanh menu, che vùng tai thỏ.
        win.setLevel(25 + 2);
        win.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        win.setHasShadow(false);
        win.setMovable(false);
    }

    pub fn is_main_thread() -> bool {
        MainThreadMarker::new().is_some()
    }

    /// Đặt kích thước và vị trí cửa sổ nguyên tử (atomic) theo hệ toạ độ góc trên-trái (top-left) của màn hình chính.
    /// Tránh lỗi `set_size` và `set_position` của Tauri chạy ở 2 lệnh GCD tách rời làm lệch cạnh trên.
    ///
    /// # Safety
    /// `ns_window` phải là NSWindow hợp lệ, gọi trên main thread.
    pub unsafe fn set_window_frame_top_left(ns_window: *mut c_void, x: f64, top_y: f64, w: f64, h: f64) {
        if ns_window.is_null() {
            return;
        }
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let win: &NSWindow = &*(ns_window as *const NSWindow);
        let screens = NSScreen::screens(mtm);
        let primary_h = screens
            .firstObject()
            .or_else(|| NSScreen::mainScreen(mtm))
            .map(|s| s.frame().size.height)
            .unwrap_or(900.0);
        let bottom_y = primary_h - top_y - h;
        let rect = objc2_foundation::NSRect::new(
            objc2_foundation::NSPoint::new(x, bottom_y),
            objc2_foundation::NSSize::new(w, h),
        );
        win.setFrame_display(rect, true);
    }

    /// Đổi tầng cửa sổ: 27 = trên thanh menu (bình thường), 0 = tầng thường (để hộp thoại xin quyền không bị che)
    ///
    /// # Safety
    /// `ns_window` phải là NSWindow hợp lệ, gọi trên main thread.
    pub unsafe fn set_window_level(ns_window: *mut c_void, level: isize) {
        if ns_window.is_null() {
            return;
        }
        let win: &NSWindow = &*(ns_window as *const NSWindow);
        win.setLevel(level);
    }

    /// Trả lại quyền điều khiển bàn phím cho app trước đó khi thu gọn tai thỏ.
    pub fn deactivate_app() {
        if let Some(mtm) = MainThreadMarker::new() {
            let app = objc2_app_kit::NSApplication::sharedApplication(mtm);
            app.deactivate();
        }
    }

    /// Đưa Mini-Asi lên trước (để cửa sổ mới như Threads không bị app khác che)
    #[allow(deprecated)]
    pub fn activate_app() {
        if let Some(mtm) = MainThreadMarker::new() {
            let app = objc2_app_kit::NSApplication::sharedApplication(mtm);
            app.activateIgnoringOtherApps(true);
        }
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct CGPoint {
        x: f64,
        y: f64,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct CGSize {
        width: f64,
        height: f64,
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFRelease(cf: *const c_void);
        fn CFRetain(cf: *const c_void) -> *const c_void;
        fn CFArrayGetCount(the_array: *const c_void) -> isize;
        fn CFArrayGetValueAtIndex(the_array: *const c_void, idx: isize) -> *const c_void;
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventSourceSecondsSinceLastEventType(state_id: i32, event_type: u32) -> f64;
        fn CGEventSourceButtonState(state_id: i32, button: u32) -> bool;
        fn CGEventPost(tap: u32, event: *mut c_void);
    }

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXUIElementCreateApplication(pid: i32) -> *const c_void;
        fn AXUIElementCopyAttributeValue(element: *const c_void, attribute: *const c_void, value: *mut *const c_void) -> i32;
        fn AXUIElementSetAttributeValue(element: *const c_void, attribute: *const c_void, value: *const c_void) -> i32;
        fn AXValueCreate(the_type: u32, value_ptr: *const c_void) -> *const c_void;
        fn AXValueGetValue(value: *const c_void, the_type: u32, value_ptr: *mut c_void) -> bool;
    }

    pub fn idle_seconds() -> Option<f64> {
        // kCGEventSourceStateCombinedSessionState = 0, kCGAnyInputEventType = !0
        let s = unsafe { CGEventSourceSecondsSinceLastEventType(0, u32::MAX) };
        if s.is_finite() && s >= 0.0 { Some(s) } else { None }
    }

    /// Kiểm tra nút chuột trái có đang được nhấn giữ toàn cục không (kCGEventSourceStateCombinedSessionState = 0, kCGMouseButtonLeft = 0)
    pub fn mouse_left_pressed() -> bool {
        unsafe { CGEventSourceButtonState(0, 0) }
    }

    /// Trả về (pid, tên ứng dụng) của ứng dụng đang active phía trước (bỏ qua chính Mini-Asi)
    pub fn frontmost_external_app() -> Option<(i32, String)> {
        unsafe {
            let cls = objc2::runtime::AnyClass::get(c"NSWorkspace")?;
            let ws: *mut objc2::runtime::AnyObject = objc2::msg_send![cls, sharedWorkspace];
            if ws.is_null() {
                return None;
            }
            let app: *mut objc2::runtime::AnyObject = objc2::msg_send![ws, frontmostApplication];
            if app.is_null() {
                return None;
            }
            let pid: i32 = objc2::msg_send![app, processIdentifier];
            if pid <= 0 || pid == std::process::id() as i32 {
                return None;
            }
            let name_obj: *const objc2_foundation::NSString = objc2::msg_send![app, localizedName];
            let name = if !name_obj.is_null() {
                (*name_obj).to_string()
            } else {
                String::from("Ứng dụng")
            };
            Some((pid, name))
        }
    }

    /// Đưa một ứng dụng theo PID lên phía trước
    pub fn activate_pid(pid: i32) {
        if pid <= 0 {
            return;
        }
        unsafe {
            let Some(cls) = objc2::runtime::AnyClass::get(c"NSRunningApplication") else { return };
            let app: *mut objc2::runtime::AnyObject = objc2::msg_send![cls, runningApplicationWithProcessIdentifier: pid];
            if !app.is_null() {
                // NSApplicationActivateIgnoringOtherApps = 1 << 1 = 2
                let _: bool = objc2::msg_send![app, activateWithOptions: 2usize];
            }
        }
    }

    /// Vùng làm việc khả dụng của màn hình chính (trừ thanh Menu Bar / Tai thỏ và thanh Dock) theo hệ toạ độ góc trên-trái (top-left).
    /// Phải gọi trên main thread.
    pub fn work_area() -> Option<(f64, f64, f64, f64)> {
        let mtm = MainThreadMarker::new()?;
        let screens = NSScreen::screens(mtm);
        let primary_h = screens
            .firstObject()
            .or_else(|| NSScreen::mainScreen(mtm))
            .map(|s| s.frame().size.height)
            .unwrap_or(900.0);
        let screen = NSScreen::mainScreen(mtm).or_else(|| screens.firstObject())?;
        let vis = screen.visibleFrame();
        let top_y = primary_h - (vis.origin.y + vis.size.height);
        Some((vis.origin.x, top_y.max(24.0), vis.size.width, vis.size.height))
    }

    unsafe fn copy_ax_window(pid: i32) -> Option<*const c_void> {
        if pid <= 0 {
            return None;
        }
        let app_el = AXUIElementCreateApplication(pid);
        if app_el.is_null() {
            return None;
        }
        let mut win_el: *const c_void = std::ptr::null();
        for key in ["AXFocusedWindow", "AXMainWindow"] {
            let ns = objc2_foundation::NSString::from_str(key);
            if AXUIElementCopyAttributeValue(app_el, (&*ns as *const objc2_foundation::NSString).cast(), &mut win_el) == 0 && !win_el.is_null() {
                CFRelease(app_el);
                return Some(win_el);
            }
        }
        let mut arr: *const c_void = std::ptr::null();
        let ns_wins = objc2_foundation::NSString::from_str("AXWindows");
        if AXUIElementCopyAttributeValue(app_el, (&*ns_wins as *const objc2_foundation::NSString).cast(), &mut arr) == 0 && !arr.is_null() {
            if CFArrayGetCount(arr) > 0 {
                let item = CFArrayGetValueAtIndex(arr, 0);
                if !item.is_null() {
                    CFRetain(item);
                    CFRelease(arr);
                    CFRelease(app_el);
                    return Some(item);
                }
            }
            CFRelease(arr);
        }
        CFRelease(app_el);
        None
    }

    /// Đọc toạ độ và kích thước hiện tại (x, y, w, h) của cửa sổ đang active thuộc `pid` (nếu đã có quyền Trợ năng)
    pub fn window_rect_for_pid(pid: i32) -> Option<(f64, f64, f64, f64)> {
        if super::perm::accessibility() != super::perm::State::Granted {
            return None;
        }
        unsafe {
            let win_el = copy_ax_window(pid)?;
            let mut pos_val: *const c_void = std::ptr::null();
            let mut sz_val: *const c_void = std::ptr::null();
            let ns_pos = objc2_foundation::NSString::from_str("AXPosition");
            let ns_sz = objc2_foundation::NSString::from_str("AXSize");
            let mut pt = CGPoint::default();
            let mut sz = CGSize::default();
            let ok_pos = AXUIElementCopyAttributeValue(win_el, (&*ns_pos as *const objc2_foundation::NSString).cast(), &mut pos_val) == 0
                && !pos_val.is_null()
                && AXValueGetValue(pos_val, 1, (&mut pt as *mut CGPoint).cast());
            if !pos_val.is_null() {
                CFRelease(pos_val);
            }
            let ok_sz = AXUIElementCopyAttributeValue(win_el, (&*ns_sz as *const objc2_foundation::NSString).cast(), &mut sz_val) == 0
                && !sz_val.is_null()
                && AXValueGetValue(sz_val, 2, (&mut sz as *mut CGSize).cast());
            if !sz_val.is_null() {
                CFRelease(sz_val);
            }
            CFRelease(win_el);
            if ok_pos && ok_sz {
                Some((pt.x, pt.y, sz.width, sz.height))
            } else {
                None
            }
        }
    }

    /// Di chuyển và đổi kích thước cửa sổ của ứng dụng `pid` tới (x, y, w, h) bằng Accessibility API
    pub fn snap_app_window(pid: i32, x: f64, y: f64, w: f64, h: f64) -> Result<(), String> {
        if super::perm::accessibility() != super::perm::State::Granted {
            let _ = super::perm::prompt_accessibility();
            return Err("Cần bật quyền Trợ năng (Accessibility) cho Mini-Asi để chia cửa sổ — hãy gạt bật trong bảng Cài đặt vừa mở nhé!".into());
        }
        unsafe {
            let win_el = copy_ax_window(pid).ok_or_else(|| "Không tìm thấy cửa sổ nào của ứng dụng này để chia màn hình".to_string())?;
            let pt = CGPoint { x, y };
            let sz = CGSize { width: w, height: h };
            let pos_val = AXValueCreate(1, (&pt as *const CGPoint).cast());
            let sz_val = AXValueCreate(2, (&sz as *const CGSize).cast());
            let ns_pos = objc2_foundation::NSString::from_str("AXPosition");
            let ns_sz = objc2_foundation::NSString::from_str("AXSize");
            if !pos_val.is_null() && !sz_val.is_null() {
                let _ = AXUIElementSetAttributeValue(win_el, (&*ns_pos as *const objc2_foundation::NSString).cast(), pos_val);
                let _ = AXUIElementSetAttributeValue(win_el, (&*ns_sz as *const objc2_foundation::NSString).cast(), sz_val);
                let _ = AXUIElementSetAttributeValue(win_el, (&*ns_pos as *const objc2_foundation::NSString).cast(), pos_val);
            }
            if !pos_val.is_null() {
                CFRelease(pos_val);
            }
            if !sz_val.is_null() {
                CFRelease(sz_val);
            }
            CFRelease(win_el);
            activate_pid(pid);
            Ok(())
        }
    }

    /// Mở bảng chia sẻ AirDrop gốc của macOS cho danh sách đường dẫn file (gọi trên main thread).
    pub fn airdrop_files(paths: &[String]) -> Result<(), String> {
        if paths.is_empty() {
            return Err("Chưa chọn file để gửi AirDrop".into());
        }
        unsafe {
            let Some(cls) = objc2::runtime::AnyClass::get(c"NSSharingService") else {
                return Err("Không tìm thấy dịch vụ AirDrop".into());
            };
            let name = objc2_foundation::NSString::from_str("com.apple.share.AirDrop.send");
            let svc: *mut objc2::runtime::AnyObject = objc2::msg_send![cls, sharingServiceNamed: &*name];
            if svc.is_null() {
                return Err("Máy này không hỗ trợ AirDrop".into());
            }
            let urls: Vec<objc2::rc::Retained<objc2_foundation::NSURL>> = paths
                .iter()
                .map(|p| objc2_foundation::NSURL::fileURLWithPath(&objc2_foundation::NSString::from_str(p)))
                .collect();
            let arr = objc2_foundation::NSArray::from_retained_slice(&urls);
            activate_app();
            let _: () = objc2::msg_send![svc, performWithItems: &*arr];
            Ok(())
        }
    }

    /// Gửi phím cứng điều khiển Media của macOS (Play/Pause = 16, Next = 17, Prev = 18, Mute = 7, VolUp = 0, VolDown = 1).
    pub fn send_media_key(key: u32) -> bool {
        unsafe {
            let Some(cls) = objc2::runtime::AnyClass::get(c"NSEvent") else {
                return false;
            };
            let pt = objc2_foundation::NSPoint::new(0.0, 0.0);
            for down in [true, false] {
                let flags: usize = if down { 0xa00 } else { 0xb00 };
                let data1: isize = ((key as isize) << 16) | ((if down { 0x0a } else { 0x0b }) << 8);
                let ev: *mut objc2::runtime::AnyObject = objc2::msg_send![
                    cls,
                    otherEventWithType: 14usize,
                    location: pt,
                    modifierFlags: flags,
                    timestamp: 0.0f64,
                    windowNumber: 0isize,
                    context: std::ptr::null::<objc2::runtime::AnyObject>(),
                    subtype: 8i16,
                    data1: data1,
                    data2: -1isize
                ];
                if !ev.is_null() {
                    let cg: *mut c_void = objc2::msg_send![ev, CGEvent];
                    if !cg.is_null() {
                        CGEventPost(0, cg);
                    }
                }
            }
            true
        }
    }
}

// ─────────────────────────── Windows ───────────────────────────
#[cfg(target_os = "windows")]
mod imp {
    use super::*;
    use windows_sys::Win32::System::SystemInformation::GetTickCount;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{keybd_event, GetLastInputInfo, KEYEVENTF_KEYUP, LASTINPUTINFO};

    pub fn notch_info() -> Option<NotchInfo> {
        None // Windows không có tai thỏ → dùng viên thuốc ảo, kích thước lấy từ Tauri
    }

    pub unsafe fn configure_window(_ns_window: *mut c_void) {}

    pub fn deactivate_app() {}

    pub fn activate_app() {}

    pub unsafe fn set_window_level(_w: *mut c_void, _l: isize) {}

    pub unsafe fn set_window_frame_top_left(_w: *mut c_void, _x: f64, _top_y: f64, _width: f64, _height: f64) {}

    pub fn is_main_thread() -> bool {
        false
    }

    pub fn idle_seconds() -> Option<f64> {
        let mut info = LASTINPUTINFO { cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
        let ok = unsafe { GetLastInputInfo(&mut info) };
        if ok == 0 {
            return None;
        }
        let now = unsafe { GetTickCount() };
        Some(now.wrapping_sub(info.dwTime) as f64 / 1000.0)
    }

    pub fn airdrop_files(_paths: &[String]) -> Result<(), String> {
        Err("AirDrop chỉ hỗ trợ trên macOS".into())
    }

    pub fn mouse_left_pressed() -> bool {
        false
    }

    pub fn frontmost_external_app() -> Option<(i32, String)> {
        None
    }

    pub fn activate_pid(_pid: i32) {}

    pub fn work_area() -> Option<(f64, f64, f64, f64)> {
        None
    }

    pub fn window_rect_for_pid(_pid: i32) -> Option<(f64, f64, f64, f64)> {
        None
    }

    pub fn snap_app_window(_pid: i32, _x: f64, _y: f64, _w: f64, _h: f64) -> Result<(), String> {
        Err("Chia cửa sổ hiện chỉ hỗ trợ trên macOS".into())
    }

    pub fn send_media_key(key: u32) -> bool {
        let vk: u8 = match key {
            16 => 0xB3, // VK_MEDIA_PLAY_PAUSE
            17 => 0xB0, // VK_MEDIA_NEXT_TRACK
            18 => 0xB1, // VK_MEDIA_PREV_TRACK
            7 => 0xAD,  // VK_VOLUME_MUTE
            0 => 0xAF,  // VK_VOLUME_UP
            1 => 0xAE,  // VK_VOLUME_DOWN
            _ => return false,
        };
        unsafe {
            keybd_event(vk, 0, 0, 0);
            keybd_event(vk, 0, KEYEVENTF_KEYUP, 0);
        }
        true
    }
}

// ─────────────────────────── Linux & khác ───────────────────────────
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod imp {
    use super::*;

    pub fn notch_info() -> Option<NotchInfo> {
        None
    }

    pub unsafe fn configure_window(_ns_window: *mut c_void) {}

    pub fn deactivate_app() {}

    pub fn activate_app() {}

    pub unsafe fn set_window_level(_w: *mut c_void, _l: isize) {}

    pub unsafe fn set_window_frame_top_left(_w: *mut c_void, _x: f64, _top_y: f64, _width: f64, _height: f64) {}

    pub fn is_main_thread() -> bool {
        false
    }

    /// Dùng `xprintidle` nếu có (X11). Wayland: không hỗ trợ → None.
    pub fn idle_seconds() -> Option<f64> {
        let out = std::process::Command::new("xprintidle").output().ok()?;
        if !out.status.success() {
            return None;
        }
        let ms: f64 = String::from_utf8_lossy(&out.stdout).trim().parse().ok()?;
        Some(ms / 1000.0)
    }

    pub fn airdrop_files(_paths: &[String]) -> Result<(), String> {
        Err("AirDrop chỉ hỗ trợ trên macOS".into())
    }

    pub fn mouse_left_pressed() -> bool {
        false
    }

    pub fn frontmost_external_app() -> Option<(i32, String)> {
        None
    }

    pub fn activate_pid(_pid: i32) {}

    pub fn work_area() -> Option<(f64, f64, f64, f64)> {
        None
    }

    pub fn window_rect_for_pid(_pid: i32) -> Option<(f64, f64, f64, f64)> {
        None
    }

    pub fn snap_app_window(_pid: i32, _x: f64, _y: f64, _w: f64, _h: f64) -> Result<(), String> {
        Err("Chia cửa sổ hiện chỉ hỗ trợ trên macOS".into())
    }

    pub fn send_media_key(key: u32) -> bool {
        let cmd = match key {
            16 => "play-pause",
            17 => "next",
            18 => "previous",
            _ => return false,
        };
        std::process::Command::new("playerctl").arg(cmd).status().map(|s| s.success()).unwrap_or(false)
    }
}

pub use imp::{
    activate_app, activate_pid, airdrop_files, configure_window, deactivate_app, frontmost_external_app,
    idle_seconds, is_main_thread, mouse_left_pressed, notch_info, send_media_key, set_window_frame_top_left,
    set_window_level, snap_app_window, window_rect_for_pid, work_area,
};

/// Mã phím cứng điều khiển Media (chuẩn NX_KEYTYPE của macOS, ánh xạ đa nền tảng)
pub const MEDIA_KEY_VOL_UP: u32 = 0;
pub const MEDIA_KEY_VOL_DOWN: u32 = 1;
pub const MEDIA_KEY_MUTE: u32 = 7;
pub const MEDIA_KEY_PLAY: u32 = 16;
pub const MEDIA_KEY_NEXT: u32 = 17;
pub const MEDIA_KEY_PREV: u32 = 18;

/// Tầng cửa sổ tai thỏ bình thường (trên thanh menu)
pub const NOTCH_LEVEL: isize = 27;

pub mod calendar;
pub mod clip;
pub mod speech;
pub mod ime;
pub mod perm;
