//! Ghi âm & ghi chú cuộc họp.
//!
//! - Thu **micro** (họp trực tiếp, giọng bạn) và — nếu bật — **âm thanh của máy** (giọng người khác trong
//!   Zoom / Meet / Teams…, kể cả khi đeo tai nghe; macOS 14.6+, cpal tạo "process tap").
//! - Cứ ~30–45 giây cắt một đoạn ở chỗ lặng nhất, chuyển sang 16 kHz mono, ghi file đoạn để nhận dạng
//!   (Apple Speech, ưu tiên ngay trên máy) và nối vào file ghi âm đầy đủ `audio.wav`.
//! - Có 2 nguồn thì biên bản tự chia "Bạn" (micro) / "Người khác" (âm thanh máy); câu micro chỉ là tiếng loa
//!   lọt vào thì bỏ (`is_echo`).
//! - Tóm tắt: Apple Intelligence (chia đoạn vì mô hình chỉ đọc được ~4096 token) hoặc Gemini — xem
//!   `commands/meeting.rs`. Phần thuần (cắt đoạn, đổi tần số, đọc kết quả tóm tắt…) có test.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::nlp::fold;

pub const RATE: u32 = 16_000;
/// Cắt đoạn trong khoảng 30–45 giây (máy chủ nhận dạng của Apple giới hạn ~1 phút mỗi lần)
pub const SEG_MIN: f32 = 30.0;
pub const SEG_MAX: f32 = 45.0;

