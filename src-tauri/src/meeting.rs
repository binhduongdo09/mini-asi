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
    /// giây trong ghi âm nơi việc này được nhắc tới (để nghe lại)
    #[serde(default)]
    pub at: Option<f32>,
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
    /// mốc giây (để nghe lại) cho từng ý tóm tắt / quyết định — cùng độ dài với `overview` / `decisions`
    #[serde(default)]
    pub overview_at: Vec<Option<f32>>,
    #[serde(default)]
    pub decisions_at: Vec<Option<f32>>,
}

/// Bản dịch biên bản (Việt ↔ Anh)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Translation {
    /// "en" | "vi" — ngôn ngữ của bản dịch
    pub lang: String,
    pub summary: Summary,
    pub engine: String,
    pub at: i64,
}

/// Một đoạn của "ghi chép đầy đủ" (Take note): toàn bộ lời nói đã được AI làm sạch, không lược ý
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct NotePart {
    /// giây bắt đầu đoạn (để nghe lại)
    pub at: f32,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct FullNote {
    pub parts: Vec<NotePart>,
    pub engine: String,
    pub at: i64,
    /// số đoạn chưa làm sạch được (giữ nguyên văn)
    #[serde(default)]
    pub raw_parts: u32,
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
    /// Ghi chép đầy đủ (nút Take note) + ghi chú tương ứng trong tab Ghi chú
    #[serde(default)]
    pub full: Option<FullNote>,
    #[serde(default)]
    pub full_note_id: Option<u64>,
    /// Bản dịch biên bản
    #[serde(default)]
    pub translation: Option<Translation>,
    /// Đã nhận dạng lại toàn văn bằng gì ("Apple · vi-VN", "Gemini · …") — trống nếu là bản ghi lúc họp
    #[serde(default)]
    pub retranscribed: String,
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

/// Đọc file ghi âm thành mẫu 16 kHz mono (m4a → đổi tạm sang wav bằng `afconvert` của macOS)
pub fn load_audio_16k(data_dir: &Path, id: &str) -> Result<Vec<f32>, String> {
    let src = audio_file(data_dir, id).ok_or("Không còn file ghi âm")?;
    let (path, tmp) = if src.extension().is_some_and(|e| e == "m4a") {
        let tmp = dir_of(data_dir, id).join("audio.decode.wav");
        let ok = std::process::Command::new("/usr/bin/afconvert")
            .args(["-f", "WAVE", "-d", "LEI16@16000", "-c", "1"])
            .arg(&src)
            .arg(&tmp)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ok {
            let _ = std::fs::remove_file(&tmp);
            return Err("macOS không giải nén được file ghi âm".into());
        }
        (tmp.clone(), Some(tmp))
    } else {
        (src, None)
    };
    let r = read_wav_16k(&path);
    if let Some(t) = tmp {
        let _ = std::fs::remove_file(t);
    }
    r
}

/// WAV bất kỳ (mono/stereo, 16 bit / float) → mẫu 16 kHz mono
pub fn read_wav_16k(path: &Path) -> Result<Vec<f32>, String> {
    let mut r = hound::WavReader::open(path).map_err(|e| format!("Không đọc được file ghi âm: {e}"))?;
    let spec = r.spec();
    let ch = spec.channels.max(1) as usize;
    let raw: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().filter_map(Result::ok).collect(),
        hound::SampleFormat::Int => {
            let scale = (1i64 << (spec.bits_per_sample.max(1) - 1)) as f32;
            r.samples::<i32>().filter_map(Result::ok).map(|x| x as f32 / scale).collect()
        }
    };
    let mono: Vec<f32> = raw.chunks(ch).map(|c| c.iter().sum::<f32>() / c.len() as f32).collect();
    Ok(resample(&mono, spec.sample_rate, RATE))
}

/// Cắt bản ghi dài thành các đoạn ~`min_s`–`max_s` giây ở chỗ lặng nhất → (giây bắt đầu, mẫu)
pub fn split_audio(samples: &[f32], min_s: f32, max_s: f32) -> Vec<(f32, Vec<f32>)> {
    let mut out = vec![];
    let mut pos = 0usize;
    let max = (max_s * RATE as f32) as usize;
    while pos < samples.len() {
        let rest = &samples[pos..];
        let cut = if rest.len() <= max { rest.len() } else { best_cut(rest, RATE, min_s, max_s).max(1) };
        out.push((pos as f32 / RATE as f32, rest[..cut].to_vec()));
        pos += cut;
    }
    out
}

/// Khúc toàn văn kế tiếp để ghi chú trong lúc họp: phần chưa ghi chú đã đủ ~3 phút lời nói (hoặc đủ dài theo ký tự) →
/// trả về khúc đầu tiên theo `chunk_ranges` (cắt ở chỗ đổi người nói nếu được). Chưa đủ → None.
pub fn next_live_chunk(lines: &[Line], noted: usize, max_secs: f32, max_chars: usize) -> Option<(usize, usize)> {
    let rest = lines.get(noted..)?;
    let first = rest.first()?;
    let chars: usize = rest.iter().map(|l| l.text.chars().count() + 12).sum();
    let span = rest.last().map(|l| l.at - first.at).unwrap_or(0.0);
    if chars < max_chars && span < max_secs {
        return None;
    }
    chunk_ranges(rest, max_secs, max_chars).first().map(|(a, b)| (noted + a, noted + b))
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
        // "- [12:30] ý chính" → mốc để nghe lại
        let (at, body) = split_stamp(body.trim().trim_start_matches(['-', '•', '*', '–']).trim());
        match sec {
            2 => {
                if let Some(t) = clean_item(body) {
                    s.decisions.push(t);
                    s.decisions_at.push(at);
                }
            }
            3 => {
                if let Some(t) = clean_item(body) {
                    let parts: Vec<&str> = t.split('|').collect();
                    let (at2, task) = split_stamp(parts[0].trim());
                    s.actions.push(ActionItem {
                        task: task.trim().to_string(),
                        owner: parts.get(1).map(|x| none_field(x)).unwrap_or_default(),
                        due: parts.get(2).map(|x| none_field(x)).unwrap_or_default(),
                        at: at.or(at2),
                    });
                }
            }
            _ => {
                if let Some(t) = clean_item(body) {
                    s.overview.push(t);
                    s.overview_at.push(at);
                }
            }
        }
    }
    s
}

