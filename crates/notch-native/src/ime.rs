//! Bộ gõ của macOS (Text Input Sources): xem bộ gõ đang dùng và tạm chuyển sang ABC.
//! Mini-Asi dùng để tự gõ tiếng Việt trong cửa sổ tai thỏ, rồi trả lại bộ gõ cũ.
//! Các hàm này phải gọi trên main thread.

#[cfg(target_os = "macos")]
mod imp {
    use std::ffi::{c_char, c_void, CStr, CString};

    type CFRef = *const c_void;
    const UTF8: u32 = 0x0800_0100;

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFStringCreateWithCString(alloc: CFRef, s: *const c_char, enc: u32) -> CFRef;
        fn CFStringGetCString(s: CFRef, buf: *mut c_char, len: isize, enc: u32) -> bool;
        fn CFDictionaryCreate(alloc: CFRef, keys: *const CFRef, values: *const CFRef, n: isize, kcb: *const c_void, vcb: *const c_void) -> CFRef;
        fn CFArrayGetCount(a: CFRef) -> isize;
        fn CFArrayGetValueAtIndex(a: CFRef, i: isize) -> CFRef;
        fn CFRelease(p: CFRef);
        static kCFTypeDictionaryKeyCallBacks: [u8; 0];
        static kCFTypeDictionaryValueCallBacks: [u8; 0];
    }

    #[link(name = "Carbon", kind = "framework")]
    extern "C" {
        fn TISCopyCurrentKeyboardInputSource() -> CFRef;
        fn TISGetInputSourceProperty(src: CFRef, key: CFRef) -> CFRef;
        fn TISCreateInputSourceList(props: CFRef, include_all: bool) -> CFRef;
        fn TISSelectInputSource(src: CFRef) -> i32;
        static kTISPropertyInputSourceID: CFRef;
    }

    fn cfstr_to_string(s: CFRef) -> Option<String> {
        if s.is_null() {
            return None;
        }
        let mut buf = vec![0 as c_char; 512];
        unsafe {
            if CFStringGetCString(s, buf.as_mut_ptr(), buf.len() as isize, UTF8) {
                Some(CStr::from_ptr(buf.as_ptr()).to_string_lossy().to_string())
            } else {
                None
            }
        }
    }

    /// Ví dụ: "com.apple.inputmethod.VietnameseIM.VietnameseTelex", "com.apple.keylayout.ABC"
    pub fn current() -> Option<String> {
        unsafe {
            let src = TISCopyCurrentKeyboardInputSource();
            if src.is_null() {
                return None;
            }
            let id = cfstr_to_string(TISGetInputSourceProperty(src, kTISPropertyInputSourceID));
            CFRelease(src);
            id
        }
    }

    pub fn select(id: &str) -> bool {
        let Ok(c) = CString::new(id) else { return false };
        unsafe {
            let val = CFStringCreateWithCString(std::ptr::null(), c.as_ptr(), UTF8);
            let keys = [kTISPropertyInputSourceID];
            let vals = [val];
            let dict = CFDictionaryCreate(
                std::ptr::null(),
                keys.as_ptr(),
                vals.as_ptr(),
                1,
                kCFTypeDictionaryKeyCallBacks.as_ptr() as *const c_void,
                kCFTypeDictionaryValueCallBacks.as_ptr() as *const c_void,
            );
            let list = TISCreateInputSourceList(dict, false);
            let mut ok = false;
            if !list.is_null() {
                if CFArrayGetCount(list) > 0 {
                    ok = TISSelectInputSource(CFArrayGetValueAtIndex(list, 0)) == 0;
                }
                CFRelease(list);
            }
            CFRelease(dict);
            CFRelease(val);
            ok
        }
    }

    /// Danh sách id các bộ gõ đang bật trên máy
    pub fn enabled() -> Vec<String> {
        let mut out = vec![];
        unsafe {
            let list = TISCreateInputSourceList(std::ptr::null(), false);
            if list.is_null() {
                return out;
            }
            for i in 0..CFArrayGetCount(list) {
                if let Some(id) = cfstr_to_string(TISGetInputSourceProperty(CFArrayGetValueAtIndex(list, i), kTISPropertyInputSourceID)) {
                    out.push(id);
                }
            }
            CFRelease(list);
        }
        out
    }

    /// Chuyển sang một bàn phím Latin: ABC → U.S. → bàn phím bất kỳ đang bật
    pub fn select_latin() -> bool {
        if select("com.apple.keylayout.ABC") || select("com.apple.keylayout.US") {
            return true;
        }
        enabled().iter().filter(|id| super::is_latin_layout(id)).any(|id| select(id))
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub fn current() -> Option<String> {
        None
    }
    pub fn select(_id: &str) -> bool {
        false
    }
    pub fn select_latin() -> bool {
        false
    }
    pub fn enabled() -> Vec<String> {
        vec![]
    }
}

pub use imp::{current, enabled, select, select_latin};

/// Bàn phím thường (không ghép chữ): ABC, U.S., British…
pub fn is_latin_layout(id: &str) -> bool {
    id.starts_with("com.apple.keylayout.")
}

/// Bộ gõ tiếng Việt của Apple đang bật? Trả về "telex" | "vni"
pub fn vietnamese_mode(id: &str) -> Option<&'static str> {
    let l = id.to_lowercase();
    // Bộ gõ của Apple + các bộ gõ tiếng Việt bên thứ ba (VietTelex, OpenKey, EVKey, GoTiengViet, Unikey…)
    const KEYS: &[&str] = &["vietnam", "viet", "telex", "vni", "unikey", "evkey", "openkey", "gotiengviet", "goviet", "xkey", "vkey", ".vn.", "tiengviet"];
    if is_latin_layout(id) || !KEYS.iter().any(|k| l.contains(k)) {
        return None;
    }
    Some(if l.contains("vni") { "vni" } else { "telex" })
}

#[cfg(test)]
mod tests {
    #[test]
    fn modes() {
        assert_eq!(super::vietnamese_mode("com.apple.inputmethod.VietnameseIM.VietnameseTelex"), Some("telex"));
        assert_eq!(super::vietnamese_mode("com.apple.inputmethod.VietnameseIM.VietnameseSimpleTelex"), Some("telex"));
        assert_eq!(super::vietnamese_mode("com.apple.inputmethod.VietnameseIM.VietnameseVNI"), Some("vni"));
        assert_eq!(super::vietnamese_mode("com.apple.keylayout.ABC"), None);
        assert_eq!(super::vietnamese_mode("com.lacviet.inputmethod.VietTelex"), Some("telex"));
        assert_eq!(super::vietnamese_mode("org.openkey.inputmethod.OpenKey"), Some("telex"));
        assert_eq!(super::vietnamese_mode("com.tuyenmai.inputmethod.EVKey.VNI"), Some("vni"));
        assert_eq!(super::vietnamese_mode("com.apple.inputmethod.SCIM.ITABC"), None, "bộ gõ tiếng Trung không phải tiếng Việt");
    }
}
