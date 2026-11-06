//! Quyền hệ thống macOS: Micro, Trợ năng, Full Disk Access.
//! (Lịch & Nhận dạng giọng nói nằm ở calendar.rs / speech.rs)

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Granted,
    Denied,
    NotAsked,
    Unknown,
}

#[cfg(target_os = "macos")]
mod imp {
    #![allow(unused_unsafe)]
    use super::State;
    use block2::RcBlock;
    use objc2::runtime::Bool;
    use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice, AVMediaTypeAudio};
    use std::ffi::c_void;
    use std::sync::mpsc;
    use std::time::Duration;

    pub fn microphone() -> State {
        let Some(t) = (unsafe { AVMediaTypeAudio }) else { return State::Unknown };
        match unsafe { AVCaptureDevice::authorizationStatusForMediaType(t) } {
            AVAuthorizationStatus::Authorized => State::Granted,
            AVAuthorizationStatus::NotDetermined => State::NotAsked,
            _ => State::Denied,
        }
    }

    pub fn request_microphone() -> bool {
        if microphone() == State::Granted {
            return true;
        }
        let Some(t) = (unsafe { AVMediaTypeAudio }) else { return false };
        let (tx, rx) = mpsc::channel();
        let block = RcBlock::new(move |ok: Bool| {
            let _ = tx.send(ok.as_bool());
        });
        unsafe { AVCaptureDevice::requestAccessForMediaType_completionHandler(t, &block) };
        rx.recv_timeout(Duration::from_secs(120)).unwrap_or(false)
    }

    type CFRef = *const c_void;
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFDictionaryCreate(alloc: CFRef, keys: *const CFRef, values: *const CFRef, n: isize, kcb: *const c_void, vcb: *const c_void) -> CFRef;
        fn CFRelease(p: CFRef);
        static kCFTypeDictionaryKeyCallBacks: [u8; 0];
        static kCFTypeDictionaryValueCallBacks: [u8; 0];
        static kCFBooleanTrue: CFRef;
    }
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrusted() -> bool;
        fn AXIsProcessTrustedWithOptions(options: CFRef) -> bool;
        static kAXTrustedCheckOptionPrompt: CFRef;
    }

    pub fn accessibility() -> State {
        if unsafe { AXIsProcessTrusted() } { State::Granted } else { State::Denied }
    }

    /// Hiện hộp thoại macOS "Mini-Asi muốn điều khiển máy tính" (dẫn tới Cài đặt › Trợ năng)
    pub fn prompt_accessibility() -> bool {
        unsafe {
            let keys = [kAXTrustedCheckOptionPrompt];
            let vals = [kCFBooleanTrue];
            let d = CFDictionaryCreate(
                std::ptr::null(),
                keys.as_ptr(),
                vals.as_ptr(),
                1,
                kCFTypeDictionaryKeyCallBacks.as_ptr() as *const c_void,
                kCFTypeDictionaryValueCallBacks.as_ptr() as *const c_void,
            );
            let r = AXIsProcessTrustedWithOptions(d);
            CFRelease(d);
            r
        }
    }

    /// Không có API kiểm tra trực tiếp → thử đọc thư mục được bảo vệ
    pub fn full_disk_access() -> State {
        let Some(home) = std::env::var_os("HOME") else { return State::Unknown };
        let p = std::path::Path::new(&home).join("Library/Safari");
        match std::fs::read_dir(&p) {
            Ok(_) => State::Granted,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => State::Denied,
            Err(_) => State::Unknown,
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::State;
    pub fn microphone() -> State {
        State::Granted
    }
    pub fn request_microphone() -> bool {
        true
    }
    pub fn accessibility() -> State {
        State::Unknown
    }
    pub fn prompt_accessibility() -> bool {
        false
    }
    pub fn full_disk_access() -> State {
        State::Granted
    }
}

pub use imp::{accessibility, full_disk_access, microphone, prompt_accessibility, request_microphone};

/// Trạng thái quyền Lịch
pub fn calendar() -> State {
    #[cfg(target_os = "macos")]
    {
        use objc2_event_kit::{EKAuthorizationStatus, EKEntityType, EKEventStore};
        let s = unsafe { EKEventStore::authorizationStatusForEntityType(EKEntityType::Event) };
        // 3 = Toàn quyền, 4 = Chỉ ghi (macOS 14+) — đều coi là đã cấp
        if s.0 == 3 || s.0 == 4 {
            return State::Granted;
        }
        if s == EKAuthorizationStatus::NotDetermined {
            return State::NotAsked;
        }
        State::Denied
    }
    #[cfg(not(target_os = "macos"))]
    State::Unknown
}

/// Trạng thái quyền Nhận dạng giọng nói
pub fn speech() -> State {
    #[cfg(target_os = "macos")]
    {
        use objc2_speech::{SFSpeechRecognizer, SFSpeechRecognizerAuthorizationStatus as S};
        match unsafe { SFSpeechRecognizer::authorizationStatus() } {
            S::Authorized => State::Granted,
            S::NotDetermined => State::NotAsked,
            _ => State::Denied,
        }
    }
    #[cfg(not(target_os = "macos"))]
    State::Unknown
}