/// Đọc biên bản dạng JSON (Gemini): {"overview":[..],"decisions":[..],"actions":[{"task","owner","due"}]}
pub fn parse_summary_json(raw: &str) -> Option<Summary> {
    let a = raw.find('{')?;
    let b = raw.rfind('}')?;
    let v: serde_json::Value = serde_json::from_str(raw.get(a..=b)?).ok()?;
    // mỗi ý là chuỗi ("[12:30] ý") hoặc đối tượng {"text": "...", "at": "12:30" | 750}
    let strs = |k: &str| -> Vec<(String, Option<f32>)> {
        let one = |i: &serde_json::Value| -> Option<(String, Option<f32>)> {
            let (at0, raw) = match i {
                serde_json::Value::String(x) => (None, x.as_str()),
                serde_json::Value::Object(_) => (stamp_value(&i["at"]), i["text"].as_str().or_else(|| i["point"].as_str())?),
                _ => return None,
            };
            let (at1, body) = split_stamp(raw.trim());
            clean_item(body).map(|t| (t, at0.or(at1)))
        };
        match &v[k] {
            serde_json::Value::Array(x) => x.iter().filter_map(one).collect(),
            serde_json::Value::String(x) => x.lines().filter_map(|l| one(&serde_json::Value::String(l.to_string()))).collect(),
            _ => vec![],
        }
    };
    let actions = v["actions"]
        .as_array()
        .map(|x| {
            x.iter()
                .filter_map(|i| {
                    let (at1, task) = split_stamp(i["task"].as_str().or_else(|| i.as_str())?.trim());
                    let task = task.trim().to_string();
                    (!task.is_empty()).then(|| ActionItem { task, owner: none_field(i["owner"].as_str().unwrap_or("")), due: none_field(i["due"].as_str().unwrap_or("")), at: stamp_value(&i["at"]).or(at1) })
                })
                .collect()
        })
        .unwrap_or_default();
    let (overview, overview_at) = strs("overview").into_iter().unzip();
    let (decisions, decisions_at) = strs("decisions").into_iter().unzip();
    let s = Summary { overview, overview_at, decisions, decisions_at, actions, ..Default::default() };
    (!s.overview.is_empty() || !s.actions.is_empty() || !s.decisions.is_empty()).then_some(s)
}

