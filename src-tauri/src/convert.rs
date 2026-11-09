//! Đổi định dạng file khi kéo vào tai thỏ (nửa phải → "Công cụ file"):
//! - **Chữ → PDF**: .txt .md .csv .log .json .html .docx .rtf .doc .odt .pages(không) → PDF khổ A4, font Unicode của máy
//!   (tiếng Việt hiển thị đúng, chọn & tìm chữ được). Markdown: tiêu đề to/đậm, gạch đầu dòng. Bộ ghi PDF tự viết ở dưới
//!   (nhúng font TrueType, mã hoá Identity-H + ToUnicode) — không cần thư viện ngoài.
//! - **Ảnh ↔ ảnh**: JPG · PNG · HEIC · TIFF · PDF (nhiều ảnh → gộp 1 PDF) bằng `sips` có sẵn trên macOS.
//! - Kéo cả **thư mục**: lấy mọi file phù hợp bên trong (kể cả thư mục con), kết quả ghi vào thư mục mới cạnh thư mục gốc
//!   ("Ảnh chuyến đi - JPG"), giữ nguyên cấu trúc. Kéo file lẻ → kết quả nằm ngay cạnh file gốc.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::files::{unique, FileResult};

pub const IMAGE_EXT: &[&str] = &["png", "jpg", "jpeg", "heic", "heif", "tif", "tiff", "gif", "bmp", "webp"];
pub const TEXT_EXT: &[&str] = &["txt", "text", "md", "markdown", "csv", "log", "json", "html", "htm", "docx", "rtf", "doc", "odt"];
const MAX_FILES: usize = 2000;

fn ext(p: &Path) -> String {
    p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}
pub fn is_image(p: &Path) -> bool {
    IMAGE_EXT.contains(&ext(p).as_str())
}
pub fn is_text(p: &Path) -> bool {
    TEXT_EXT.contains(&ext(p).as_str())
}

/// Mở rộng danh sách kéo vào: file → (file, thư mục kết quả = thư mục chứa nó); thư mục → mọi file thoả `keep` bên trong,
/// kết quả vào "<tên thư mục> - <nhãn>" cạnh thư mục gốc, giữ cấu trúc thư mục con.
pub fn expand(paths: &[PathBuf], keep: impl Fn(&Path) -> bool, label: &str) -> Vec<(PathBuf, PathBuf)> {
    let mut out = vec![];
    for p in paths {
        if p.is_dir() {
            let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Thư mục".into());
            let parent = p.parent().map(Path::to_path_buf).unwrap_or_default();
            let dest_root = parent.join(format!("{name} - {label}"));
            for e in walkdir::WalkDir::new(p).into_iter().filter_entry(|e| !e.file_name().to_string_lossy().starts_with('.')).filter_map(|e| e.ok()) {
                if e.file_type().is_file() && keep(e.path()) {
                    let rel = e.path().parent().and_then(|d| d.strip_prefix(p).ok()).map(Path::to_path_buf).unwrap_or_default();
                    out.push((e.path().to_path_buf(), dest_root.join(rel)));
                    if out.len() >= MAX_FILES {
                        return out;
                    }
                }
            }
        } else if p.is_file() && keep(p) {
            out.push((p.clone(), p.parent().map(Path::to_path_buf).unwrap_or_default()));
        }
    }
    out
}

/// Đếm ảnh / file chữ (kể cả trong thư mục) để tấm công cụ biết hiện nút nào
pub fn count_kinds(paths: &[PathBuf]) -> (usize, usize) {
    (expand(paths, is_image, "").len(), expand(paths, is_text, "").len())
}

fn total_size(v: &[PathBuf]) -> u64 {
    v.iter().filter_map(|p| std::fs::metadata(p).ok()).map(|m| m.len()).sum()
}

fn result(outputs: Vec<PathBuf>, before: u64, what: &str, errors: Vec<String>) -> Result<FileResult, String> {
    if outputs.is_empty() {
        return Err(errors.into_iter().next().unwrap_or_else(|| "Không có file nào phù hợp".into()));
    }
    let after = total_size(&outputs);
    let output = outputs[0].clone(); // nút "Mở thư mục" chọn file đầu tiên trong Finder
    let mut message = if outputs.len() == 1 { format!("Đã chuyển sang {what}") } else { format!("Đã chuyển {} file sang {what}", outputs.len()) };
    if !errors.is_empty() {
        message.push_str(&format!(" · {} file lỗi", errors.len()));
    }
    Ok(FileResult { output, before, after, message })
}

