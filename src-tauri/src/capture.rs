//! "Thả vào tai thỏ" → việc / lịch.
//!
//! Người dùng kéo một nội dung bất kỳ (đoạn chữ đang chọn, tin nhắn trong app chat, ảnh/ảnh chụp màn hình,
//! danh bạ, đường link) vào tai thỏ. Module này (thuần, có test) đọc nội dung đó thành một **đề xuất**:
//! tiêu đề, ngày giờ, địa điểm, ghi chú, nên là *việc* hay *sự kiện lịch*. Người dùng xem & sửa lại rồi mới tạo.
//! Chữ trong ảnh được đọc bằng Vision trên máy (notch-native::ocr) trước khi tới đây.

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime};
use serde::{Deserialize, Serialize};

use crate::nlp;

/// Đề xuất hiển thị trong bảng "Tạo từ nội dung thả vào"
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Draft {
    /// "task" | "event"
    pub kind: String,
    pub title: String,
    /// Giờ địa phương (chưa đổi sang ms — lệnh sẽ đổi theo múi giờ của máy)
    #[serde(skip)]
    pub start: Option<NaiveDateTime>,
    #[serde(skip)]
    pub end: Option<NaiveDateTime>,
    pub all_day: bool,
    pub location: String,
    pub notes: String,
    /// Ai đã đọc: "rule" (quy tắc trên máy) hoặc tên bộ não AI
    pub engine: String,
}

pub const MAX_NOTES: usize = 1500;
const MAX_TITLE: usize = 90;

// ───────────────────────── danh bạ (vCard) ─────────────────────────

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Contact {
    pub name: String,
    pub org: String,
    pub phones: Vec<String>,
    pub emails: Vec<String>,
    /// (tháng, ngày)
    #[serde(skip)]
    pub birthday: Option<(u32, u32)>,
}

/// Đọc vCard 2.1/3.0/4.0 (có thể nhiều người một lúc). Bỏ qua ảnh đại diện và các trường lạ.
pub fn parse_vcard(s: &str) -> Vec<Contact> {
    // Gộp dòng gập (dòng tiếp theo bắt đầu bằng khoảng trắng)
    let mut lines: Vec<String> = vec![];
    for raw in s.replace("\r\n", "\n").split('\n') {
        if (raw.starts_with(' ') || raw.starts_with('\t')) && !lines.is_empty() {
            lines.last_mut().unwrap().push_str(&raw[1..]);
        } else {
            lines.push(raw.to_string());
        }
    }
    let mut out = vec![];
    let mut cur: Option<Contact> = None;
    let mut n_field = String::new();
    for l in lines {
        let Some((key, val)) = l.split_once(':') else { continue };
        let up = key.to_ascii_uppercase();
        // "item1.TEL;type=CELL" → "TEL"
        let name = up.split(';').next().unwrap_or("").rsplit('.').next().unwrap_or("").to_string();
        let val = unescape(val.trim());
        match name.as_str() {
            "BEGIN" if val.eq_ignore_ascii_case("VCARD") => {
                cur = Some(Contact::default());
                n_field.clear();
            }
            "END" if val.eq_ignore_ascii_case("VCARD") => {
                if let Some(mut c) = cur.take() {
                    if c.name.is_empty() {
                        // N:Họ;Tên;Đệm;; → "Tên Đệm Họ" kiểu quốc tế, nhưng người Việt quen "Họ Đệm Tên"
                        let p: Vec<&str> = n_field.split(';').map(str::trim).collect();
                        let (fam, giv, mid) = (p.first().copied().unwrap_or(""), p.get(1).copied().unwrap_or(""), p.get(2).copied().unwrap_or(""));
                        c.name = [fam, mid, giv].iter().filter(|x| !x.is_empty()).copied().collect::<Vec<_>>().join(" ");
                    }
                    if c.name.is_empty() {
                        c.name = if !c.org.is_empty() { c.org.clone() } else { c.phones.first().or(c.emails.first()).cloned().unwrap_or_default() };
                    }
                    if !c.name.is_empty() {
                        out.push(c);
                    }
                }
            }
            _ => {
                let Some(c) = cur.as_mut() else { continue };
                match name.as_str() {
                    "FN" if !val.is_empty() => c.name = val,
                    "N" => n_field = val,
                    "ORG" => c.org = val.split(';').filter(|x| !x.trim().is_empty()).collect::<Vec<_>>().join(" · "),
                    "TEL" if !val.is_empty() => c.phones.push(val.trim_start_matches("tel:").to_string()),
                    "EMAIL" if !val.is_empty() => c.emails.push(val),
                    "BDAY" => c.birthday = parse_bday(&val),
                    _ => {}
                }
            }
        }
    }
    out
}