/// "[12:30] ý chính" / "(1:02:05) …" / "12:30 – …" ở đầu dòng → (mốc giây, phần còn lại)
pub fn split_stamp(s: &str) -> (Option<f32>, &str) {
    let t = s.trim_start();
    let (inner, rest) = if let Some(r) = t.strip_prefix('[') {
        match r.split_once(']') {
            Some(x) => x,
            None => return (None, s),
        }
    } else if let Some(r) = t.strip_prefix('(') {
        match r.split_once(')') {
            Some(x) => x,
            None => return (None, s),
        }
    } else {
        let end = t.find(|c: char| !(c.is_ascii_digit() || c == ':')).unwrap_or(t.len());
        (&t[..end], &t[end..])
    };
    match parse_clock(inner.trim()) {
        Some(v) => (Some(v), rest.trim_start_matches([' ', '-', '–', ':', '·']).trim_start()),
        None => (None, s),
    }
}

/// "12:30" → 750 giây; "1:02:05" → 3725
pub fn parse_clock(s: &str) -> Option<f32> {
    let parts: Vec<&str> = s.split(':').collect();
    if !(2..=3).contains(&parts.len()) || parts.iter().any(|p| p.is_empty() || p.len() > 2 || !p.chars().all(|c| c.is_ascii_digit())) {
        return None;
    }
    let n: Vec<u32> = parts.iter().map(|p| p.parse().unwrap_or(0)).collect();
    let secs = if n.len() == 3 { n[0] * 3600 + n[1] * 60 + n[2] } else { n[0] * 60 + n[1] };
    (n[n.len() - 1] < 60).then_some(secs as f32)
}

fn stamp_value(v: &serde_json::Value) -> Option<f32> {
    match v {
        serde_json::Value::Number(n) => n.as_f64().map(|x| x as f32).filter(|x| *x >= 0.0),
        serde_json::Value::String(x) => parse_clock(x.trim().trim_matches(['[', ']'])),
        _ => None,
    }
}

impl Summary {
    /// Bảo đảm `overview_at` / `decisions_at` đủ dài; ý chưa có mốc nhận `fallback` (vd. giây bắt đầu của khúc)
    pub fn fill_stamps(mut self, fallback: Option<f32>) -> Summary {
        self.overview_at.resize(self.overview.len(), None);
        self.decisions_at.resize(self.decisions.len(), None);
        for a in self.overview_at.iter_mut().chain(self.decisions_at.iter_mut()) {
            if a.is_none() {
                *a = fallback;
            }
        }
        for a in self.actions.iter_mut() {
            if a.at.is_none() {
                a.at = fallback;
            }
        }
        self
    }
    pub fn is_empty(&self) -> bool {
        self.overview.is_empty() && self.decisions.is_empty() && self.actions.is_empty()
    }
}

/// Chia toàn văn thành các khúc để AI đọc: mỗi khúc ≤ `max_secs` giây lời nói và ≤ `max_chars` ký tự (mô hình trên máy chỉ
/// đọc được ~4096 token). Ưu tiên cắt ở chỗ đổi người nói khi khúc đã dài quá nửa — tức là
/// tóm tắt theo phiên nói nếu phiên ngắn. Trả về (dòng đầu, dòng cuối + 1).
pub fn chunk_ranges(lines: &[Line], max_secs: f32, max_chars: usize) -> Vec<(usize, usize)> {
    let mut out = vec![];
    let mut a = 0;
    while a < lines.len() {
        let mut chars = 0usize;
        let mut b = a;
        let mut soft: Option<usize> = None; // chỗ cắt đẹp gần nhất
        while b < lines.len() {
            let n = lines[b].text.chars().count() + 12;
            let span = lines[b].at - lines[a].at;
            if b > a && (chars + n > max_chars || span > max_secs) {
                break;
            }
            if b > a {
                // đổi người nói = hết một phiên nói (mỗi dòng đã là một đoạn ~30–45 giây nên không đo được chỗ ngừng)
                let turn = lines[b].who != lines[b - 1].who;
                if turn && (chars * 2 >= max_chars || span * 2.0 >= max_secs) {
                    soft = Some(b);
                }
            }
            chars += n;
            b += 1;
        }
        let end = if b < lines.len() { soft.filter(|s| *s > a).unwrap_or(b) } else { b };
        out.push((a, end.max(a + 1)));
        a = end.max(a + 1);
    }
    out
}

