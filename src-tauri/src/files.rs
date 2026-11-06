//! Công cụ file: nén ZIP, giải nén (zip / tar / tar.gz, rar & 7z qua unar/7z nếu có),
//! nén PDF và gộp PDF (Ghostscript; macOS có phương án dự phòng bằng PDFKit có sẵn).

use serde::Serialize;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize)]
pub struct FileResult {
    pub output: PathBuf,
    pub before: u64,
    pub after: u64,
    pub message: String,
}

/// Tên chưa tồn tại: "a.zip" → "a 2.zip" → "a 3.zip"...
pub fn unique(p: PathBuf) -> PathBuf {
    if !p.exists() {
        return p;
    }
    let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let ext = p.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    let dir = p.parent().map(Path::to_path_buf).unwrap_or_default();
    for i in 2.. {
        let c = dir.join(format!("{stem} {i}{ext}"));
        if !c.exists() {
            return c;
        }
    }
    unreachable!()
}

fn size_of(p: &Path) -> u64 {
    WalkDir::new(p).into_iter().filter_map(|e| e.ok()).filter_map(|e| e.metadata().ok()).filter(|m| m.is_file()).map(|m| m.len()).sum()
}

// ───────────────────────── ZIP ─────────────────────────

pub fn zip_paths(paths: &[PathBuf]) -> Result<FileResult, String> {
    let first = paths.first().ok_or("Chưa chọn file")?;
    let dir = first.parent().ok_or("Đường dẫn không hợp lệ")?;
    let name = if paths.len() == 1 {
        format!("{}.zip", first.file_name().unwrap_or_default().to_string_lossy())
    } else {
        "Lưu trữ.zip".to_string()
    };
    let out = unique(dir.join(name));
    let f = File::create(&out).map_err(|e| format!("Không tạo được file: {e}"))?;
    let mut z = zip::ZipWriter::new(f);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644)
        .large_file(true);
    let mut before = 0u64;
    let mut buf = Vec::new();
    for p in paths {
        let base = p.parent().unwrap_or(Path::new(""));
        for e in WalkDir::new(p).follow_links(false).into_iter().filter_map(|e| e.ok()) {
            let rel = e.path().strip_prefix(base).unwrap_or(e.path());
            let rel_s = rel.to_string_lossy().replace('\\', "/");
            let name = e.file_name().to_string_lossy();
            if name == ".DS_Store" || rel_s.contains("__MACOSX") {
                continue;
            }
            if e.file_type().is_dir() {
                z.add_directory(format!("{rel_s}/"), opts).map_err(|e| e.to_string())?;
            } else if e.file_type().is_file() {
                z.start_file(rel_s, opts).map_err(|e| e.to_string())?;
                buf.clear();
                File::open(e.path()).and_then(|mut f| f.read_to_end(&mut buf)).map_err(|e| e.to_string())?;
                before += buf.len() as u64;
                z.write_all(&buf).map_err(|e| e.to_string())?;
            }
        }
    }
    z.finish().map_err(|e| e.to_string())?;
    let after = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
    Ok(FileResult { output: out, before, after, message: "Đã nén xong".into() })
}

// ───────────────────────── GIẢI NÉN ─────────────────────────

fn archive_kind(p: &Path) -> Option<&'static str> {
    let n = p.file_name()?.to_string_lossy().to_lowercase();
    Some(if n.ends_with(".zip") {
        "zip"
    } else if n.ends_with(".tar.gz") || n.ends_with(".tgz") {
        "tgz"
    } else if n.ends_with(".tar") {
        "tar"
    } else if n.ends_with(".rar") || n.ends_with(".7z") || n.ends_with(".tar.xz") || n.ends_with(".tar.bz2") {
        "external"
    } else {
        return None;
    })
}

pub fn is_archive(p: &Path) -> bool {
    archive_kind(p).is_some()
}

fn strip_archive_ext(n: &str) -> String {
    let l = n.to_lowercase();
    for e in [".tar.gz", ".tar.xz", ".tar.bz2", ".tgz", ".zip", ".tar", ".rar", ".7z"] {
        if l.ends_with(e) {
            return n[..n.len() - e.len()].to_string();
        }
    }
    n.to_string()
}