// ───────────────────────── dữ liệu ─────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Line {
    /// giây tính từ lúc bắt đầu
    pub at: f32,
    /// "me" (micro) | "others" (âm thanh máy) | "" (chỉ có micro — không phân biệt)
    pub who: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ActionItem {
    pub task: String,
    #[serde(default)]
    pub owner: String,
    #[serde(default)]
    pub due: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Summary {
    pub overview: Vec<String>,
    pub decisions: Vec<String>,
    pub actions: Vec<ActionItem>,
    /// "Apple Intelligence" | "Gemini · …"
    pub engine: String,
    pub at: i64,
    /// ghi chú thêm (vd. "Gemini lỗi … — đã dùng Apple Intelligence")
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Meeting {
    pub id: String,
    pub title: String,
    /// unix ms
    pub started: i64,
    /// giây
    pub duration: u32,
    pub system_audio: bool,
    pub locale: String,
    /// "on-device" | "server"
    pub stt: String,
    pub lines: Vec<Line>,
    pub summary: Option<Summary>,
    /// đoạn đang chờ chuyển chữ
    #[serde(default)]
    pub pending: u32,
    /// đoạn nhận dạng lỗi (vd. mất mạng khi dùng máy chủ Apple)
    #[serde(default)]
    pub failed: u32,
    #[serde(default)]
    pub last_error: String,
    /// Ghi chú trong lúc họp (mỗi ~5 phút lời nói một mục) — máy đủ mạnh mới có
    #[serde(default)]
    pub live: Vec<LiveNote>,
    /// Số dòng toàn văn đã được ghi chú trong lúc họp
    #[serde(default)]
    pub noted: usize,
    /// Ghi chú "Biên bản họp" tương ứng trong tab Ghi chú
    #[serde(default)]
    pub note_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct LiveNote {
    /// giây (từ … đến …)
    pub from: f32,
    pub to: f32,
    pub summary: Summary,
}

#[derive(Debug, Clone, Serialize)]
pub struct MeetingBrief {
    pub id: String,
    pub title: String,
    pub started: i64,
    pub duration: u32,
    pub lines: usize,
    pub summarized: bool,
    pub actions: usize,
    pub has_audio: bool,
}

// ───────────────────────── lưu trữ ─────────────────────────

pub fn root(data_dir: &Path) -> PathBuf {
    data_dir.join("meetings")
}

pub fn dir_of(data_dir: &Path, id: &str) -> PathBuf {
    root(data_dir).join(id)
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 40 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn load(data_dir: &Path, id: &str) -> Result<Meeting, String> {
    if !valid_id(id) {
        return Err("Mã cuộc họp không hợp lệ".into());
    }
    let s = std::fs::read_to_string(dir_of(data_dir, id).join("meeting.json")).map_err(|_| "Không tìm thấy cuộc họp".to_string())?;
    serde_json::from_str(&s).map_err(|e| format!("File cuộc họp hỏng: {e}"))
}

pub fn save(data_dir: &Path, m: &Meeting) -> Result<(), String> {
    let d = dir_of(data_dir, &m.id);
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    let tmp = d.join("meeting.json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(m).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, d.join("meeting.json")).map_err(|e| e.to_string())
}

/// File ghi âm: ưu tiên bản nén m4a, chưa nén thì wav
pub fn audio_file(data_dir: &Path, id: &str) -> Option<PathBuf> {
    if !valid_id(id) {
        return None;
    }
    let d = dir_of(data_dir, id);
    ["audio.m4a", "audio.wav"].iter().map(|f| d.join(f)).find(|p| p.metadata().map(|m| m.len() > 1024).unwrap_or(false))
}

/// Nén audio.wav → audio.m4a (AAC 32 kbps, ~14 MB/giờ thay vì ~115 MB) bằng `afconvert` có sẵn trên macOS.
/// Thành công thì xoá wav. Máy khác / lỗi → giữ wav.
pub fn compress_audio(data_dir: &Path, id: &str) -> Result<PathBuf, String> {
    let d = dir_of(data_dir, id);
    let (wav, m4a) = (d.join("audio.wav"), d.join("audio.m4a"));
    if !wav.exists() {
        return audio_file(data_dir, id).ok_or_else(|| "Không có file ghi âm".into());
    }
    if !cfg!(target_os = "macos") {
        return Ok(wav);
    }
    let tmp = d.join("audio.tmp.m4a");
    let ok = std::process::Command::new("/usr/bin/afconvert")
        .args(["-f", "m4af", "-d", "aac", "-b", "32000"])
        .arg(&wav)
        .arg(&tmp)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if ok && tmp.metadata().map(|m| m.len() > 1024).unwrap_or(false) {
        std::fs::rename(&tmp, &m4a).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(&wav);
        Ok(m4a)
    } else {
        let _ = std::fs::remove_file(&tmp);
        Ok(wav)
    }
}

/// Các khúc toàn văn chưa được ghi chú trong lúc họp, đủ dài để tóm tắt (≥ `min_chars`)
pub fn next_live_chunk(lines: &[Line], noted: usize, min_chars: usize, max_chars: usize) -> Option<(usize, usize)> {
    let mut n = 0usize;
    for (i, l) in lines.iter().enumerate().skip(noted) {
        n += l.text.chars().count() + 12;
        if n >= min_chars || n >= max_chars {
            return Some((noted, i + 1));
        }
    }
    None
}

/// Sau khi app bị tắt giữa chừng (thoát, mất điện) lúc đang ghi / đang chuyển chữ: các cuộc họp còn "đang chờ" không
/// bao giờ xong → đặt lại `pending = 0` để giao diện không quay mãi; trả về id những cuộc họp còn file wav chưa nén.
pub fn recover(data_dir: &Path) -> Vec<String> {
    let mut need_compress = vec![];
    if let Ok(rd) = std::fs::read_dir(root(data_dir)) {
        for e in rd.flatten() {
            let id = e.file_name().to_string_lossy().to_string();
            let Ok(mut m) = load(data_dir, &id) else { continue };
            if m.pending > 0 {
                m.pending = 0;
                let _ = save(data_dir, &m);
            }
            let d = dir_of(data_dir, &id);
            // file đoạn tạm còn sót
            if let Ok(files) = std::fs::read_dir(&d) {
                for f in files.flatten() {
                    let n = f.file_name().to_string_lossy().to_string();
                    if n.starts_with("seg-") && n.ends_with(".wav") {
                        let _ = std::fs::remove_file(f.path());
                    }
                }
            }
            if d.join("audio.wav").exists() && !d.join("audio.m4a").exists() {
                need_compress.push(id);
            }
        }
    }
    need_compress
}

pub fn list(data_dir: &Path) -> Vec<MeetingBrief> {
    let mut v = vec![];
    if let Ok(rd) = std::fs::read_dir(root(data_dir)) {
        for e in rd.flatten() {
            let id = e.file_name().to_string_lossy().to_string();
            if let Ok(m) = load(data_dir, &id) {
                v.push(MeetingBrief {
                    has_audio: audio_file(data_dir, &id).is_some(),
                    id: m.id,
                    title: m.title,
                    started: m.started,
                    duration: m.duration,
                    lines: m.lines.len(),
                    summarized: m.summary.is_some(),
                    actions: m.summary.as_ref().map(|s| s.actions.len()).unwrap_or(0),
                });
            }
        }
    }
    v.sort_by(|a, b| b.started.cmp(&a.started));
    v
}

// ───────────────────────── âm thanh (thuần) ─────────────────────────

/// Đổi tần số lấy mẫu (nội suy tuyến tính) — đủ tốt cho nhận dạng giọng nói
pub fn resample(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || input.is_empty() {
        return input.to_vec();
    }
    let n = ((input.len() as u64 * to as u64) / from as u64) as usize;
    let step = from as f64 / to as f64;
    (0..n)
        .map(|i| {
            let x = i as f64 * step;
            let j = x as usize;
            let f = (x - j as f64) as f32;
            let a = input[j.min(input.len() - 1)];
            let b = input[(j + 1).min(input.len() - 1)];
            a + (b - a) * f
        })
        .collect()
}

pub fn rms(x: &[f32]) -> f32 {
    if x.is_empty() {
        return 0.0;
    }
    (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
}

/// Chỗ cắt đoạn: điểm lặng nhất (cửa sổ 300 ms) trong khoảng [min_s, max_s] giây
pub fn best_cut(samples: &[f32], rate: u32, min_s: f32, max_s: f32) -> usize {
    let r = rate as f32;
    let lo = (min_s * r) as usize;
    let hi = ((max_s * r) as usize).min(samples.len());
    if hi <= lo {
        return hi.max(samples.len().min(lo));
    }
    let win = (0.3 * r) as usize;
    let hop = (0.1 * r) as usize;
    let mut best = (f32::MAX, hi);
    let mut i = lo;
    while i + win <= hi {
        let e = rms(&samples[i..i + win]);
        if e < best.0 - 1e-6 {
            best = (e, i + win / 2);
        }
        i += hop.max(1);
    }
    best.1
}

/// Có tiếng nói không (bỏ qua đoạn im lặng / tiếng ồn nền để không gửi đi nhận dạng)
pub fn has_speech(samples: &[f32], rate: u32) -> bool {
    let win = (rate as usize / 10).max(1);
    let loud = samples.chunks(win).filter(|c| rms(c) > 0.012).count();
    loud >= 6 // ≥ 0.6 giây có tiếng
}

pub fn write_wav16(path: &Path, samples: &[f32]) -> Result<(), String> {
    let spec = hound::WavSpec { channels: 1, sample_rate: RATE, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut w = hound::WavWriter::create(path, spec).map_err(|e| e.to_string())?;
    for s in samples {
        w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16).map_err(|e| e.to_string())?;
    }
    w.finalize().map_err(|e| e.to_string())
}

// ───────────────────────── biên bản (thuần) ─────────────────────────

fn norm_words(s: &str) -> Vec<String> {
    fold(s).chars().map(|c| if c.is_alphanumeric() { c } else { ' ' }).collect::<String>().split_whitespace().map(str::to_string).collect()
}

/// Câu micro thực ra là tiếng loa (người khác nói) lọt vào micro: phần lớn từ trùng với câu của âm thanh máy
pub fn is_echo(mic: &str, sys: &str) -> bool {
    let (m, s) = (norm_words(mic), norm_words(sys));
    if m.is_empty() || s.is_empty() {
        return false;
    }
    let shared = m.iter().filter(|w| s.contains(w)).count();
    shared * 10 >= m.len() * 6
}

pub fn clock(sec: f32) -> String {
    let s = sec.max(0.0) as u32;
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60) } else { format!("{:02}:{:02}", s / 60, s % 60) }
}

fn who_label(w: &str, en: bool) -> &'static str {
    match (w, en) {
        ("me", false) => "Bạn",
        ("others", false) => "Người khác",
        ("me", true) => "You",
        ("others", true) => "Others",
        _ => "",
    }
}

/// Cuộc họp nói tiếng Anh (theo ngôn ngữ nhận dạng lúc ghi) → biên bản, nhãn và lời nhắc cho AI bằng tiếng Anh
pub fn is_en(locale: &str) -> bool {
    locale.to_lowercase().starts_with("en")
}

/// Toàn văn có mốc thời gian (nhãn người nói tiếng Việt)
pub fn transcript_text(lines: &[Line]) -> String {
    transcript_text_in(lines, false)
}

pub fn transcript_text_in(lines: &[Line], en: bool) -> String {
    lines
        .iter()
        .map(|l| {
            let w = who_label(&l.who, en);
            if w.is_empty() { format!("[{}] {}", clock(l.at), l.text) } else { format!("[{}] {w}: {}", clock(l.at), l.text) }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Chia toàn văn thành các khúc ≤ `max_chars` ký tự (không cắt giữa câu)
pub fn chunks(lines: &[Line], max_chars: usize) -> Vec<String> {
    chunks_in(lines, max_chars, false)
}

pub fn chunks_in(lines: &[Line], max_chars: usize, en: bool) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    for l in transcript_text_in(lines, en).lines() {
        if !cur.is_empty() && cur.chars().count() + l.chars().count() + 1 > max_chars {
            out.push(std::mem::take(&mut cur));
        }
        let l: String = l.chars().take(max_chars).collect();
        if !cur.is_empty() {
            cur.push('\n');
        }
        cur.push_str(&l);
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

fn clean_item(s: &str) -> Option<String> {
    let t = s.trim().trim_start_matches(|c: char| c == '-' || c == '•' || c == '*' || c == '–' || c.is_ascii_digit() || c == '.' || c == ')').trim();
    let f = fold(t);
    let none = ["khong co", "khong", "chua co", "khong ro", "none", "n a", "khong co gi", "no decisions", "no action items", "nothing", "none mentioned", "not mentioned"];
    if t.chars().filter(|c| c.is_alphanumeric()).count() < 3 || none.contains(&f.trim_end_matches('.')) {
        return None;
    }
    Some(t.trim_end_matches(';').to_string())
}

fn none_field(s: &str) -> String {
    let t = s.trim().trim_matches(['[', ']']).trim();
    let f = fold(t);
    if t.is_empty() || t == "?" || ["khong ro", "chua ro", "chua co", "khong", "n a", "none", "unknown"].contains(&f.as_str()) { String::new() } else { t.to_string() }
}

/// Đọc biên bản dạng chữ (Apple Intelligence trả về) theo các mục TÓM TẮT / QUYẾT ĐỊNH / VIỆC CẦN LÀM.
/// Việc cần làm dạng "việc | người phụ trách | hạn".
pub fn parse_summary_text(raw: &str) -> Summary {
    let mut s = Summary::default();
    let mut sec = 0u8; // 1 tóm tắt, 2 quyết định, 3 việc
    for line in raw.lines() {
        let l = line.trim().trim_matches('*').trim();
        if l.is_empty() {
            continue;
        }
        let f = fold(l).trim_start_matches('#').trim().to_string();
        let head = |k: &str| f.starts_with(k) && f.len() <= k.len() + 3;
        let (h, rest) = if let Some((a, b)) = l.split_once(':') { (fold(a).trim_start_matches('#').trim().to_string(), b.trim()) } else { (f.clone(), "") };
        let new_sec = if head("tom tat") || h == "tom tat" || h == "noi dung chinh" || h == "y chinh" || head("summary") || h == "summary" || h == "key points" || h == "overview" {
            1
        } else if head("quyet dinh") || h == "quyet dinh" || h == "ket luan" || head("decisions") || h == "decisions" || h == "decision" {
            2
        } else if head("viec can lam") || h == "viec can lam" || h == "cong viec" || h == "viec tiep theo" || head("action items") || matches!(h.as_str(), "action items" | "actions" | "to do" | "todo" | "next steps" | "tasks") {
            3
        } else {
            0
        };
        if new_sec > 0 {
            sec = new_sec;
            if rest.is_empty() {
                continue;
            }
        }
        let body = if new_sec > 0 { rest } else { l };
        match sec {
            1 => s.overview.extend(clean_item(body)),
            2 => s.decisions.extend(clean_item(body)),
            3 => {
                if let Some(t) = clean_item(body) {
                    let parts: Vec<&str> = t.split('|').collect();
                    s.actions.push(ActionItem {
                        task: parts[0].trim().to_string(),
                        owner: parts.get(1).map(|x| none_field(x)).unwrap_or_default(),
                        due: parts.get(2).map(|x| none_field(x)).unwrap_or_default(),
                    });
                }
            }
            _ => s.overview.extend(clean_item(body)),
        }
    }
    s
}

/// Đọc biên bản dạng JSON (Gemini): {"overview":[..],"decisions":[..],"actions":[{"task","owner","due"}]}
pub fn parse_summary_json(raw: &str) -> Option<Summary> {
    let a = raw.find('{')?;
    let b = raw.rfind('}')?;
    let v: serde_json::Value = serde_json::from_str(raw.get(a..=b)?).ok()?;
    let strs = |k: &str| -> Vec<String> {
        match &v[k] {
            serde_json::Value::Array(x) => x.iter().filter_map(|i| i.as_str()).filter_map(clean_item).collect(),
            serde_json::Value::String(x) => x.lines().filter_map(clean_item).collect(),
            _ => vec![],
        }
    };
    let actions = v["actions"]
        .as_array()
        .map(|x| {
            x.iter()
                .filter_map(|i| {
                    let task = i["task"].as_str().or_else(|| i.as_str())?.trim().to_string();
                    (!task.is_empty()).then(|| ActionItem { task, owner: none_field(i["owner"].as_str().unwrap_or("")), due: none_field(i["due"].as_str().unwrap_or("")) })
                })
                .collect()
        })
        .unwrap_or_default();
    let s = Summary { overview: strs("overview"), decisions: strs("decisions"), actions, ..Default::default() };
    (!s.overview.is_empty() || !s.actions.is_empty() || !s.decisions.is_empty()).then_some(s)
}

/// Gộp kết quả tóm tắt của nhiều khúc: bỏ trùng (so sánh không dấu)
pub fn merge(parts: Vec<Summary>) -> Summary {
    let mut out = Summary::default();
    let key = |s: &str| norm_words(s).join(" ");
    for p in parts {
        for o in p.overview {
            if !out.overview.iter().any(|x| key(x) == key(&o)) {
                out.overview.push(o);
            }
        }
        for d in p.decisions {
            if !out.decisions.iter().any(|x| key(x) == key(&d)) {
                out.decisions.push(d);
            }
        }
        for a in p.actions {
            if !out.actions.iter().any(|x| key(&x.task) == key(&a.task)) {
                out.actions.push(a);
            }
        }
    }
    out
}

/// Biên bản dạng chữ thường cho tab Ghi chú (dòng đầu là tiêu đề; đồng bộ sang app Ghi chú của Apple cũng gọn)
pub fn to_note_text(m: &Meeting, with_transcript: bool) -> String {
    let en = is_en(&m.locale);
    let l = |vi: &'static str, e: &'static str| if en { e } else { vi };
    let when = chrono::DateTime::from_timestamp_millis(m.started).map(|d| d.with_timezone(&chrono::Local).format("%d/%m/%Y %H:%M").to_string()).unwrap_or_default();
    let mut o = format!("{}\n{} · {when} · {}\n", m.title, l("Biên bản họp", "Meeting minutes"), clock(m.duration as f32));
    if let Some(s) = &m.summary {
        let list = |o: &mut String, t: &str, xs: &[String]| {
            if !xs.is_empty() {
                o.push_str(&format!("\n{t}\n"));
                for x in xs {
                    o.push_str(&format!("• {x}\n"));
                }
            }
        };
        list(&mut o, l("Tóm tắt", "Summary"), &s.overview);
        list(&mut o, l("Quyết định", "Decisions"), &s.decisions);
        if !s.actions.is_empty() {
            o.push_str(l("\nViệc cần làm\n", "\nAction items\n"));
            for a in &s.actions {
                let mut t = format!("☐ {}", a.task);
                if !a.owner.is_empty() {
                    t.push_str(&format!(" — {}", a.owner));
                }
                if !a.due.is_empty() {
                    t.push_str(&format!(" ({}: {})", l("hạn", "due"), a.due));
                }
                o.push_str(&t);
                o.push('\n');
            }
        }
    }
    if with_transcript && !m.lines.is_empty() {
        o.push_str(l("\nToàn văn\n", "\nTranscript\n"));
        o.push_str(&transcript_text_in(&m.lines, en));
        o.push('\n');
    }
    o.push_str(l("\nNghe lại ghi âm: Mini-Asi › Họp", "\nReplay the recording: Mini-Asi › Meetings"));
    o
}

/// Biên bản dạng Markdown (lưu vào Ghi chú / chép)
pub fn to_markdown(m: &Meeting, with_transcript: bool) -> String {
    let en = is_en(&m.locale);
    let l = |vi: &'static str, e: &'static str| if en { e } else { vi };
    let mut o = format!("# {}\n{} · {}\n", m.title, chrono::DateTime::from_timestamp_millis(m.started).map(|d| d.with_timezone(&chrono::Local).format("%d/%m/%Y %H:%M").to_string()).unwrap_or_default(), clock(m.duration as f32));
    if let Some(s) = &m.summary {
        if !s.overview.is_empty() {
            o.push_str(l("\n## Tóm tắt\n", "\n## Summary\n"));
            for x in &s.overview {
                o.push_str(&format!("- {x}\n"));
            }
        }
        if !s.decisions.is_empty() {
            o.push_str(l("\n## Quyết định\n", "\n## Decisions\n"));
            for x in &s.decisions {
                o.push_str(&format!("- {x}\n"));
            }
        }
        if !s.actions.is_empty() {
            o.push_str(l("\n## Việc cần làm\n", "\n## Action items\n"));
            for a in &s.actions {
                let mut t = format!("- [ ] {}", a.task);
                if !a.owner.is_empty() {
                    t.push_str(&format!(" — {}", a.owner));
                }
                if !a.due.is_empty() {
                    t.push_str(&format!(" ({}: {})", l("hạn", "due"), a.due));
                }
                o.push_str(&t);
                o.push('\n');
            }
        }
    }
    if with_transcript && !m.lines.is_empty() {
        o.push_str(l("\n## Toàn văn\n", "\n## Transcript\n"));
        o.push_str(&transcript_text_in(&m.lines, en));
        o.push('\n');
    }
    o
}

// ───────────────────────── thu âm ─────────────────────────

/// Một đoạn đã cắt, chờ chuyển chữ
pub struct Segment {
    pub index: u32,
    /// giây bắt đầu
    pub at: f32,
    pub mic: Vec<f32>,
    /// None = không thu âm thanh máy
    pub sys: Option<Vec<f32>>,
}

pub struct Recorder {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<Result<(), String>>>,
    pub system_audio: bool,
}

/// Trạng thái gửi cho giao diện ~3 lần/giây
#[derive(Debug, Clone, Copy, Default)]
pub struct Level {
    pub mic: f32,
    pub sys: f32,
    pub secs: f32,
}

type Buf = Arc<Mutex<Vec<f32>>>;

fn open_stream(dev: &cpal::Device, cfg: cpal::SupportedStreamConfig, buf: Buf, what: &'static str) -> Result<cpal::Stream, String> {
    let channels = cfg.channels() as usize;
    let sc: cpal::StreamConfig = cfg.clone().into();
    let err_fn = move |e| eprintln!("Lỗi {what}: {e}");
    macro_rules! build {
        ($t:ty, $conv:expr) => {{
            let b = buf.clone();
            dev.build_input_stream(
                sc.clone(),
                move |data: &[$t], _: &cpal::InputCallbackInfo| {
                    let mut v = b.lock().unwrap();
                    for frame in data.chunks(channels.max(1)) {
                        v.push(frame.iter().map(|x| $conv(*x)).sum::<f32>() / frame.len() as f32);
                    }
                },
                err_fn,
                None,
            )
        }};
    }
    let s = match cfg.sample_format() {
        cpal::SampleFormat::F32 => build!(f32, |x: f32| x),
        cpal::SampleFormat::I16 => build!(i16, |x: i16| x as f32 / 32768.0),
        cpal::SampleFormat::U16 => build!(u16, |x: u16| (x as f32 - 32768.0) / 32768.0),
        cpal::SampleFormat::I32 => build!(i32, |x: i32| x as f32 / 2147483648.0),
        f => return Err(format!("Định dạng âm thanh chưa hỗ trợ: {f:?}")),
    }
    .map_err(|e| format!("{e}"))?;
    s.play().map_err(|e| format!("{e}"))?;
    Ok(s)
}

/// Bắt đầu thu. `system`: thu thêm âm thanh của máy. Trả về (Recorder, cảnh báo nếu không thu được âm thanh máy).
/// `on_segment` nhận từng đoạn ~30–45 giây; `audio_path`: file ghi âm đầy đủ (16 kHz mono).
pub fn start(
    system: bool,
    audio_path: PathBuf,
    on_level: impl Fn(Level) + Send + 'static,
    on_segment: impl Fn(Segment) + Send + 'static,
) -> Result<(Recorder, Option<String>), String> {
    let stop = Arc::new(AtomicBool::new(false));
    let stop2 = stop.clone();
    let (ready_tx, ready_rx) = mpsc::channel::<Result<(bool, Option<String>), String>>();
    let handle = std::thread::spawn(move || -> Result<(), String> {
        let host = cpal::default_host();
        let Some(mic_dev) = host.default_input_device() else {
            let _ = ready_tx.send(Err("Không tìm thấy micro".into()));
            return Err("Không tìm thấy micro".into());
        };
        let mic_cfg = match mic_dev.default_input_config() {
            Ok(c) => c,
            Err(e) => {
                let _ = ready_tx.send(Err(format!("Không mở được micro: {e}")));
                return Err(e.to_string());
            }
        };
        let mic_rate = mic_cfg.sample_rate();
        let mic_buf: Buf = Arc::new(Mutex::new(vec![]));
        let _mic_stream = match open_stream(&mic_dev, mic_cfg, mic_buf.clone(), "micro") {
            Ok(s) => s,
            Err(e) => {
                let msg = format!("Không thu âm được (đã cấp quyền Micro chưa?): {e}");
                let _ = ready_tx.send(Err(msg.clone()));
                return Err(msg);
            }
        };
        // Âm thanh của máy: mở luồng "thu" trên thiết bị phát mặc định → cpal dùng process tap (macOS 14.6+)
        let sys_buf: Buf = Arc::new(Mutex::new(vec![]));
        let mut sys_rate = RATE;
        let mut warn = None;
        let sys_stream = if system {
            let r = host.default_output_device().ok_or_else(|| "không thấy loa / tai nghe".to_string()).and_then(|d| {
                // Thiết bị vừa phát vừa có micro (vd. tai nghe Bluetooth đang gọi) → cpal sẽ mở micro của nó thay vì
                // thu âm thanh máy → bỏ qua để không ghi nhầm
                if d.supports_input() {
                    return Err(format!("thiết bị phát \"{}\" có kèm micro nên chưa thu được âm thanh máy qua nó", d.description().map(|x| x.name().to_string()).unwrap_or_default()));
                }
                let cfg = d.default_output_config().map_err(|e| e.to_string())?;
                sys_rate = cfg.sample_rate();
                open_stream(&d, cfg, sys_buf.clone(), "âm thanh máy")
            });
            match r {
                Ok(s) => Some(s),
                Err(e) => {
                    warn = Some(format!(
                        "Chưa thu được âm thanh của máy ({e}) — chỉ ghi micro. Cần macOS 14.6+ và quyền \"Ghi âm thanh hệ thống\" trong Cài đặt hệ thống › Quyền riêng tư."
                    ));
                    None
                }
            }
        } else {
            None
        };
        let with_sys = sys_stream.is_some();
        let _ = ready_tx.send(Ok((with_sys, warn)));

        let spec = hound::WavSpec { channels: 1, sample_rate: RATE, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
        let mut writer = hound::WavWriter::create(&audio_path, spec).ok();
        let t0 = Instant::now();
        let (mut pm, mut ps): (Vec<f32>, Vec<f32>) = (vec![], vec![]); // phần chưa cắt, 16 kHz
        let mut seg_at = 0f32;
        let mut index = 0u32;
        let emit = |pm: &mut Vec<f32>, ps: &mut Vec<f32>, cut: usize, seg_at: &mut f32, index: &mut u32, writer: &mut Option<hound::WavWriter<std::io::BufWriter<std::fs::File>>>| {
            let cut = cut.min(pm.len());
            let mic: Vec<f32> = pm.drain(..cut).collect();
            let sys: Option<Vec<f32>> = with_sys.then(|| {
                let c = cut.min(ps.len());
                let mut v: Vec<f32> = ps.drain(..c).collect();
                v.resize(cut, 0.0);
                v
            });
            if let Some(w) = writer.as_mut() {
                for (i, m) in mic.iter().enumerate() {
                    let s = m + sys.as_ref().map(|v| v[i]).unwrap_or(0.0);
                    let _ = w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16);
                }
                let _ = w.flush();
            }
            let at = *seg_at;
            *seg_at += cut as f32 / RATE as f32;
            *index += 1;
            on_segment(Segment { index: *index, at, mic, sys });
        };
        let mut last_level = Instant::now();
        loop {
            let stopping = stop2.load(Ordering::Relaxed);
            std::thread::sleep(Duration::from_millis(if stopping { 0 } else { 200 }));
            let raw_m = std::mem::take(&mut *mic_buf.lock().unwrap());
            let raw_s = std::mem::take(&mut *sys_buf.lock().unwrap());
            let (m16, s16) = (resample(&raw_m, mic_rate, RATE), resample(&raw_s, sys_rate, RATE));
            if last_level.elapsed() >= Duration::from_millis(300) || stopping {
                on_level(Level { mic: (rms(&m16) * 6.0).min(1.0), sys: (rms(&s16) * 6.0).min(1.0), secs: t0.elapsed().as_secs_f32() });
                last_level = Instant::now();
            }
            pm.extend(m16);
            if with_sys {
                ps.extend(s16);
            }
            let max = (SEG_MAX * RATE as f32) as usize;
            if pm.len() >= max {
                let mix: Vec<f32> = pm.iter().enumerate().map(|(i, m)| m + ps.get(i).copied().unwrap_or(0.0)).collect();
                let cut = best_cut(&mix, RATE, SEG_MIN, SEG_MAX);
                emit(&mut pm, &mut ps, cut, &mut seg_at, &mut index, &mut writer);
            }
            if stopping {
                if pm.len() >= RATE as usize {
                    let n = pm.len();
                    emit(&mut pm, &mut ps, n, &mut seg_at, &mut index, &mut writer);
                }
                break;
            }
        }
        drop(sys_stream);
        if let Some(w) = writer {
            let _ = w.finalize();
        }
        Ok(())
    });
    match ready_rx.recv_timeout(Duration::from_secs(20)) {
        Ok(Ok((sys, warn))) => Ok((Recorder { stop, handle: Some(handle), system_audio: sys }, warn)),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("Micro không phản hồi".into()),
    }
}

impl Recorder {
    /// Dừng thu, đợi gửi nốt đoạn cuối
    pub fn finish(mut self) -> Result<(), String> {
        self.stop.store(true, Ordering::Relaxed);
        self.handle.take().ok_or("Đã dừng")?.join().map_err(|_| "Luồng thu âm bị lỗi".to_string())?
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(secs: f32, amp: f32) -> Vec<f32> {
        (0..(secs * RATE as f32) as usize).map(|i| amp * ((i as f32) * 0.07).sin()).collect()
    }

    #[test]
    fn resample_and_cut() {
        let x: Vec<f32> = (0..48_000).map(|i| i as f32 / 48_000.0).collect();
        let y = resample(&x, 48_000, RATE);
        assert_eq!(y.len(), 16_000);
        assert!((y[8000] - 0.5).abs() < 0.001);
        assert_eq!(resample(&x, 16_000, 16_000).len(), 48_000);
        // 50 giây có tiếng, lặng 1 giây ở giây 37 → cắt ở đó
        let mut s = tone(37.0, 0.3);
        s.extend(vec![0.0; RATE as usize]);
        s.extend(tone(12.0, 0.3));
        let c = best_cut(&s, RATE, SEG_MIN, SEG_MAX) as f32 / RATE as f32;
        assert!((37.0..38.0).contains(&c), "cắt ở {c}");
        assert!(has_speech(&tone(2.0, 0.2), RATE));
        assert!(!has_speech(&vec![0.001; RATE as usize * 5], RATE));
    }

    #[test]
    fn echo_and_transcript() {
        assert!(is_echo("anh gửi báo giá trước thứ sáu nhé", "Anh gửi báo giá trước thứ 6 nhé, em chờ"));
        assert!(!is_echo("ok em đồng ý", "Anh gửi báo giá trước thứ sáu nhé"));
        let lines = vec![
            Line { at: 3.0, who: "me".into(), text: "Chào mọi người".into() },
            Line { at: 65.0, who: "others".into(), text: "Chào anh".into() },
            Line { at: 3725.0, who: "".into(), text: "Kết thúc".into() },
        ];
        assert_eq!(transcript_text(&lines), "[00:03] Bạn: Chào mọi người\n[01:05] Người khác: Chào anh\n[1:02:05] Kết thúc");
        let long: Vec<Line> = (0..50).map(|i| Line { at: i as f32 * 10.0, who: "".into(), text: "x".repeat(90) }).collect();
        let c = chunks(&long, 1000);
        assert!(c.len() >= 5 && c.iter().all(|x| x.chars().count() <= 1000));
        assert_eq!(c.iter().map(|x| x.lines().count()).sum::<usize>(), 50);
    }

    #[test]
    fn parse_apple_text_summary() {
        let raw = "**TÓM TẮT:**\n- Chốt kế hoạch truyền thông quý 4\n- Ngân sách giữ nguyên\nQUYẾT ĐỊNH:\n- Chạy thử TikTok Ads 2 tuần\n- Không có\nVIỆC CẦN LÀM:\n- Gửi báo giá cho Hoiana | anh Tuấn | thứ 6\n- Làm brief video | ? | ?\n1. Chốt lịch quay | Lan | 20/11";
        let s = parse_summary_text(raw);
        assert_eq!(s.overview, vec!["Chốt kế hoạch truyền thông quý 4", "Ngân sách giữ nguyên"]);
        assert_eq!(s.decisions, vec!["Chạy thử TikTok Ads 2 tuần"]);
        assert_eq!(s.actions.len(), 3);
        assert_eq!(s.actions[0], ActionItem { task: "Gửi báo giá cho Hoiana".into(), owner: "anh Tuấn".into(), due: "thứ 6".into() });
        assert_eq!(s.actions[1], ActionItem { task: "Làm brief video".into(), owner: "".into(), due: "".into() });
        assert_eq!(s.actions[2].owner, "Lan");
        // tiêu đề viết thường, có nội dung cùng dòng
        let s = parse_summary_text("Tóm tắt: Họp nhanh về tuyển dụng\nViệc cần làm: Đăng tin tuyển dụng | chị Hà | mai");
        assert_eq!(s.overview, vec!["Họp nhanh về tuyển dụng"]);
        assert_eq!(s.actions[0].due, "mai");
    }

    #[test]
    fn parse_gemini_json_and_merge() {
        let raw = "```json\n{\"overview\":[\"Bàn kế hoạch Q4\"],\"decisions\":[\"Tăng ngân sách Meta\"],\"actions\":[{\"task\":\"Gửi báo cáo\",\"owner\":\"Bình\",\"due\":\"thứ 6\"},{\"task\":\"Đặt phòng họp\",\"owner\":\"không rõ\",\"due\":\"\"}]}\n```";
        let s = parse_summary_json(raw).unwrap();
        assert_eq!(s.overview, vec!["Bàn kế hoạch Q4"]);
        assert_eq!(s.actions[1].owner, "");
        assert!(parse_summary_json("{}").is_none());
        assert!(parse_summary_json("xin lỗi").is_none());
        let m = merge(vec![
            Summary { overview: vec!["A".repeat(5)], actions: vec![ActionItem { task: "Gửi báo giá".into(), ..Default::default() }], ..Default::default() },
            Summary { overview: vec!["aaaaa".into(), "B ý mới".into()], actions: vec![ActionItem { task: "gửi báo giá".into(), ..Default::default() }], ..Default::default() },
        ]);
        assert_eq!(m.overview.len(), 2);
        assert_eq!(m.actions.len(), 1);
    }

    #[test]
    fn english_minutes_and_recover() {
        let s = parse_summary_text("SUMMARY:\n- Agreed on the Q4 budget\nDECISIONS:\n- No decisions\nACTION ITEMS:\n- Send the quote | Tuan | Friday\n- Book the venue | ? | ?");
        assert_eq!(s.overview, vec!["Agreed on the Q4 budget"]);
        assert!(s.decisions.is_empty());
        assert_eq!(s.actions.len(), 2);
        assert_eq!((s.actions[0].owner.as_str(), s.actions[0].due.as_str(), s.actions[1].owner.as_str()), ("Tuan", "Friday", ""));
        let lines = vec![Line { at: 3.0, who: "me".into(), text: "Hello".into() }, Line { at: 9.0, who: "others".into(), text: "Hi".into() }];
        assert_eq!(transcript_text_in(&lines, true), "[00:03] You: Hello\n[00:09] Others: Hi");
        let m = Meeting { id: "x".into(), title: "Sync".into(), locale: "en-US".into(), lines, summary: Some(s), ..Default::default() };
        let nt = to_note_text(&m, true);
        assert!(nt.contains("Meeting minutes") && nt.contains("Action items") && nt.contains("(due: Friday)") && nt.contains("Transcript") && !nt.contains("Tóm tắt"));
        // khôi phục cuộc họp dở dang
        let dir = std::env::temp_dir().join(format!("asi-meet-recover-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut m2 = Meeting { id: "20261111-090000".into(), pending: 3, ..Default::default() };
        m2.title = "Dở dang".into();
        save(&dir, &m2).unwrap();
        std::fs::write(dir_of(&dir, &m2.id).join("audio.wav"), vec![0u8; 4096]).unwrap();
        std::fs::write(dir_of(&dir, &m2.id).join("seg-3-mic.wav"), b"x").unwrap();
        assert_eq!(recover(&dir), vec![m2.id.clone()]);
        assert_eq!(load(&dir, &m2.id).unwrap().pending, 0);
        assert!(!dir_of(&dir, &m2.id).join("seg-3-mic.wav").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn live_chunks() {
        let lines: Vec<Line> = (0..10).map(|i| Line { at: i as f32 * 30.0, who: "".into(), text: "x".repeat(288) }).collect();
        assert_eq!(next_live_chunk(&lines, 0, 2200, 2600), Some((0, 8)));
        assert_eq!(next_live_chunk(&lines, 8, 2200, 2600), None, "chưa đủ dài");
        assert_eq!(next_live_chunk(&[], 0, 2200, 2600), None);
    }

    #[test]
    fn store_roundtrip_and_markdown() {
        let d = tempfile::tempdir().unwrap();
        let m = Meeting {
            id: "20261109-1400".into(),
            title: "Họp agency".into(),
            started: 1_700_000_000_000,
            duration: 1830,
            lines: vec![Line { at: 1.0, who: "me".into(), text: "Bắt đầu".into() }],
            summary: Some(Summary { overview: vec!["Ý chính".into()], actions: vec![ActionItem { task: "Gửi file".into(), owner: "Lan".into(), due: "mai".into() }], ..Default::default() }),
            ..Default::default()
        };
        save(d.path(), &m).unwrap();
        let l = list(d.path());
        assert_eq!((l.len(), l[0].actions, l[0].summarized), (1, 1, true));
        assert_eq!(load(d.path(), "20261109-1400").unwrap().title, "Họp agency");
        assert!(load(d.path(), "../etc").is_err());
        let nt = to_note_text(&m, false);
        assert!(nt.starts_with("Họp agency\nBiên bản họp · "));
        assert!(nt.contains("• Ý chính") && nt.contains("☐ Gửi file — Lan (hạn: mai)") && !nt.contains("[00:01]"));
        let md = to_markdown(&m, true);
        assert!(md.starts_with("# Họp agency"));
        assert!(md.contains("- [ ] Gửi file — Lan (hạn: mai)"));
        assert!(md.contains("[00:01] Bạn: Bắt đầu"));
    }
}