/// Lỗi "quá dài" của Apple Intelligence (vượt cửa sổ ngữ cảnh) → nên chia nhỏ khúc rồi thử lại
pub fn is_too_long(err: &str) -> bool {
    let f = fold(err);
    ["context", "exceeded", "too long", "token", "qua dai", "vuot"].iter().any(|k| f.contains(k))
}

/// Đọc danh sách đánh số "1. …\n2. …" (kết quả dịch theo dòng). Đúng `n` dòng mới nhận.
pub fn parse_numbered(raw: &str, n: usize) -> Option<Vec<String>> {
    let mut out: Vec<(usize, String)> = vec![];
    for l in raw.lines() {
        let t = l.trim().trim_matches('*').trim();
        let d = t.chars().take_while(|c| c.is_ascii_digit()).count();
        if d == 0 || d > 3 {
            continue;
        }
        let rest = t[d..].trim_start_matches(['.', ')', ':', '-']).trim();
        if let Ok(i) = t[..d].parse::<usize>() {
            if (1..=n).contains(&i) && !rest.is_empty() && !out.iter().any(|x| x.0 == i) {
                out.push((i, rest.to_string()));
            }
        }
    }
    if out.len() != n {
        return None;
    }
    out.sort_by_key(|x| x.0);
    Some(out.into_iter().map(|x| x.1).collect())
}

/// Toàn văn Gemini nghe lại từ file ghi âm: JSON [{"at": "mm:ss" | giây, "who": "Người 1", "text": "…"}] → dòng (cộng `offset`)
pub fn parse_transcript_json(raw: &str, offset: f32) -> Vec<Line> {
    let (Some(a), Some(b)) = (raw.find('['), raw.rfind(']')) else { return vec![] };
    let Ok(serde_json::Value::Array(items)) = serde_json::from_str::<serde_json::Value>(&raw[a..=b]) else { return vec![] };
    items
        .iter()
        .filter_map(|i| {
            let text = i["text"].as_str()?.trim().to_string();
            if text.is_empty() {
                return None;
            }
            let who = i["who"].as_str().or_else(|| i["speaker"].as_str()).unwrap_or("").trim().chars().take(30).collect::<String>();
            Some(Line { at: offset + stamp_value(&i["at"]).unwrap_or(0.0), who, text })
        })
        .collect()
}

/// Ghi chép đầy đủ dạng chữ cho tab Ghi chú
pub fn full_note_text(m: &Meeting) -> String {
    let en = is_en(&m.locale);
    let when = chrono::DateTime::from_timestamp_millis(m.started).map(|d| d.with_timezone(&chrono::Local).format("%d/%m/%Y %H:%M").to_string()).unwrap_or_default();
    let mut o = format!("{}\n{} · {when} · {}\n", m.title, if en { "Full meeting notes" } else { "Ghi chép đầy đủ" }, clock(m.duration as f32));
    if let Some(f) = &m.full {
        for p in &f.parts {
            o.push_str(&format!("\n[{}] {}\n", clock(p.at), p.text.trim()));
        }
    }
    o.push_str(if en { "\nReplay the recording: Mini-Asi › Meetings" } else { "\nNghe lại ghi âm: Mini-Asi › Họp" });
    o
}