// ───────────────────────── ảnh ─────────────────────────

/// Định dạng đích → (đuôi file, tên định dạng của sips)
pub fn image_target(fmt: &str) -> Option<(&'static str, &'static str)> {
    match fmt.to_lowercase().as_str() {
        "jpg" | "jpeg" => Some(("jpg", "jpeg")),
        "png" => Some(("png", "png")),
        "heic" => Some(("heic", "heic")),
        "tiff" | "tif" => Some(("tiff", "tiff")),
        "pdf" => Some(("pdf", "pdf")),
        _ => None,
    }
}

fn same_format(src: &Path, target_ext: &str) -> bool {
    let e = ext(src);
    e == target_ext || (target_ext == "jpg" && e == "jpeg") || (target_ext == "tiff" && e == "tif") || (target_ext == "heic" && e == "heif")
}

/// Đổi định dạng ảnh. `merge_pdf` (khi đích là PDF và có nhiều ảnh): gộp tất cả thành 1 file PDF.
pub fn convert_images(paths: &[PathBuf], fmt: &str, merge_pdf: bool) -> Result<FileResult, String> {
    let (target_ext, sips_fmt) = image_target(fmt).ok_or("Định dạng chưa hỗ trợ")?;
    if !cfg!(target_os = "macos") {
        return Err("Đổi định dạng ảnh hiện có trên macOS".into());
    }
    let label = target_ext.to_uppercase();
    let items = expand(paths, is_image, &label);
    if items.is_empty() {
        return Err("Không có ảnh nào để chuyển".into());
    }
    let before = total_size(&items.iter().map(|x| x.0.clone()).collect::<Vec<_>>());
    let mut outs = vec![];
    let mut errs = vec![];
    let merging = target_ext == "pdf" && merge_pdf && items.len() > 1;
    let tmp = std::env::temp_dir().join(format!("mini-asi-img-{}", std::process::id()));
    if merging {
        let _ = std::fs::create_dir_all(&tmp);
    }
    for (i, (src, dir)) in items.iter().enumerate() {
        if same_format(src, target_ext) && !merging {
            continue; // đã đúng định dạng
        }
        let stem = src.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "anh".into());
        let out = if merging { tmp.join(format!("{i:05}.pdf")) } else {
            let _ = std::fs::create_dir_all(dir);
            unique(dir.join(format!("{stem}.{target_ext}")))
        };
        let mut c = Command::new("/usr/bin/sips");
        c.args(["-s", "format", sips_fmt]);
        if target_ext == "jpg" || target_ext == "heic" {
            c.args(["-s", "formatOptions", "high"]);
        }
        let ok = c.arg(src).arg("--out").arg(&out).output().map(|o| o.status.success()).unwrap_or(false);
        if ok && out.exists() {
            outs.push(out);
        } else {
            errs.push(format!("Không chuyển được {}", src.display()));
        }
    }
    if merging {
        if outs.is_empty() {
            return Err(errs.into_iter().next().unwrap_or_else(|| "Không chuyển được ảnh".into()));
        }
        let first = &items[0];
        let name = if paths.len() == 1 && paths[0].is_dir() { paths[0].file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default() } else { first.0.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default() };
        let base = if paths.len() == 1 && paths[0].is_dir() { paths[0].parent().map(Path::to_path_buf).unwrap_or_default() } else { first.1.clone() };
        let dest = unique(base.join(format!("{name} - {} ảnh.pdf", outs.len())));
        // gộp các trang PDF tạm (merge_pdfs đặt tên "-gop" cạnh file đầu → đổi tên về đích)
        let r = crate::files::merge_pdfs(&outs);
        let r = match r {
            Ok(r) => {
                // thư mục tạm có thể ở ổ khác → chép rồi xoá thay vì đổi tên
                std::fs::copy(&r.output, &dest).map_err(|e| e.to_string())?;
                let _ = std::fs::remove_file(&r.output);
                Ok(FileResult { output: dest, before, after: r.after, message: format!("Đã gộp {} ảnh thành 1 PDF", outs.len()) })
            }
            Err(e) => Err(e),
        };
        let _ = std::fs::remove_dir_all(&tmp);
        return r;
    }
    if outs.is_empty() && errs.is_empty() {
        return Err(format!("Ảnh đã là {label} rồi"));
    }
    result(outs, before, &label, errs)
}