pub fn extract(p: &Path) -> Result<FileResult, String> {
    let kind = archive_kind(p).ok_or("Không phải file nén được hỗ trợ")?;
    let dir = p.parent().ok_or("Đường dẫn không hợp lệ")?;
    let name = strip_archive_ext(&p.file_name().unwrap_or_default().to_string_lossy());
    let out = unique(dir.join(&name));
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let before = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
    let res = match kind {
        "zip" => extract_zip(p, &out),
        "tgz" => {
            let f = File::open(p).map_err(|e| e.to_string())?;
            extract_tar(flate2::read::GzDecoder::new(f), &out)
        }
        "tar" => extract_tar(File::open(p).map_err(|e| e.to_string())?, &out),
        _ => extract_external(p, &out),
    };
    if let Err(e) = res {
        let _ = std::fs::remove_dir_all(&out);
        return Err(e);
    }
    flatten_single(&out, &name);
    Ok(FileResult { after: size_of(&out), output: out, before, message: "Đã giải nén xong".into() })
}

fn extract_zip(p: &Path, out: &Path) -> Result<(), String> {
    let f = File::open(p).map_err(|e| e.to_string())?;
    let mut a = zip::ZipArchive::new(f).map_err(|_| "File zip bị hỏng hoặc không hợp lệ".to_string())?;
    for i in 0..a.len() {
        let mut entry = a.by_index(i).map_err(|e| e.to_string())?;
        if entry.encrypted() {
            return Err("File zip có mật khẩu — chưa hỗ trợ".into());
        }
        // enclosed_name chặn "zip slip" (../../ thoát ra ngoài thư mục đích)
        let Some(rel) = entry.enclosed_name() else { continue };
        if rel.components().any(|c| c.as_os_str() == "__MACOSX") {
            continue;
        }
        let target = out.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut w = File::create(&target).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut w).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn extract_tar<R: Read>(r: R, out: &Path) -> Result<(), String> {
    let mut a = tar::Archive::new(r);
    a.set_preserve_permissions(false);
    // unpack_in từ chối đường dẫn thoát ra ngoài thư mục đích
    for e in a.entries().map_err(|e| e.to_string())? {
        let mut e = e.map_err(|e| e.to_string())?;
        e.unpack_in(out).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn find_tool(names: &[&str]) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    // App mở từ Finder không có PATH của Terminal → thêm các nơi Homebrew hay cài
    for d in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "C:\\Program Files\\7-Zip", "C:\\Program Files\\gs\\bin"] {
        dirs.push(PathBuf::from(d));
    }
    for n in names {
        for d in &dirs {
            for cand in [d.join(n), d.join(format!("{n}.exe"))] {
                if cand.is_file() {
                    return Some(cand);
                }
            }
        }
    }
    None
}

fn extract_external(p: &Path, out: &Path) -> Result<(), String> {
    if let Some(unar) = find_tool(&["unar"]) {
        let s = Command::new(unar).arg("-q").arg("-f").arg("-D").arg("-o").arg(out).arg(p).status().map_err(|e| e.to_string())?;
        return if s.success() { Ok(()) } else { Err("unar giải nén thất bại".into()) };
    }
    if let Some(sz) = find_tool(&["7zz", "7z"]) {
        let s = Command::new(sz).arg("x").arg("-y").arg(format!("-o{}", out.display())).arg(p).status().map_err(|e| e.to_string())?;
        return if s.success() { Ok(()) } else { Err("7-Zip giải nén thất bại".into()) };
    }
    let n = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if n == "xz" || n == "bz2" {
        let s = Command::new("tar").arg("-xf").arg(p).arg("-C").arg(out).status().map_err(|e| e.to_string())?;
        return if s.success() { Ok(()) } else { Err("tar giải nén thất bại".into()) };
    }
    Err(if cfg!(target_os = "macos") {
        "Cần cài công cụ giải nén RAR/7z: mở Terminal và chạy  brew install unar".into()
    } else {
        "Cần cài 7-Zip hoặc unar để giải nén RAR/7z".into()
    })
}

/// Nếu bên trong chỉ có đúng 1 thư mục cùng tên → kéo nội dung lên, tránh "a/a/..."
fn flatten_single(out: &Path, archive_stem: &str) {
    let Ok(rd) = std::fs::read_dir(out) else { return };
    let items: Vec<_> = rd.filter_map(|e| e.ok()).collect();
    if items.len() != 1 || !items[0].path().is_dir() {
        return;
    }
    let inner = items[0].path();
    if inner.file_name().map(|n| n.to_string_lossy() != archive_stem).unwrap_or(true) {
        return;
    }
    let tmp = out.with_extension("miniasi-tmp");
    if std::fs::rename(&inner, &tmp).is_ok() && std::fs::remove_dir(out).is_ok() {
        let _ = std::fs::rename(&tmp, out);
    }
}

// ───────────────────────── PDF ─────────────────────────

pub fn is_pdf(p: &Path) -> bool {
    p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

fn with_suffix(p: &Path, suffix: &str) -> PathBuf {
    let stem = p.file_stem().unwrap_or_default().to_string_lossy();
    unique(p.with_file_name(format!("{stem}{suffix}.pdf")))
}

/// quality: "screen" (nhỏ nhất) | "ebook" (cân bằng) | "printer" (đẹp)
pub fn compress_pdf(p: &Path, quality: &str) -> Result<FileResult, String> {
    if !is_pdf(p) {
        return Err("Không phải file PDF".into());
    }
    let q = match quality {
        "screen" | "ebook" | "printer" => quality,
        _ => "ebook",
    };
    let out = with_suffix(p, "-nen");
    let before = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
    let engine;
    if let Some(gs) = find_tool(&["gs", "gswin64c", "gswin32c"]) {
        let s = Command::new(gs)
            .args(["-sDEVICE=pdfwrite", "-dCompatibilityLevel=1.5", "-dNOPAUSE", "-dQUIET", "-dBATCH", "-dDetectDuplicateImages=true"])
            .arg(format!("-dPDFSETTINGS=/{q}"))
            .arg(format!("-sOutputFile={}", out.display()))
            .arg(p)
            .status()
            .map_err(|e| e.to_string())?;
        if !s.success() {
            let _ = std::fs::remove_file(&out);
            return Err("Ghostscript nén PDF thất bại".into());
        }
        engine = "Ghostscript";
    } else if cfg!(target_os = "macos") {
        run_jxa(JXA_COMPRESS, &[p.as_os_str(), out.as_os_str()])?;
        engine = "macOS";
    } else {
        return Err("Cần cài Ghostscript để nén PDF".into());
    }
    let after = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
    if after == 0 || after >= before {
        let _ = std::fs::remove_file(&out);
        return Ok(FileResult { output: p.to_path_buf(), before, after: before, message: "File đã được tối ưu sẵn, không nhỏ hơn được nữa".into() });
    }
    let pct = 100 - (after * 100 / before.max(1));
    Ok(FileResult { output: out, before, after, message: format!("Giảm {pct}% ({engine})") })
}

pub fn merge_pdfs(paths: &[PathBuf]) -> Result<FileResult, String> {
    if paths.len() < 2 || !paths.iter().all(|p| is_pdf(p)) {
        return Err("Cần ít nhất 2 file PDF".into());
    }
    let out = with_suffix(&paths[0], "-gop");
    let before: u64 = paths.iter().filter_map(|p| std::fs::metadata(p).ok()).map(|m| m.len()).sum();
    if let Some(gs) = find_tool(&["gs", "gswin64c", "gswin32c"]) {
        let s = Command::new(gs)
            .args(["-sDEVICE=pdfwrite", "-dNOPAUSE", "-dQUIET", "-dBATCH"])
            .arg(format!("-sOutputFile={}", out.display()))
            .args(paths)
            .status()
            .map_err(|e| e.to_string())?;
        if !s.success() {
            return Err("Ghostscript gộp PDF thất bại".into());
        }
    } else if cfg!(target_os = "macos") {
        let mut args: Vec<&std::ffi::OsStr> = vec![out.as_os_str()];
        args.extend(paths.iter().map(|p| p.as_os_str()));
        run_jxa(JXA_MERGE, &args)?;
    } else {
        return Err("Cần cài Ghostscript để gộp PDF".into());
    }
    let after = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
    Ok(FileResult { output: out, before, after, message: format!("Đã gộp {} file", paths.len()) })
}

/// macOS: chạy JavaScript for Automation dùng PDFKit + Quartz Filter có sẵn trong hệ thống.
fn run_jxa(script: &str, args: &[&std::ffi::OsStr]) -> Result<(), String> {
    let o = Command::new("osascript").args(["-l", "JavaScript", "-e", script]).args(args).output().map_err(|e| e.to_string())?;
    let out = String::from_utf8_lossy(&o.stdout);
    if o.status.success() && out.trim() == "ok" {
        Ok(())
    } else {
        Err(format!("macOS không xử lý được PDF: {}", String::from_utf8_lossy(&o.stderr).trim()))
    }
}

const JXA_COMPRESS: &str = r#"
ObjC.import('Quartz');
function run(argv) {
  const doc = $.PDFDocument.alloc.initWithURL($.NSURL.fileURLWithPath(argv[0]));
  if (!doc || doc.isNil()) return 'fail';
  const filter = $.QuartzFilter.quartzFilterWithURL($.NSURL.fileURLWithPath('/System/Library/Filters/Reduce File Size.qfilter'));
  const opts = $.NSDictionary.dictionaryWithObjectForKey(filter, 'QuartzFilter');
  return doc.writeToURLWithOptions($.NSURL.fileURLWithPath(argv[1]), opts) ? 'ok' : 'fail';
}
"#;

const JXA_MERGE: &str = r#"
ObjC.import('Quartz');
function run(argv) {
  const out = $.PDFDocument.alloc.init;
  let idx = 0;
  for (let i = 1; i < argv.length; i++) {
    const d = $.PDFDocument.alloc.initWithURL($.NSURL.fileURLWithPath(argv[i]));
    if (!d || d.isNil()) return 'fail';
    for (let p = 0; p < d.pageCount; p++) { out.insertPageAtIndex(d.pageAtIndex(p), idx++); }
  }
  return out.writeToFile(argv[0]) ? 'ok' : 'fail';
}
"#;

/// Mở Finder / Explorer tại file.
pub fn reveal(p: &Path) {
    #[cfg(target_os = "macos")]
    let _ = Command::new("open").arg("-R").arg(p).spawn();
    #[cfg(target_os = "windows")]
    let _ = Command::new("explorer").arg(format!("/select,{}", p.display())).spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = Command::new("xdg-open").arg(p.parent().unwrap_or(p)).spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zip_and_extract_roundtrip() {
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("BaoCao");
        std::fs::create_dir_all(src.join("anh")).unwrap();
        std::fs::write(src.join("a.txt"), "xin chào ".repeat(1000)).unwrap();
        std::fs::write(src.join("anh/b.txt"), "b").unwrap();
        let z = zip_paths(&[src.clone()]).unwrap();
        assert!(z.output.ends_with("BaoCao.zip"));
        assert!(z.after < z.before, "nén phải nhỏ hơn");
        // giải nén ra thư mục mới (BaoCao đã tồn tại → "BaoCao 2"), không lồng BaoCao/BaoCao
        let x = extract(&z.output).unwrap();
        assert!(x.output.ends_with("BaoCao 2"), "{:?}", x.output);
        assert_eq!(std::fs::read_to_string(x.output.join("anh/b.txt")).unwrap(), "b");
        // nén lần 2 → tên không trùng
        let z2 = zip_paths(&[src]).unwrap();
        assert!(z2.output.ends_with("BaoCao 2.zip"));
    }

    #[test]
    fn zip_slip_is_blocked() {
        let d = tempfile::tempdir().unwrap();
        let evil = d.path().join("evil.zip");
        {
            let mut z = zip::ZipWriter::new(File::create(&evil).unwrap());
            z.start_file("../../thoat-ra.txt", zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(b"x").unwrap();
            z.start_file("ok.txt", zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(b"y").unwrap();
            z.finish().unwrap();
        }
        let r = extract(&evil).unwrap();
        assert!(r.output.join("ok.txt").exists());
        assert!(!d.path().parent().unwrap().join("thoat-ra.txt").exists());
    }

    #[test]
    fn tgz_extract() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("data.tar.gz");
        {
            let gz = flate2::write::GzEncoder::new(File::create(&p).unwrap(), flate2::Compression::default());
            let mut t = tar::Builder::new(gz);
            let mut h = tar::Header::new_gnu();
            h.set_size(3);
            h.set_mode(0o644);
            h.set_cksum();
            t.append_data(&mut h, "x/hi.txt", &b"abc"[..]).unwrap();
            t.into_inner().unwrap().finish().unwrap();
        }
        let r = extract(&p).unwrap();
        assert!(r.output.ends_with("data"));
        assert_eq!(std::fs::read_to_string(r.output.join("x/hi.txt")).unwrap(), "abc");
    }

    #[test]
    fn pdf_compress_with_ghostscript_if_available() {
        if find_tool(&["gs"]).is_none() {
            eprintln!("bỏ qua: không có gs");
            return;
        }
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("in.pdf");
        // PDF tối thiểu hợp lệ
        std::fs::write(&p, MINI_PDF).unwrap();
        let r = compress_pdf(&p, "screen").unwrap();
        assert!(r.after <= r.before);
        let m = merge_pdfs(&[p.clone(), p.clone()]).unwrap();
        assert!(m.output.exists());
    }

    const MINI_PDF: &str = "%PDF-1.4\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 200 200]>>endobj\ntrailer<</Root 1 0 R>>\n%%EOF\n";
}