/// Gộp kết quả tóm tắt của nhiều khúc: bỏ trùng (so sánh không dấu)
pub fn merge(parts: Vec<Summary>) -> Summary {
    let mut out = Summary::default();
    let key = |s: &str| norm_words(s).join(" ");
    for p in parts {
        let p = p.fill_stamps(None);
        for (o, at) in p.overview.into_iter().zip(p.overview_at) {
            if !out.overview.iter().any(|x| key(x) == key(&o)) {
                out.overview.push(o);
                out.overview_at.push(at);
            }
        }
        for (d, at) in p.decisions.into_iter().zip(p.decisions_at) {
            if !out.decisions.iter().any(|x| key(x) == key(&d)) {
                out.decisions.push(d);
                out.decisions_at.push(at);
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
        let stamp = |xs: &[String], at: &[Option<f32>]| -> Vec<String> { xs.iter().enumerate().map(|(i, x)| match at.get(i).copied().flatten() { Some(t) => format!("[{}] {x}", clock(t)), None => x.clone() }).collect() };
        list(&mut o, l("Tóm tắt", "Summary"), &stamp(&s.overview, &s.overview_at));
        list(&mut o, l("Quyết định", "Decisions"), &stamp(&s.decisions, &s.decisions_at));
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
    if let Some(t) = &m.translation {
        let ten = t.lang == "en";
        o.push_str(if ten { "\n— English —\n" } else { "\n— Tiếng Việt —\n" });
        let list2 = |o: &mut String, xs: &[String]| {
            for x in xs {
                o.push_str(&format!("• {x}\n"));
            }
        };
        list2(&mut o, &t.summary.overview);
        if !t.summary.decisions.is_empty() {
            o.push_str(if ten { "Decisions\n" } else { "Quyết định\n" });
            list2(&mut o, &t.summary.decisions);
        }
        if !t.summary.actions.is_empty() {
            o.push_str(if ten { "Action items\n" } else { "Việc cần làm\n" });
            for a in &t.summary.actions {
                o.push_str(&format!("☐ {}\n", a.task));
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
        assert_eq!(s.actions[0], ActionItem { task: "Gửi báo giá cho Hoiana".into(), owner: "anh Tuấn".into(), due: "thứ 6".into(), at: None });
        assert_eq!(s.actions[1], ActionItem { task: "Làm brief video".into(), owner: "".into(), due: "".into(), at: None });
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
        // mỗi dòng 30 giây, 300 ký tự → khúc 3 phút = 6 dòng (≤ 180 giây tính từ dòng đầu)
        let lines: Vec<Line> = (0..10).map(|i| Line { at: i as f32 * 30.0, who: "".into(), text: "x".repeat(288) }).collect();
        assert_eq!(next_live_chunk(&lines, 0, 180.0, 4000), Some((0, 7)));
        assert_eq!(next_live_chunk(&lines, 7, 180.0, 4000), None, "chưa đủ 3 phút");
        assert_eq!(next_live_chunk(&[], 0, 180.0, 4000), None);
        // giới hạn ký tự đến trước
        assert_eq!(next_live_chunk(&lines, 0, 180.0, 1000), Some((0, 3)));
    }

    #[test]
    fn audio_split_and_wav_roundtrip() {
        let mut x = tone(70.0, 0.3);
        for i in 0..(RATE as usize / 2) {
            x[(40.0 * RATE as f32) as usize + i] = 0.0; // lặng ở giây 40
        }
        let parts = split_audio(&x, 30.0, 45.0);
        assert_eq!(parts.len(), 2);
        assert!((parts[0].0 - 0.0).abs() < 0.01 && (parts[1].0 - 40.2).abs() < 0.6, "{:?}", parts.iter().map(|p| p.0).collect::<Vec<_>>());
        assert_eq!(parts.iter().map(|p| p.1.len()).sum::<usize>(), x.len());
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("a.wav");
        write_wav16(&p, &x[..16000]).unwrap();
        let back = read_wav_16k(&p).unwrap();
        assert_eq!(back.len(), 16000);
        assert!((back[100] - x[100]).abs() < 0.001);
    }

    #[test]
    fn chunks_by_time_and_speaker() {
        // 2 người nói xen kẽ theo phiên: A 0–100 s, B 110–200 s, A 210–… → cắt ở chỗ đổi người khi khúc đã dài quá nửa
        let mut lines = vec![];
        for i in 0..6 {
            lines.push(Line { at: i as f32 * 20.0, who: "me".into(), text: "a".repeat(50) });
        }
        for i in 0..5 {
            lines.push(Line { at: 120.0 + i as f32 * 20.0, who: "others".into(), text: "b".repeat(50) });
        }
        let r = chunk_ranges(&lines, 180.0, 5000);
        assert_eq!(r[0], (0, 6), "cắt đúng chỗ B bắt đầu nói");
        assert_eq!(r.last().unwrap().1, lines.len());
        // không có chỗ đổi người → cắt theo thời gian
        let flat: Vec<Line> = (0..20).map(|i| Line { at: i as f32 * 20.0, who: "".into(), text: "c".repeat(40) }).collect();
        let r = chunk_ranges(&flat, 180.0, 5000);
        assert!(r.iter().all(|(a, b)| flat[b - 1].at - flat[*a].at <= 180.0));
        assert_eq!(r.iter().map(|(a, b)| b - a).sum::<usize>(), 20);
        // một dòng quá dài vẫn thành một khúc (không lặp vô hạn)
        let big = vec![Line { at: 0.0, who: "".into(), text: "d".repeat(9000) }];
        assert_eq!(chunk_ranges(&big, 180.0, 2000), vec![(0, 1)]);
    }

    #[test]
    fn stamps_numbered_and_gemini_transcript() {
        assert_eq!(split_stamp("[12:30] Chốt ngân sách"), (Some(750.0), "Chốt ngân sách"));
        assert_eq!(split_stamp("(1:02:05) – Kết thúc"), (Some(3725.0), "Kết thúc"));
        assert_eq!(split_stamp("03:10 - Gửi báo giá"), (Some(190.0), "Gửi báo giá"));
        assert_eq!(split_stamp("Quý 4 tăng 10%"), (None, "Quý 4 tăng 10%"));
        assert_eq!(split_stamp("2024 là năm tốt"), (None, "2024 là năm tốt"));
        let s = parse_summary_text("TÓM TẮT:\n- [00:40] Chốt kế hoạch Q4\n- Bàn về TikTok\nQUYẾT ĐỊNH:\n- [02:10] Chạy thử 2 tuần\nVIỆC CẦN LÀM:\n- [03:00] Gửi báo giá | Tuấn | thứ 6");
        assert_eq!(s.overview, vec!["Chốt kế hoạch Q4", "Bàn về TikTok"]);
        assert_eq!(s.overview_at, vec![Some(40.0), None]);
        assert_eq!(s.decisions_at, vec![Some(130.0)]);
        assert_eq!((s.actions[0].task.as_str(), s.actions[0].at), ("Gửi báo giá", Some(180.0)));
        let f = s.clone().fill_stamps(Some(5.0));
        assert_eq!(f.overview_at, vec![Some(40.0), Some(5.0)]);
        let j = parse_summary_json(r#"{"overview":[{"text":"Chốt Q4","at":"01:05"},"[02:00] Ý hai"],"decisions":[],"actions":[{"task":"Gửi file","owner":"","due":"","at":95}]}"#).unwrap();
        assert_eq!(j.overview_at, vec![Some(65.0), Some(120.0)]);
        assert_eq!(j.actions[0].at, Some(95.0));
        // dịch theo dòng
        assert_eq!(parse_numbered("1. Close Q4 plan\n2) Try TikTok\n", 2), Some(vec!["Close Q4 plan".into(), "Try TikTok".into()]));
        assert_eq!(parse_numbered("1. Only one", 2), None);
        // Gemini nghe lại
        let t = parse_transcript_json(r#"Đây: [{"at":"00:05","who":"Người 1","text":"Chào cả nhà"},{"at":12,"who":"Người 2","text":"Chào anh"},{"at":"0:20","text":""}]"#, 300.0);
        assert_eq!(t.len(), 2);
        assert_eq!((t[0].at, t[0].who.as_str(), t[1].at), (305.0, "Người 1", 312.0));
        assert!(is_too_long("Exceeded model context window size") && !is_too_long("network down"));
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
            summary: Some(Summary { overview: vec!["Ý chính".into()], actions: vec![ActionItem { task: "Gửi file".into(), owner: "Lan".into(), due: "mai".into(), at: None }], ..Default::default() }),
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