fn unescape(s: &str) -> String {
    s.replace("\\n", "\n").replace("\\N", "\n").replace("\\,", ",").replace("\\;", ";").replace("\\\\", "\\")
}

/// "1990-05-17", "19900517", "--0517", "--05-17" → (5, 17)
fn parse_bday(v: &str) -> Option<(u32, u32)> {
    let d: String = v.chars().take_while(|c| *c != 'T').filter(|c| c.is_ascii_digit()).collect();
    let md = if d.len() >= 8 { &d[4..8] } else if d.len() == 4 { &d[..] } else { return None };
    let m: u32 = md[..2].parse().ok()?;
    let day: u32 = md[2..].parse().ok()?;
    ((1..=12).contains(&m) && (1..=31).contains(&day)).then_some((m, day))
}

/// Danh bạ → đề xuất: có ngày sinh → sự kiện cả ngày "Sinh nhật …" lần tới; không thì việc "Liên hệ …".
pub fn from_contact(c: &Contact, now: NaiveDateTime) -> Draft {
    let mut notes = vec![];
    if !c.org.is_empty() {
        notes.push(c.org.clone());
    }
    notes.extend(c.phones.iter().map(|p| format!("☎ {p}")));
    notes.extend(c.emails.iter().map(|e| format!("✉ {e}")));
    let notes = notes.join("\n");
    if let Some((m, d)) = c.birthday {
        let mut date = NaiveDate::from_ymd_opt(now.year(), m, d).or_else(|| NaiveDate::from_ymd_opt(now.year(), 3, 1));
        if let Some(x) = date {
            if x < now.date() {
                date = NaiveDate::from_ymd_opt(now.year() + 1, m, d).or_else(|| NaiveDate::from_ymd_opt(now.year() + 1, 3, 1));
            }
        }
        if let Some(date) = date {
            let start = date.and_time(NaiveTime::MIN);
            return Draft { kind: "event".into(), title: format!("Sinh nhật {}", c.name), start: Some(start), end: Some(start + Duration::days(1)), all_day: true, location: String::new(), notes, engine: "rule".into() };
        }
    }
    Draft { kind: "task".into(), title: format!("Liên hệ {}", c.name), notes, engine: "rule".into(), ..Default::default() }
}

// ───────────────────────── chữ / ảnh / link ─────────────────────────

/// Có ghi giờ rõ ràng không ("9h", "14:30", "3 giờ", "9am") — có giờ thì nên là sự kiện lịch.
pub fn has_clock(text: &str) -> bool {
    let f = nlp::fold(text);
    let b = f.as_bytes();
    for i in 0..b.len() {
        if !b[i].is_ascii_digit() || (i > 0 && b[i - 1].is_ascii_digit()) {
            continue;
        }
        let mut j = i;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        let n: u32 = f[i..j].parse().unwrap_or(99);
        if n > 23 || j - i > 2 {
            continue;
        }
        let rest = &f[j..];
        let rest_t = rest.trim_start();
        if rest.starts_with(':') && rest[1..].chars().take(2).all(|c| c.is_ascii_digit()) && rest.len() >= 3 {
            return true;
        }
        if rest.starts_with('h') && !rest[1..].starts_with(|c: char| c.is_ascii_alphabetic()) {
            return true;
        }
        if rest.starts_with("h") && rest[1..].starts_with(|c: char| c.is_ascii_digit()) {
            return true;
        }
        if rest_t.starts_with("gio") || rest_t.starts_with("am") || rest_t.starts_with("pm") {
            return true;
        }
    }
    false
}

/// Bỏ "Tin nhắn từ …", dấu thời gian kiểu chat ("[10:21]"), dòng trống → các dòng có nội dung.
fn meaningful_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| l.trim().trim_start_matches(['-', '•', '*', '>', '–']).trim().to_string())
        .filter(|l| l.chars().filter(|c| c.is_alphanumeric()).count() >= 2)
        .take(40)
        .collect()
}

