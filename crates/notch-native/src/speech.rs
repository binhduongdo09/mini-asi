//! Nhận dạng giọng nói từ file âm thanh (macOS: Apple Speech — vi-VN, en-US và nhiều ngôn ngữ khác).

#[cfg(target_os = "macos")]
mod imp {
    #![allow(unused_unsafe)]
    use block2::RcBlock;
    use objc2::{sel, AnyThread};
    use objc2_foundation::{NSError, NSLocale, NSObjectProtocol, NSOperationQueue, NSString, NSURL};
    use objc2_speech::{
        SFSpeechRecognitionResult, SFSpeechRecognizer, SFSpeechRecognizerAuthorizationStatus, SFSpeechURLRecognitionRequest,
    };
    use std::sync::mpsc;
    use std::time::Duration;

    pub fn ensure_access() -> Result<(), String> {
        let st = unsafe { SFSpeechRecognizer::authorizationStatus() };
        if st == SFSpeechRecognizerAuthorizationStatus::Authorized {
            return Ok(());
        }
        if st == SFSpeechRecognizerAuthorizationStatus::Denied || st == SFSpeechRecognizerAuthorizationStatus::Restricted {
            return Err("Chưa cấp quyền Nhận dạng giọng nói — vào Cài đặt hệ thống › Quyền riêng tư".into());
        }
        let (tx, rx) = mpsc::channel();
        let block = RcBlock::new(move |s: SFSpeechRecognizerAuthorizationStatus| {
            let _ = tx.send(s);
        });
        unsafe { SFSpeechRecognizer::requestAuthorization(&block) };
        match rx.recv_timeout(Duration::from_secs(120)) {
            Ok(SFSpeechRecognizerAuthorizationStatus::Authorized) => Ok(()),
            _ => Err("Bạn chưa cho phép nhận dạng giọng nói".into()),
        }
    }

    /// `locale`: "vi-VN" hoặc "en-US"...
    pub fn transcribe_file(path: &str, locale: &str) -> Result<String, String> {
        ensure_access()?;
        unsafe {
            let loc = NSLocale::localeWithLocaleIdentifier(&NSString::from_str(locale));
            let rec = SFSpeechRecognizer::initWithLocale(SFSpeechRecognizer::alloc(), &loc)
                .ok_or_else(|| format!("Máy chưa hỗ trợ nhận dạng {locale}"))?;
            if !rec.isAvailable() {
                return Err("Nhận dạng giọng nói đang không sẵn sàng (kiểm tra kết nối mạng)".into());
            }
            // Kết quả trả về trên hàng đợi riêng → không phụ thuộc main thread
            rec.setQueue(&NSOperationQueue::new());
            let url = NSURL::fileURLWithPath(&NSString::from_str(path));
            let req = SFSpeechURLRecognitionRequest::initWithURL(SFSpeechURLRecognitionRequest::alloc(), &url);
            req.setShouldReportPartialResults(false);
            if req.respondsToSelector(sel!(setAddsPunctuation:)) {
                req.setAddsPunctuation(true);
            }
            let (tx, rx) = mpsc::channel::<Result<String, String>>();
            let block = RcBlock::new(move |result: *mut SFSpeechRecognitionResult, err: *mut NSError| {
                if let Some(r) = result.as_ref() {
                    if r.isFinal() {
                        let _ = tx.send(Ok(r.bestTranscription().formattedString().to_string()));
                    }
                } else if let Some(e) = err.as_ref() {
                    let _ = tx.send(Err(e.localizedDescription().to_string()));
                }
            });
            let _task = rec.recognitionTaskWithRequest_resultHandler(&req, &block);
            match rx.recv_timeout(Duration::from_secs(90)) {
                Ok(Ok(t)) => Ok(t),
                Ok(Err(e)) if e.contains("No speech") || e.contains("1110") => Err("Không nghe thấy gì, thử nói to hơn nhé".into()),
                Ok(Err(e)) => Err(format!("Nhận dạng thất bại: {e}")),
                Err(_) => Err("Nhận dạng quá lâu, thử lại nhé".into()),
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub fn ensure_access() -> Result<(), String> {
        Err("Nhận dạng giọng nói hiện có trên macOS; Windows/Linux sẽ có ở bản sau".into())
    }
    pub fn transcribe_file(_path: &str, _locale: &str) -> Result<String, String> {
        ensure_access().map(|_| String::new())
    }
}

pub use imp::{ensure_access, transcribe_file};