// ───────────────────────── chữ → PDF ─────────────────────────

/// Đọc chữ từ file: UTF-8 / UTF-16; .docx đọc trực tiếp; .rtf .doc .odt .html dùng `textutil` của macOS
pub fn read_text(p: &Path) -> Result<String, String> {
    let e = ext(p);
    if cfg!(target_os = "macos") && ["rtf", "doc", "odt", "docx", "html", "htm"].contains(&e.as_str()) {
        if let Ok(o) = Command::new("/usr/bin/textutil").args(["-convert", "txt", "-encoding", "UTF-8", "-stdout"]).arg(p).output() {
            if o.status.success() && !o.stdout.is_empty() {
                return Ok(String::from_utf8_lossy(&o.stdout).to_string());
            }
        }
    }
    match e.as_str() {
        "docx" => docx_text(p),
        "html" | "htm" => Ok(html_text(&decode(&std::fs::read(p).map_err(|e| e.to_string())?))),
        "rtf" | "doc" | "odt" => Err("Cần macOS để đọc file này".into()),
        _ => Ok(decode(&std::fs::read(p).map_err(|e| format!("Không đọc được file: {e}"))?)),
    }
}

fn decode(b: &[u8]) -> String {
    if b.starts_with(&[0xFF, 0xFE]) || b.starts_with(&[0xFE, 0xFF]) {
        let le = b[0] == 0xFF;
        let u: Vec<u16> = b[2..].chunks(2).filter(|c| c.len() == 2).map(|c| if le { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) }).collect();
        return String::from_utf16_lossy(&u);
    }
    String::from_utf8_lossy(b.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(b)).to_string()
}

fn xml_unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&#39;", "'").replace("&nbsp;", " ").replace("&amp;", "&")
}

/// .docx: word/document.xml → mỗi <w:p> một dòng, chữ trong <w:t>, <w:tab/> → tab
pub fn docx_text(p: &Path) -> Result<String, String> {
    let f = std::fs::File::open(p).map_err(|e| e.to_string())?;
    let mut z = zip::ZipArchive::new(f).map_err(|_| "File .docx hỏng".to_string())?;
    let mut xml = String::new();
    std::io::Read::read_to_string(&mut z.by_name("word/document.xml").map_err(|_| "File .docx hỏng".to_string())?, &mut xml).map_err(|e| e.to_string())?;
    Ok(docx_xml_text(&xml))
}

pub fn docx_xml_text(xml: &str) -> String {
    let mut out = String::new();
    for para in xml.split("</w:p>") {
        let mut line = String::new();
        let mut rest = para;
        while let Some(i) = rest.find('<') {
            let Some(j) = rest[i..].find('>') else { break };
            let tag = &rest[i + 1..i + j];
            rest = &rest[i + j + 1..];
            if tag == "w:t" || tag.starts_with("w:t ") {
                if let Some(k) = rest.find("</w:t>") {
                    line.push_str(&xml_unescape(&rest[..k]));
                    rest = &rest[k + 6..];
                }
            } else if tag.starts_with("w:tab") {
                line.push('\t');
            } else if tag.starts_with("w:br") {
                line.push('\n');
            }
        }
        if para.contains("<w:p") {
            out.push_str(&line);
            out.push('\n');
        }
    }
    out.trim_end().to_string()
}