fn find_location(lines: &[String]) -> String {
    const KEYS: &[&str] = &["dia diem", "dia chi", "location", "address", "venue", "tai:", "o dau"];
    for l in lines {
        let f = nlp::fold(l);
        for k in KEYS {
            if f.starts_with(k) {
                // phần sau dấu ":" (nếu có)
                let v = l.split_once(':').map(|x| x.1).unwrap_or_else(|| l.get(k.len()..).unwrap_or(""));
                let v = v.trim();
                if !v.is_empty() {
                    return truncate(v, 120);
                }
            }
        }
    }
    String::new()
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_string();
    }
    let mut t: String = s.chars().take(n).collect();
    if let Some(i) = t.rfind(' ') {
        if i > n / 2 {
            t.truncate(i);
        }
    }
    format!("{}…", t.trim_end())
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Chữ (đoạn chọn / tin nhắn / chữ đọc từ ảnh) → đề xuất, chỉ bằng quy tắc trên máy.
pub fn from_text(text: &str, url: Option<&str>, now: NaiveDateTime) -> Draft {
    let lines = meaningful_lines(text);
    let mut due = None;
    let mut due_line = None;
    for (i, l) in lines.iter().enumerate().take(15) {
        let p = nlp::parse(l, now);
        if p.due.is_some() {
            due = p.due;
            due_line = Some((i, p.title));
            break;
        }
    }
    let clock = due_line.as_ref().map(|(i, _)| has_clock(&lines[*i])).unwrap_or(false);
    // Tiêu đề: dòng có hẹn ngày (đã bỏ phần ngày giờ) nếu đủ nghĩa, không thì dòng đầu
    let title = match &due_line {
        Some((_, t)) if t.chars().filter(|c| c.is_alphabetic()).count() >= 4 => t.clone(),
        _ => lines.first().cloned().unwrap_or_default(),
    };
    let title = if title.is_empty() {
        url.map(|u| format!("Xem lại {}", host_of(u))).unwrap_or_default()
    } else {
        title
    };
    let title = capitalize(&truncate(title.trim_end_matches([':', ',', '.']), MAX_TITLE));
    let mut notes = text.trim().to_string();
    if let Some(u) = url {
        if !notes.contains(u) {
            notes = if notes.is_empty() { u.to_string() } else { format!("{notes}\n{u}") };
        }
    }
    let notes = truncate(&notes, MAX_NOTES);
    let location = find_location(&lines);
    match due {
        Some(d) if clock => Draft { kind: "event".into(), title, start: Some(d), end: Some(d + Duration::hours(1)), all_day: false, location, notes, engine: "rule".into() },
        Some(d) => Draft { kind: "task".into(), title, start: Some(d), end: None, all_day: false, location, notes, engine: "rule".into() },
        None => Draft { kind: "task".into(), title, location, notes, engine: "rule".into(), ..Default::default() },
    }
}

pub fn host_of(url: &str) -> String {
    let s = url.split("://").nth(1).unwrap_or(url);
    s.split(['/', '?', '#']).next().unwrap_or(s).trim_start_matches("www.").to_string()
}

// ───────────────────────── AI (tuỳ chọn) ─────────────────────────

/// Lời dặn cho bộ não AI: chỉ trích thông tin, trả JSON, không bịa ngày.
pub fn ai_system(now: NaiveDateTime) -> String {
    format!(
        "Bạn trích xuất thông tin để tạo VIỆC hoặc SỰ KIỆN LỊCH từ nội dung người dùng kéo vào (tin nhắn, đoạn văn, chữ đọc từ ảnh). \
Bây giờ là {} ({}). Trả lời DUY NHẤT một đối tượng JSON: \
{{\"kind\":\"task\"|\"event\",\"title\":\"ngắn gọn ≤ 70 ký tự, tiếng Việt nếu nội dung tiếng Việt\",\"date\":\"YYYY-MM-DD hoặc rỗng\",\"time\":\"HH:MM hoặc rỗng\",\"end_time\":\"HH:MM hoặc rỗng\",\"all_day\":true|false,\"location\":\"địa điểm hoặc rỗng\",\"notes\":\"tóm tắt 1-3 dòng thông tin quan trọng (số điện thoại, mã đặt chỗ…) hoặc rỗng\"}}. \
Chọn \"event\" khi có cuộc hẹn/lịch họp/chuyến bay/sự kiện ở một thời điểm; \"task\" khi là việc cần làm. \
KHÔNG bịa ngày giờ: nội dung không nói ngày thì để rỗng. Đổi \"mai\", \"thứ 6\"… thành ngày cụ thể.",
        now.format("%Y-%m-%d %H:%M"),
        weekday_vi(now.date())
    )
}

