//! Đọc chữ trong ảnh ngay trên máy (macOS: Vision — VNRecognizeTextRequest). Dùng khi người dùng kéo ảnh /
//! ảnh chụp màn hình vào tai thỏ để tạo việc hoặc lịch. Không gửi ảnh đi đâu.

#[cfg(target_os = "macos")]
mod imp {
    use objc2::AnyThread;
    use objc2_foundation::{NSArray, NSData, NSDictionary, NSString};
    use objc2_vision::{VNImageRequestHandler, VNRecognizeTextRequest, VNRequest, VNRequestTextRecognitionLevel};

    fn run(bytes: &[u8], langs: Option<&[&str]>) -> Result<String, String> {
        let data = NSData::with_bytes(bytes);
        let req = VNRecognizeTextRequest::new();
        req.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
        req.setUsesLanguageCorrection(true);
        if let Some(l) = langs {
            let ls: Vec<_> = l.iter().map(|s| NSString::from_str(s)).collect();
            req.setRecognitionLanguages(&NSArray::from_retained_slice(&ls));
        }
        let handler = VNImageRequestHandler::initWithData_options(VNImageRequestHandler::alloc(), &data, &NSDictionary::new());
        let r: &VNRequest = &req;
        handler.performRequests_error(&NSArray::from_slice(&[r])).map_err(|e| format!("Không đọc được chữ trong ảnh: {}", e.localizedDescription()))?;
        let mut lines = vec![];
        if let Some(results) = req.results() {
            for obs in results.iter() {
                if let Some(best) = obs.topCandidates(1).firstObject() {
                    lines.push(best.string().to_string());
                }
            }
        }
        Ok(lines.join("\n"))
    }

    /// Ảnh (PNG/JPEG/TIFF/HEIC…) → chữ, mỗi dòng một dòng. Ưu tiên tiếng Việt + tiếng Anh;
    /// macOS cũ chưa có tiếng Việt → đọc theo ngôn ngữ mặc định.
    pub fn recognize_text(bytes: &[u8]) -> Result<String, String> {
        match run(bytes, Some(&["vi-VT", "en-US"])) {
            Ok(t) if !t.trim().is_empty() => Ok(t),
            _ => run(bytes, None),
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub fn recognize_text(_bytes: &[u8]) -> Result<String, String> {
        Err("Đọc chữ trong ảnh hiện chỉ có trên macOS".into())
    }
}

pub use imp::recognize_text;