/// HTML → chữ (đủ dùng khi không có textutil): bỏ script/style, xuống dòng ở thẻ khối
pub fn html_text(h: &str) -> String {
    let mut out = String::new();
    let mut rest = h;
    while let Some(i) = rest.find('<') {
        out.push_str(&rest[..i]);
        let Some(j) = rest[i..].find('>') else { break };
        let tag = rest[i + 1..i + j].trim_start_matches('/').split([' ', '/']).next().unwrap_or("").to_lowercase();
        rest = &rest[i + j + 1..];
        if tag == "script" || tag == "style" {
            if let Some(k) = rest.to_lowercase().find(&format!("</{tag}")) {
                rest = &rest[k..];
                if let Some(m) = rest.find('>') {
                    rest = &rest[m + 1..];
                }
            }
            continue;
        }
        if ["p", "div", "br", "li", "h1", "h2", "h3", "h4", "tr", "section", "article"].contains(&tag.as_str()) {
            out.push('\n');
            if tag == "li" {
                out.push_str("- ");
            }
        }
    }
    out.push_str(rest);
    let t = xml_unescape(&out);
    t.lines().map(str::trim).collect::<Vec<_>>().join("\n").split("\n\n\n").collect::<Vec<_>>().join("\n\n").trim().to_string()
}

/// Một khối chữ trên trang
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub text: String,
    pub size: f32,
    pub bold: bool,
    /// thụt đầu dòng (pt) — gạch đầu dòng
    pub indent: f32,
    pub bullet: bool,
    /// khoảng trống trước khối (pt)
    pub gap: f32,
}

fn strip_inline_md(s: &str) -> String {
    let mut t = s.replace("**", "").replace("__", "").replace('`', "");
    // [chữ](link) → chữ (link)
    while let (Some(a), Some(b)) = (t.find("]("), t.find(')')) {
        if b < a {
            break;
        }
        let Some(open) = t[..a].rfind('[') else { break };
        let label = t[open + 1..a].to_string();
        let url = t[a + 2..b].to_string();
        t = format!("{}{} ({url}){}", &t[..open], label, &t[b + 1..]);
    }
    t
}

/// Chữ thường / Markdown → các khối có kiểu
pub fn blocks(text: &str, markdown: bool, body: f32) -> Vec<Block> {
    use unicode_normalization::UnicodeNormalization;
    let text: String = text.nfc().collect::<String>().replace('\r', "");
    let mut out = vec![];
    let mut gap = 0.0;
    let mut in_code = false;
    for raw in text.lines() {
        let line = raw.trim_end();
        if markdown && line.trim_start().starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if line.trim().is_empty() {
            gap = body * 0.7;
            continue;
        }
        let mut b = Block { text: line.replace('\t', "    "), size: body, bold: false, indent: 0.0, bullet: false, gap };
        gap = 0.0;
        if markdown && !in_code {
            let t = line.trim_start();
            let hashes = t.chars().take_while(|c| *c == '#').count();
            if (1..=6).contains(&hashes) && t[hashes..].starts_with(' ') {
                b.text = strip_inline_md(t[hashes..].trim());
                b.size = match hashes {
                    1 => body * 1.7,
                    2 => body * 1.4,
                    3 => body * 1.2,
                    _ => body * 1.05,
                };
                b.bold = true;
                b.gap = b.gap.max(body * if hashes <= 2 { 1.0 } else { 0.6 });
            } else if let Some(r) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")).or_else(|| t.strip_prefix("+ ")) {
                let depth = (line.len() - t.len()) as f32 / 2.0;
                b.text = strip_inline_md(r.trim_start_matches("[ ] ").trim_start_matches("[x] "));
                b.bullet = true;
                b.indent = 14.0 + depth * 12.0;
            } else if t.starts_with("> ") {
                b.text = strip_inline_md(&t[2..]);
                b.indent = 14.0;
            } else if t.chars().all(|c| c == '-' || c == '*' || c == '_') && t.len() >= 3 {
                gap = body;
                continue;
            } else {
                b.text = strip_inline_md(t);
                if t.starts_with("**") && t.ends_with("**") {
                    b.bold = true;
                }
            }
        }
        out.push(b);
    }
    out
}

/// Ngắt dòng theo độ rộng (`measure` trả về độ rộng pt của chuỗi). Từ dài hơn cả dòng thì cắt theo ký tự.
pub fn wrap(text: &str, max_w: f32, measure: &impl Fn(&str) -> f32) -> Vec<String> {
    let mut lines = vec![];
    let mut cur = String::new();
    for word in text.split(' ') {
        let cand = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
        if measure(&cand) <= max_w {
            cur = cand;
            continue;
        }
        if !cur.is_empty() {
            lines.push(std::mem::take(&mut cur));
        }
        if measure(word) <= max_w {
            cur = word.to_string();
        } else {
            for ch in word.chars() {
                let c2 = format!("{cur}{ch}");
                if measure(&c2) > max_w && !cur.is_empty() {
                    lines.push(std::mem::take(&mut cur));
                }
                cur.push(ch);
            }
        }
    }
    if !cur.is_empty() || lines.is_empty() {
        lines.push(cur);
    }
    lines
}