fn weekday_vi(d: NaiveDate) -> &'static str {
    ["Thứ Hai", "Thứ Ba", "Thứ Tư", "Thứ Năm", "Thứ Sáu", "Thứ Bảy", "Chủ Nhật"][d.weekday().num_days_from_monday() as usize]
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct AiDraft {
    kind: String,
    title: String,
    date: String,
    time: String,
    end_time: String,
    all_day: bool,
    location: String,
    notes: String,
}

/// Đọc JSON AI trả về (có thể kèm chữ thừa / ```json) → đề xuất. Giữ ghi chú gốc nếu AI không tóm tắt.
pub fn parse_ai(raw: &str, original_notes: &str, engine: &str) -> Option<Draft> {
    let a = raw.find('{')?;
    let b = raw.rfind('}')?;
    let v: AiDraft = serde_json::from_str(raw.get(a..=b)?).ok()?;
    let title = v.title.trim();
    if title.is_empty() {
        return None;
    }
    let date = NaiveDate::parse_from_str(v.date.trim(), "%Y-%m-%d").ok();
    let time = NaiveTime::parse_from_str(v.time.trim(), "%H:%M").ok();
    let end_t = NaiveTime::parse_from_str(v.end_time.trim(), "%H:%M").ok();
    let all_day = v.all_day || (date.is_some() && time.is_none() && v.kind == "event");
    let start = date.map(|d| d.and_time(if all_day { NaiveTime::MIN } else { time.unwrap_or(NaiveTime::from_hms_opt(9, 0, 0).unwrap()) }));
    let end = match (date, start) {
        (Some(d), Some(s)) if all_day => Some(s.max(d.and_time(NaiveTime::MIN)) + Duration::days(1)),
        (Some(d), Some(s)) => Some(end_t.map(|t| d.and_time(t)).filter(|e| *e > s).unwrap_or(s + Duration::hours(1))),
        _ => None,
    };
    let kind = if v.kind == "event" && start.is_some() { "event" } else { "task" };
    let mut notes = v.notes.trim().to_string();
    let orig = original_notes.trim();
    if !orig.is_empty() && notes != orig {
        notes = if notes.is_empty() { orig.to_string() } else { format!("{notes}\n—\n{orig}") };
    }
    Some(Draft {
        kind: kind.into(),
        title: capitalize(&truncate(title, MAX_TITLE)),
        start,
        end: if kind == "event" { end } else { None },
        all_day: kind == "event" && all_day,
        location: truncate(v.location.trim(), 120),
        notes: truncate(&notes, MAX_NOTES),
        engine: engine.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> NaiveDateTime {
        // Thứ Tư 4/11/2026 10:00
        NaiveDate::from_ymd_opt(2026, 11, 4).unwrap().and_hms_opt(10, 0, 0).unwrap()
    }

    #[test]
    fn vcard_basic_and_folded() {
        let v = "BEGIN:VCARD\r\nVERSION:3.0\r\nN:Nguyễn;An;Văn;;\r\nFN:Nguyễn Văn An\r\nORG:Trốn Glamping;\r\nitem1.TEL;type=CELL:0901 234 567\r\nEMAIL;type=INTERNET:an@example.com\r\nBDAY:1990-12-20\r\nNOTE:dòng dài\r\n  tiếp theo\r\nEND:VCARD\r\nBEGIN:VCARD\nN:Trần;Bình;;;\nTEL:+84 912\nEND:VCARD\n";
        let c = parse_vcard(v);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].name, "Nguyễn Văn An");
        assert_eq!(c[0].org, "Trốn Glamping");
        assert_eq!(c[0].phones, vec!["0901 234 567"]);
        assert_eq!(c[0].emails, vec!["an@example.com"]);
        assert_eq!(c[0].birthday, Some((12, 20)));
        assert_eq!(c[1].name, "Trần Bình");
        assert_eq!(parse_bday("--0517"), Some((5, 17)));
        assert_eq!(parse_bday("19900517T000000"), Some((5, 17)));
        assert_eq!(parse_bday("abc"), None);
    }

    #[test]
    fn contact_to_birthday_event_or_task() {
        let c = parse_vcard("BEGIN:VCARD\nFN:Mai\nBDAY:1995-03-02\nTEL:090\nEND:VCARD");
        let d = from_contact(&c[0], now());
        assert_eq!(d.kind, "event");
        assert_eq!(d.title, "Sinh nhật Mai");
        assert!(d.all_day);
        assert_eq!(d.start.unwrap().date(), NaiveDate::from_ymd_opt(2027, 3, 2).unwrap()); // đã qua năm nay → năm sau
        assert!(d.notes.contains("090"));
        let c = parse_vcard("BEGIN:VCARD\nFN:Lan\nEMAIL:lan@x.vn\nEND:VCARD");
        let d = from_contact(&c[0], now());
        assert_eq!((d.kind.as_str(), d.title.as_str()), ("task", "Liên hệ Lan"));
    }

    #[test]
    fn clock_detection() {
        for s in ["họp lúc 9h", "14:30 gặp khách", "3 giờ chiều", "9am call", "9h30 họp", "đón con 17h."] {
            assert!(has_clock(s), "{s}");
        }
        for s in ["thứ 6 nộp báo cáo", "phòng 203", "20/11 mua hoa", "mã 12345", "hihi"] {
            assert!(!has_clock(s), "{s}");
        }
    }

    #[test]
    fn chat_message_with_time_becomes_event() {
        let t = "Anh ơi\nmai 14h họp với đối tác bên Hoà Bình nhé\nĐịa điểm: 12 Lý Thái Tổ";
        let d = from_text(t, None, now());
        assert_eq!(d.kind, "event");
        assert_eq!(d.start, NaiveDate::from_ymd_opt(2026, 11, 5).unwrap().and_hms_opt(14, 0, 0));
        assert_eq!(d.end, NaiveDate::from_ymd_opt(2026, 11, 5).unwrap().and_hms_opt(15, 0, 0));
        assert!(d.title.starts_with("Họp với đối tác"), "{}", d.title);
        assert_eq!(d.location, "12 Lý Thái Tổ");
        assert!(d.notes.contains("Anh ơi"));
    }

    #[test]
    fn text_without_time_is_task() {
        let d = from_text("thứ 6 nộp báo cáo quý", None, now());
        assert_eq!(d.kind, "task");
        assert_eq!(d.title, "Nộp báo cáo quý");
        assert!(d.start.is_some());
        let d = from_text("Nhớ gửi hợp đồng cho chị Hà", None, now());
        assert_eq!((d.kind.as_str(), d.start), ("task", None));
        assert_eq!(d.title, "Nhớ gửi hợp đồng cho chị Hà");
    }

    #[test]
    fn url_only_and_long_title() {
        let d = from_text("", Some("https://www.example.com/a/b?x=1"), now());
        assert_eq!(d.title, "Xem lại example.com");
        assert!(d.notes.contains("https://www.example.com"));
        let long = "a ".repeat(100);
        assert!(from_text(&long, None, now()).title.chars().count() <= MAX_TITLE + 1);
    }

    #[test]
    fn ai_json_parsed_with_fallbacks() {
        let raw = "```json\n{\"kind\":\"event\",\"title\":\"bay đi Đà Nẵng VN123\",\"date\":\"2026-11-20\",\"time\":\"07:45\",\"end_time\":\"\",\"location\":\"Nội Bài\",\"notes\":\"Mã đặt chỗ ABC123\"}\n```";
        let d = parse_ai(raw, "gốc", "Gemini").unwrap();
        assert_eq!(d.kind, "event");
        assert_eq!(d.title, "Bay đi Đà Nẵng VN123");
        assert_eq!(d.start, NaiveDate::from_ymd_opt(2026, 11, 20).unwrap().and_hms_opt(7, 45, 0));
        assert_eq!(d.end, NaiveDate::from_ymd_opt(2026, 11, 20).unwrap().and_hms_opt(8, 45, 0));
        assert!(d.notes.starts_with("Mã đặt chỗ ABC123") && d.notes.ends_with("gốc"));
        // sự kiện có ngày, không giờ → cả ngày
        let d = parse_ai("{\"kind\":\"event\",\"title\":\"Hội chợ\",\"date\":\"2026-12-01\"}", "", "x").unwrap();
        assert!(d.all_day);
        assert_eq!(d.end.unwrap().date(), NaiveDate::from_ymd_opt(2026, 12, 2).unwrap());
        // "event" mà không có ngày → hạ xuống việc
        let d = parse_ai("{\"kind\":\"event\",\"title\":\"Gọi lại\"}", "", "x").unwrap();
        assert_eq!((d.kind.as_str(), d.end), ("task", None));
        assert!(parse_ai("không phải json", "", "x").is_none());
        assert!(parse_ai("{\"title\":\"\"}", "", "x").is_none());
    }
}