// ───────────────────────── bộ ghi PDF tối giản ─────────────────────────

/// Font TrueType của máy có đủ chữ tiếng Việt (bản thường, bản đậm nếu có)
pub fn system_fonts() -> Option<(PathBuf, Option<PathBuf>)> {
    let pairs: &[(&str, &str)] = &[
        ("/System/Library/Fonts/Supplemental/Arial.ttf", "/System/Library/Fonts/Supplemental/Arial Bold.ttf"),
        ("/Library/Fonts/Arial.ttf", "/Library/Fonts/Arial Bold.ttf"),
        ("/System/Library/Fonts/Supplemental/Times New Roman.ttf", "/System/Library/Fonts/Supplemental/Times New Roman Bold.ttf"),
        ("C:\\Windows\\Fonts\\arial.ttf", "C:\\Windows\\Fonts\\arialbd.ttf"),
        ("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"),
        ("/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf", "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf"),
    ];
    pairs.iter().find(|(r, _)| Path::new(r).is_file()).map(|(r, b)| (PathBuf::from(r), Some(PathBuf::from(b)).filter(|p| p.is_file())))
}

struct PdfFont {
    data: Vec<u8>,
    upem: f32,
    ascent: i16,
    descent: i16,
    cap: i16,
    bbox: [i16; 4],
    name: String,
    /// ký tự → (glyph id, độ rộng theo đơn vị font)
    map: BTreeMap<char, (u16, u16)>,
    used: BTreeMap<u16, (u16, char)>,
}

impl PdfFont {
    fn load(path: &Path) -> Result<Self, String> {
        let data = std::fs::read(path).map_err(|e| format!("Không đọc được font: {e}"))?;
        let face = ttf_parser::Face::parse(&data, 0).map_err(|_| "Font hỏng".to_string())?;
        let bb = face.global_bounding_box();
        let name: String = path.file_stem().map(|s| s.to_string_lossy().chars().filter(|c| c.is_ascii_alphanumeric()).collect()).unwrap_or_else(|| "Font".into());
        Ok(PdfFont {
            upem: face.units_per_em() as f32,
            ascent: face.ascender(),
            descent: face.descender(),
            cap: face.capital_height().unwrap_or(face.ascender()),
            bbox: [bb.x_min, bb.y_min, bb.x_max, bb.y_max],
            name,
            map: BTreeMap::new(),
            used: BTreeMap::new(),
            data,
        })
    }
    fn glyph(&mut self, c: char) -> (u16, u16) {
        if let Some(g) = self.map.get(&c) {
            return *g;
        }
        let face = ttf_parser::Face::parse(&self.data, 0).expect("font đã kiểm tra");
        let gid = face.glyph_index(c).or_else(|| face.glyph_index('?')).map(|g| g.0).unwrap_or(0);
        let w = face.glyph_hor_advance(ttf_parser::GlyphId(gid)).unwrap_or((self.upem / 2.0) as u16);
        self.map.insert(c, (gid, w));
        (gid, w)
    }
    fn width(&mut self, s: &str, size: f32) -> f32 {
        s.chars().map(|c| self.glyph(c).1 as f32).sum::<f32>() * size / self.upem
    }
    fn encode(&mut self, s: &str) -> String {
        let mut h = String::with_capacity(s.len() * 4);
        for c in s.chars() {
            let (g, w) = self.glyph(c);
            self.used.entry(g).or_insert((w, c));
            h.push_str(&format!("{g:04X}"));
        }
        h
    }
}

/// Văn bản → PDF A4 (byte). `bold_font` = None thì tiêu đề dùng font thường.
pub fn text_pdf(title: &str, blocks: &[Block], regular: &Path, bold: Option<&Path>) -> Result<Vec<u8>, String> {
    let mut fonts = vec![PdfFont::load(regular)?];
    // chỉ nhúng font đậm khi có tiêu đề (mỗi font ~0,5–1 MB)
    if let Some(b) = bold.filter(|_| blocks.iter().any(|x| x.bold)) {
        fonts.push(PdfFont::load(b)?);
    }
    let (pw, ph, margin) = (595.28f32, 841.89f32, 56.0f32);
    let mut pages: Vec<String> = vec![];
    let mut cur = String::new();
    let mut y = ph - margin;
    let new_page = |pages: &mut Vec<String>, cur: &mut String, y: &mut f32| {
        pages.push(std::mem::take(cur));
        *y = ph - margin;
    };
    for b in blocks {
        let fi = if b.bold && fonts.len() > 1 { 1 } else { 0 };
        let lh = b.size * 1.45;
        let x0 = margin + b.indent;
        let maxw = pw - margin - x0;
        let lines = {
            let f = &mut fonts[fi];
            let size = b.size;
            let cell = std::cell::RefCell::new(f);
            wrap(&b.text, maxw, &|s: &str| cell.borrow_mut().width(s, size))
        };
        y -= b.gap;
        for (i, l) in lines.iter().enumerate() {
            if y - lh < margin {
                new_page(&mut pages, &mut cur, &mut y);
            }
            y -= lh;
            if b.bullet && i == 0 {
                let hex = fonts[0].encode("•");
                cur.push_str(&format!("BT /F1 {:.1} Tf {:.2} {:.2} Td <{hex}> Tj ET\n", b.size, x0 - 11.0, y + lh * 0.25));
            }
            let hex = fonts[fi].encode(l);
            cur.push_str(&format!("BT /F{} {:.1} Tf {:.2} {:.2} Td <{hex}> Tj ET\n", fi + 1, b.size, x0, y + lh * 0.25));
        }
    }
    if !cur.is_empty() || pages.is_empty() {
        pages.push(cur);
    }
    Ok(write_pdf(title, &pages, &mut fonts, pw, ph))
}

fn write_pdf(title: &str, pages: &[String], fonts: &mut [PdfFont], pw: f32, ph: f32) -> Vec<u8> {
    // Số đối tượng: 1 Catalog, 2 Pages, 3 Info, rồi mỗi font 5 đối tượng, rồi mỗi trang 2 đối tượng
    let font_base = 4;
    let page_base = font_base + fonts.len() * 5;
    let mut objs: Vec<Vec<u8>> = vec![];
    let kids: Vec<String> = (0..pages.len()).map(|i| format!("{} 0 R", page_base + i * 2)).collect();
    objs.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
    objs.push(format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.join(" "), pages.len()).into_bytes());
    let t16: String = std::iter::once(0xFEFFu16).chain(title.encode_utf16()).map(|u| format!("{u:04X}")).collect();
    objs.push(format!("<< /Title <{t16}> /Producer (Mini-Asi) >>").into_bytes());
    let mut font_refs = String::new();
    for (k, f) in fonts.iter().enumerate() {
        let id = font_base + k * 5;
        font_refs.push_str(&format!("/F{} {} 0 R ", k + 1, id));
        let scale = 1000.0 / f.upem;
        let s = |v: i16| (v as f32 * scale).round() as i32;
        let base = format!("MASI{}+{}", ["AAAA", "AAAB", "AAAC"][k.min(2)], f.name);
        // Type0
        objs.push(format!("<< /Type /Font /Subtype /Type0 /BaseFont /{base} /Encoding /Identity-H /DescendantFonts [{} 0 R] /ToUnicode {} 0 R >>", id + 1, id + 4).into_bytes());
        // CIDFont
        let widths: String = f.used.iter().map(|(g, (w, _))| format!("{g} [{}]", (*w as f32 * scale).round() as i32)).collect::<Vec<_>>().join(" ");
        objs.push(format!("<< /Type /Font /Subtype /CIDFontType2 /BaseFont /{base} /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /FontDescriptor {} 0 R /DW 500 /W [{widths}] /CIDToGIDMap /Identity >>", id + 2).into_bytes());
        // FontDescriptor
        objs.push(format!("<< /Type /FontDescriptor /FontName /{base} /Flags 32 /FontBBox [{} {} {} {}] /ItalicAngle 0 /Ascent {} /Descent {} /CapHeight {} /StemV 80 /FontFile2 {} 0 R >>", s(f.bbox[0]), s(f.bbox[1]), s(f.bbox[2]), s(f.bbox[3]), s(f.ascent), s(f.descent), s(f.cap), id + 3).into_bytes());
        // FontFile2 (nén)
        let mut enc = flate2::write::ZlibEncoder::new(vec![], flate2::Compression::default());
        let _ = enc.write_all(&f.data);
        let z = enc.finish().unwrap_or_default();
        let mut o = format!("<< /Length {} /Length1 {} /Filter /FlateDecode >>\nstream\n", z.len(), f.data.len()).into_bytes();
        o.extend_from_slice(&z);
        o.extend_from_slice(b"\nendstream");
        objs.push(o);
        // ToUnicode
        let mut cmap = String::from("/CIDInit /ProcSet findresource begin 12 dict begin begincmap /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def /CMapName /Adobe-Identity-UCS def /CMapType 2 def 1 begincodespacerange <0000> <FFFF> endcodespacerange\n");
        let pairs: Vec<(&u16, &(u16, char))> = f.used.iter().collect();
        for chunk in pairs.chunks(100) {
            cmap.push_str(&format!("{} beginbfchar\n", chunk.len()));
            for (g, (_, c)) in chunk {
                let u: String = c.encode_utf16(&mut [0u16; 2]).iter().map(|x| format!("{x:04X}")).collect();
                cmap.push_str(&format!("<{g:04X}> <{u}>\n"));
            }
            cmap.push_str("endbfchar\n");
        }
        cmap.push_str("endcmap CMapName currentdict /CMap defineresource pop end end");
        objs.push(format!("<< /Length {} >>\nstream\n{cmap}\nendstream", cmap.len()).into_bytes());
    }
    for (i, content) in pages.iter().enumerate() {
        let pid = page_base + i * 2;
        objs.push(format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {pw:.2} {ph:.2}] /Resources << /Font << {font_refs}>> >> /Contents {} 0 R >>", pid + 1).into_bytes());
        objs.push(format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()).into_bytes());
    }
    let mut out = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offs = vec![];
    for (i, o) in objs.iter().enumerate() {
        offs.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(o);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for o in offs {
        out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R /Info 3 0 R >>\nstartxref\n{xref}\n%%EOF\n", objs.len() + 1).as_bytes());
    out
}

/// Các file chữ (hoặc thư mục chứa chúng) → PDF
pub fn text_to_pdf(paths: &[PathBuf]) -> Result<FileResult, String> {
    let items = expand(paths, is_text, "PDF");
    if items.is_empty() {
        return Err("Không có file chữ nào để chuyển".into());
    }
    let (regular, bold) = system_fonts().ok_or("Không tìm thấy font Unicode trên máy")?;
    let before = total_size(&items.iter().map(|x| x.0.clone()).collect::<Vec<_>>());
    let mut outs = vec![];
    let mut errs = vec![];
    for (src, dir) in &items {
        let r = (|| -> Result<PathBuf, String> {
            let text = read_text(src)?;
            let md = matches!(ext(src).as_str(), "md" | "markdown");
            let title = src.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let bytes = text_pdf(&title, &blocks(&text, md, 11.0), &regular, bold.as_deref())?;
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            let out = unique(dir.join(format!("{title}.pdf")));
            std::fs::write(&out, bytes).map_err(|e| e.to_string())?;
            Ok(out)
        })();
        match r {
            Ok(o) => outs.push(o),
            Err(e) => errs.push(format!("{}: {e}", src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())),
        }
    }
    result(outs, before, "PDF", errs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn md_blocks_and_wrap() {
        let b = blocks("# Biên bản họp\n\nNội dung **quan trọng** xem [link](https://x.vn)\n- Việc một\n  - Việc con\n---\n```\ncode\n```", true, 11.0);
        assert_eq!(b[0].text, "Biên bản họp");
        assert!(b[0].bold && b[0].size > 11.0);
        assert_eq!(b[1].text, "Nội dung quan trọng xem link (https://x.vn)");
        assert!(b[1].gap > 0.0);
        assert!(b[2].bullet && b[3].bullet && b[3].indent > b[2].indent);
        assert_eq!(b.last().unwrap().text, "code");
        let m = |s: &str| s.chars().count() as f32 * 5.0;
        assert_eq!(wrap("một hai ba bốn năm", 40.0, &m), vec!["một hai", "ba bốn", "năm"]);
        assert_eq!(wrap("abcdefghijkl", 25.0, &m), vec!["abcde", "fghij", "kl"]);
        assert_eq!(wrap("", 25.0, &m), vec![""]);
    }

    #[test]
    fn docx_and_html_text() {
        let xml = r#"<w:document><w:body><w:p><w:r><w:t>Xin chào</w:t></w:r><w:r><w:t xml:space="preserve"> Việt Nam &amp; bạn</w:t></w:r></w:p><w:p><w:r><w:t>Dòng 2</w:t><w:tab/><w:t>cột</w:t></w:r></w:p></w:body></w:document>"#;
        assert_eq!(docx_xml_text(xml), "Xin chào Việt Nam & bạn\nDòng 2\tcột");
        let h = html_text("<html><head><style>p{}</style><script>x=1</script></head><body><h1>Tiêu đề</h1><p>Đoạn &amp; chữ</p><ul><li>Một</li></ul></body></html>");
        assert!(h.contains("Tiêu đề") && h.contains("Đoạn & chữ") && h.contains("- Một") && !h.contains("x=1") && !h.contains("p{}"), "{h}");
        assert_eq!(decode(&[0xFF, 0xFE, b'A', 0, 0xE0, 0]), "Aà");
    }

    #[test]
    fn expand_folders() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("Ảnh");
        std::fs::create_dir_all(root.join("con")).unwrap();
        std::fs::create_dir_all(root.join(".an")).unwrap();
        for f in ["a.heic", "con/b.png", "ghi-chu.txt", ".an/c.jpg"] {
            std::fs::write(root.join(f), b"x").unwrap();
        }
        let single = d.path().join("le.jpg");
        std::fs::write(&single, b"x").unwrap();
        let mut v = expand(&[root.clone(), single.clone()], is_image, "JPG");
        v.sort();
        assert_eq!(v.len(), 3, "bỏ thư mục ẩn, bỏ file chữ");
        assert!(v.iter().any(|(s, o)| s.ends_with("con/b.png") && o.ends_with("Ảnh - JPG/con")));
        assert!(v.iter().any(|(s, o)| s == &single && o == d.path()));
        assert_eq!(count_kinds(&[root]), (2, 1));
        assert_eq!(image_target("JPEG"), Some(("jpg", "jpeg")));
        assert!(same_format(Path::new("x.JPEG"), "jpg") && !same_format(Path::new("x.png"), "jpg"));
    }

    #[test]
    fn pdf_is_valid_when_font_available() {
        let Some((reg, bold)) = system_fonts() else { return };
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("ghi chú.md");
        let long = "Cộng hoà xã hội chủ nghĩa Việt Nam — độc lập, tự do, hạnh phúc. ".repeat(80);
        std::fs::write(&src, format!("# Biên bản họp quý 4\n\n- Gửi báo giá trước thứ 6\n- Đặt lịch quay video\n\n{long}")).unwrap();
        let r = text_to_pdf(&[src]).unwrap();
        assert!(r.output.ends_with("ghi chú.pdf"));
        let b = std::fs::read(&r.output).unwrap();
        assert!(b.starts_with(b"%PDF-1.7") && b.ends_with(b"%%EOF\n"));
        let _ = (reg, bold);
        // kiểm tra thêm bằng công cụ ngoài nếu có (qpdf / pdftotext)
        if let Ok(o) = Command::new("qpdf").arg("--check").arg(&r.output).output() {
            assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stdout));
        }
        if let Ok(o) = Command::new("pdftotext").args(["-enc", "UTF-8"]).arg(&r.output).arg("-").output() {
            let t = String::from_utf8_lossy(&o.stdout);
            assert!(t.contains("Biên bản họp quý 4") && t.contains("Gửi báo giá trước thứ 6"), "{t}");
        }
    }

    /// Xem thử: MASI_PDF_SAMPLE=/đường/dẫn/file.md cargo test -p mini-asi render_sample -- --ignored
    #[test]
    #[ignore]
    fn render_sample() {
        if let Ok(p) = std::env::var("MASI_PDF_SAMPLE") {
            let r = text_to_pdf(&[PathBuf::from(p)]).unwrap();
            println!("{}", r.output.display());
        }
    }
}
