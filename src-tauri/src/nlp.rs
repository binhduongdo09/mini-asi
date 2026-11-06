//! Hiểu câu nhắc việc tiếng Việt (có dấu hoặc không dấu).
//!
//! Ví dụ: "mai 9h gọi khách", "15h30 họp team", "thứ 6 nộp báo cáo",
//! "30 phút nữa uống nước", "tối nay 8h xem phim", "gọi chị Mai lúc 3h chiều",
//! "ngày 15 đóng tiền nhà", "20/11 mua hoa", "chủ nhật tuần sau đi Trốn".

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, Weekday};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Parsed {
    pub title: String,
    pub due: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Period {
    Morning,
    Noon,
    Afternoon,
    Evening,
    Night,
}

struct Tok {
    orig: String,
    f: String, // dạng bỏ dấu, chữ thường, bỏ dấu câu cuối
    used: bool,
}

/// Bỏ dấu + chữ thường. Nhận cả chữ dựng sẵn (NFC) lẫn chữ tổ hợp (NFD, hay gặp khi
/// gõ bằng một số bộ gõ hoặc dán từ tên file macOS).
pub fn fold(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    s.nfc()
        .collect::<String>()
        .to_lowercase()
        .chars()
        .filter(|c| !('\u{300}'..='\u{36F}').contains(c))
        .map(|c| match c {
            'à' | 'á' | 'ạ' | 'ả' | 'ã' | 'â' | 'ầ' | 'ấ' | 'ậ' | 'ẩ' | 'ẫ' | 'ă' | 'ằ' | 'ắ' | 'ặ' | 'ẳ' | 'ẵ' => 'a',
            'è' | 'é' | 'ẹ' | 'ẻ' | 'ẽ' | 'ê' | 'ề' | 'ế' | 'ệ' | 'ể' | 'ễ' => 'e',
            'ì' | 'í' | 'ị' | 'ỉ' | 'ĩ' => 'i',
            'ò' | 'ó' | 'ọ' | 'ỏ' | 'õ' | 'ô' | 'ồ' | 'ố' | 'ộ' | 'ổ' | 'ỗ' | 'ơ' | 'ờ' | 'ớ' | 'ợ' | 'ở' | 'ỡ' => 'o',
            'ù' | 'ú' | 'ụ' | 'ủ' | 'ũ' | 'ư' | 'ừ' | 'ứ' | 'ự' | 'ử' | 'ữ' => 'u',
            'ỳ' | 'ý' | 'ỵ' | 'ỷ' | 'ỹ' => 'y',
            'đ' => 'd',
            c => c,
        })
        .collect()
}

fn is_ascii_word(s: &str) -> bool {
    s.is_ascii()
}

fn num(s: &str) -> Option<u32> {
    if !s.is_empty() && s.len() <= 4 && s.chars().all(|c| c.is_ascii_digit()) {
        s.parse().ok()
    } else {
        None
    }
}

fn weekday_word(s: &str) -> Option<Weekday> {
    Some(match s {
        "2" | "hai" => Weekday::Mon,
        "3" | "ba" => Weekday::Tue,
        "4" | "tu" => Weekday::Wed,
        "5" | "nam" => Weekday::Thu,
        "6" | "sau" => Weekday::Fri,
        "7" | "bay" => Weekday::Sat,
        "8" => Weekday::Sun,
        _ => return None,
    })
}

fn eng_weekday(s: &str) -> Option<Weekday> {
    Some(match s {
        "monday" | "mon" => Weekday::Mon,
        "tuesday" | "tue" | "tues" => Weekday::Tue,
        "wednesday" | "wed" => Weekday::Wed,
        "thursday" | "thu" | "thur" | "thurs" => Weekday::Thu,
        "friday" | "fri" => Weekday::Fri,
        "saturday" | "sat" => Weekday::Sat,
        "sunday" | "sun" => Weekday::Sun,
        _ => return None,
    })
}

/// Tên tháng tiếng Anh ("may" bị bỏ qua vì dễ nhầm với động từ)
fn eng_month(s: &str) -> Option<u32> {
    Some(match s {
        "jan" | "january" => 1,
        "feb" | "february" => 2,
        "mar" | "march" => 3,
        "apr" | "april" => 4,
        "jun" | "june" => 6,
        "jul" | "july" => 7,
        "aug" | "august" => 8,
        "sep" | "sept" | "september" => 9,
        "oct" | "october" => 10,
        "nov" | "november" => 11,
        "dec" | "december" => 12,
        _ => return None,
    })
}

/// "20", "20th", "1st", "22nd", "3rd"
fn num_ord(s: &str) -> Option<u32> {
    let d = s.trim_end_matches("st").trim_end_matches("nd").trim_end_matches("rd").trim_end_matches("th");
    num(d)
}

fn eng_rel_unit(s: &str) -> Option<i64> {
    Some(match s {
        "minute" | "minutes" | "min" | "mins" => 1,
        "hour" | "hours" | "hr" | "hrs" => 60,
        "day" | "days" => 1440,
        "week" | "weeks" => 10080,
        _ => return None,
    })
}

fn month_date(now: NaiveDateTime, m: u32, d: u32) -> Option<NaiveDate> {
    let nd = NaiveDate::from_ymd_opt(now.year(), m, d)?;
    Some(if nd < now.date() { NaiveDate::from_ymd_opt(now.year() + 1, m, d).unwrap_or(nd) } else { nd })
}

const HONORIFICS: &[&str] = &["chi", "anh", "em", "co", "chu", "ban", "bac", "ong", "ba", "di", "thay", "be", "me", "bo"];

pub fn parse(input: &str, now: NaiveDateTime) -> Parsed {
    use unicode_normalization::UnicodeNormalization;
    // Chuẩn hoá về chữ dựng sẵn để "tối"/"mốt" so khớp đúng và tiêu đề hiển thị chuẩn
    let normalized: String = input.nfc().collect();
    let input = normalized.as_str();
    let mut t: Vec<Tok> = input
        .split_whitespace()
        .map(|w| {
            let f = fold(w.trim_end_matches(|c: char| ",.!?;".contains(c)));
            Tok { orig: w.to_string(), f, used: false }
        })
        .collect();
    let n = t.len();

    let mut rel: Option<Duration> = None;
    let mut day_offset: Option<i64> = None;
    let mut weekday: Option<Weekday> = None;
    let mut next_week = false;
    let mut date: Option<NaiveDate> = None;
    let mut dom: Option<u32> = None;
    let mut hour: Option<u32> = None;
    let mut minute: u32 = 0;
    let mut period: Option<Period> = None;

    let re_time = regex::Regex::new(r"^(\d{1,2})(?:h|g|:)(\d{1,2})?(?:p|ph|phut)?$").unwrap();
    let re_date = regex::Regex::new(r"^(\d{1,2})[/\-.](\d{1,2})(?:[/\-.](\d{2,4}))?$").unwrap();
    let re_ampm = regex::Regex::new(r"^(\d{1,2})(?::(\d{2}))?(am|pm|a\.m|p\.m)$").unwrap();
    let ampm = |h: u32, ap: &str| -> u32 {
        let pm = ap.starts_with('p');
        match (h % 12, pm) {
            (x, true) => x + 12,
            (x, false) => x,
        }
    };

    let f = |t: &Vec<Tok>, i: usize| -> String { t.get(i).map(|x| x.f.clone()).unwrap_or_default() };

    let mut i = 0;
    while i < n {
        let w = f(&t, i);
        // ═════════ TIẾNG ANH ═════════
        // "in 30 minutes", "after 2 hours", "in an hour", "in half an hour"
        if w == "in" || w == "after" {
            if let Some(v) = num(&f(&t, i + 1)) {
                if let Some(u) = eng_rel_unit(&f(&t, i + 2)) {
                    rel = Some(Duration::minutes(v as i64 * u));
                    for j in i..=i + 2 {
                        t[j].used = true;
                    }
                    i += 3;
                    continue;
                }
            }
            if matches!(f(&t, i + 1).as_str(), "an" | "a") {
                if let Some(u) = eng_rel_unit(&f(&t, i + 2)) {
                    rel = Some(Duration::minutes(u));
                    for j in i..=i + 2 {
                        t[j].used = true;
                    }
                    i += 3;
                    continue;
                }
            }
            if f(&t, i + 1) == "half" && f(&t, i + 2) == "an" && f(&t, i + 3) == "hour" {
                rel = Some(Duration::minutes(30));
                for j in i..=i + 3 {
                    t[j].used = true;
                }
                i += 4;
                continue;
            }
        }
        // "9am", "9:30pm", "9 am"
        if let Some(c) = re_ampm.captures(&w) {
            let h: u32 = c[1].parse().unwrap_or(99);
            if (1..=12).contains(&h) {
                hour = Some(ampm(h, &c[3]));
                minute = c.get(2).and_then(|m| m.as_str().parse().ok()).unwrap_or(0).min(59);
                t[i].used = true;
                i += 1;
                continue;
            }
        }
        if let Some(h) = num(&w) {
            let ap = f(&t, i + 1);
            if (1..=12).contains(&h) && matches!(ap.as_str(), "am" | "pm" | "a.m" | "p.m") {
                hour = Some(ampm(h, &ap));
                minute = 0;
                t[i].used = true;
                t[i + 1].used = true;
                i += 2;
                continue;
            }
        }
        // "nov 20", "november 20th"
        if let Some(m) = eng_month(&w) {
            if let Some(d) = num_ord(&f(&t, i + 1)) {
                if let Some(nd) = month_date(now, m, d) {
                    date = Some(nd);
                    t[i].used = true;
                    t[i + 1].used = true;
                    i += 2;
                    continue;
                }
            }
        }
        // "20 nov", "20th november"
        if let Some(d) = num_ord(&w) {
            if let Some(m) = eng_month(&f(&t, i + 1)) {
                if let Some(nd) = month_date(now, m, d) {
                    date = Some(nd);
                    t[i].used = true;
                    t[i + 1].used = true;
                    i += 2;
                    continue;
                }
            }
        }
        // thứ trong tuần: "friday", "next friday", "this friday"
        // "thu" vừa là "thứ" tiếng Việt ("thứ 6") vừa là Thursday → ưu tiên tiếng Việt khi có số/tên thứ đi sau
        let vn_thu = w == "thu" && weekday_word(&f(&t, i + 1)).is_some();
        if let (Some(wd), false) = (eng_weekday(&w), vn_thu) {
            weekday = Some(wd);
            t[i].used = true;
            if i > 0 && !t[i - 1].used {
                match t[i - 1].f.as_str() {
                    "next" => {
                        next_week = true;
                        t[i - 1].used = true;
                    }
                    "this" | "coming" => t[i - 1].used = true,
                    _ => {}
                }
            }
            i += 1;
            continue;
        }
        if w == "next" && f(&t, i + 1) == "week" {
            next_week = true;
            t[i].used = true;
            t[i + 1].used = true;
            i += 2;
            continue;
        }
        if w == "day" && f(&t, i + 1) == "after" && f(&t, i + 2) == "tomorrow" {
            day_offset = Some(2);
            for j in i..=i + 2 {
                t[j].used = true;
            }
            if i > 0 && t[i - 1].f == "the" {
                t[i - 1].used = true;
            }
            i += 3;
            continue;
        }
        {
            let mut hit = true;
            match w.as_str() {
                "today" => day_offset = Some(0),
                "tomorrow" | "tmr" | "tmrw" | "tomorow" => day_offset = Some(1),
                "tonight" => {
                    day_offset = Some(0);
                    period = Some(Period::Evening);
                }
                "noon" | "midday" => {
                    hour = Some(12);
                    minute = 0;
                }
                "midnight" => {
                    hour = Some(23);
                    minute = 59;
                }
                "morning" => period = Some(Period::Morning),
                "afternoon" => period = Some(Period::Afternoon),
                "evening" => period = Some(Period::Evening),
                "night" => period = Some(Period::Night),
                _ => hit = false,
            }
            if hit {
                t[i].used = true;
                if i > 0 && !t[i - 1].used && matches!(t[i - 1].f.as_str(), "this" | "in") {
                    t[i - 1].used = true;
                    if matches!(w.as_str(), "morning" | "afternoon" | "evening" | "night") && day_offset.is_none() {
                        day_offset = Some(0);
                    }
                }
                if i > 1 && t[i - 1].f == "the" && t[i - 2].f == "in" {
                    t[i - 1].used = true;
                    t[i - 2].used = true;
                }
                i += 1;
                continue;
            }
        }
        // ── "sau 30 phút", "30 phút nữa", "2 tiếng nữa"
        {
            let (start, k) = if w == "sau" && num(&f(&t, i + 1)).is_some() { (i + 1, true) } else { (i, false) };
            if let Some(v) = num(&f(&t, start)) {
                let unit = f(&t, start + 1);
                let has_nua = f(&t, start + 2) == "nua";
                let mins = match unit.as_str() {
                    "phut" | "p" | "ph" => Some(v as i64),
                    "tieng" | "gio" | "h" => Some(v as i64 * 60),
                    "ngay" => Some(v as i64 * 1440),
                    _ => None,
                };
                if let Some(m) = mins {
                    if k || has_nua {
                        rel = Some(Duration::minutes(m));
                        for j in i..=(start + 1 + has_nua as usize) {
                            t[j].used = true;
                        }
                        i = start + 2 + has_nua as usize;
                        continue;
                    }
                }
            }
        }
        // ── giờ: "9h", "9h30", "21:15", "9 giờ", "9 giờ 30", "9h rưỡi"
        if let Some(c) = re_time.captures(&w) {
            let h: u32 = c[1].parse().unwrap_or(99);
            if h <= 24 {
                hour = Some(h % 24);
                minute = c.get(2).and_then(|m| m.as_str().parse().ok()).unwrap_or(0).min(59);
                t[i].used = true;
                if f(&t, i + 1) == "ruoi" {
                    minute = 30;
                    t[i + 1].used = true;
                    i += 1;
                }
                i += 1;
                continue;
            }
        }
        if let Some(h) = num(&w) {
            let u = f(&t, i + 1);
            if (u == "gio" || u == "h") && h <= 24 {
                hour = Some(h % 24);
                t[i].used = true;
                t[i + 1].used = true;
                i += 2;
                if let Some(m) = num(&f(&t, i)) {
                    if m < 60 {
                        minute = m;
                        t[i].used = true;
                        i += 1;
                        if matches!(f(&t, i).as_str(), "phut" | "p") {
                            t[i].used = true;
                            i += 1;
                        }
                    }
                } else if f(&t, i) == "ruoi" {
                    minute = 30;
                    t[i].used = true;
                    i += 1;
                }
                continue;
            }
        }
        // ── ngày tháng: "20/11", "20/11/2026"
        if let Some(c) = re_date.captures(&w) {
            let d: u32 = c[1].parse().unwrap_or(0);
            let m: u32 = c[2].parse().unwrap_or(0);
            let y: i32 = c
                .get(3)
                .and_then(|y| y.as_str().parse::<i32>().ok())
                .map(|y| if y < 100 { 2000 + y } else { y })
                .unwrap_or(now.year());
            if let Some(nd) = NaiveDate::from_ymd_opt(y, m, d) {
                // không ghi năm và ngày đã qua → năm sau
                let nd = if c.get(3).is_none() && nd < now.date() {
                    NaiveDate::from_ymd_opt(y + 1, m, d).unwrap_or(nd)
                } else {
                    nd
                };
                date = Some(nd);
                t[i].used = true;
                i += 1;
                continue;
            }
        }
        // ── "ngày 15"
        if w == "ngay" {
            if let Some(d) = num(&f(&t, i + 1)) {
                if (1..=31).contains(&d) {
                    dom = Some(d);
                    t[i].used = true;
                    t[i + 1].used = true;
                    i += 2;
                    continue;
                }
            }
        }
        // ── thứ trong tuần
        if w == "thu" {
            if let Some(wd) = weekday_word(&f(&t, i + 1)) {
                weekday = Some(wd);
                t[i].used = true;
                t[i + 1].used = true;
                i += 2;
                continue;
            }
        }
        if let Some(rest) = w.strip_prefix('t') {
            if rest.len() == 1 {
                if let Some(wd) = weekday_word(rest) {
                    weekday = Some(wd);
                    t[i].used = true;
                    i += 1;
                    continue;
                }
            }
        }
        if w == "chu" && f(&t, i + 1) == "nhat" {
            weekday = Some(Weekday::Sun);
            t[i].used = true;
            t[i + 1].used = true;
            i += 2;
            continue;
        }
        if w == "cn" {
            weekday = Some(Weekday::Sun);
            t[i].used = true;
            i += 1;
            continue;
        }
        if w == "tuan" && matches!(f(&t, i + 1).as_str(), "sau" | "toi") {
            next_week = true;
            t[i].used = true;
            t[i + 1].used = true;
            i += 2;
            continue;
        }
        // ── buổi trong ngày
        let p = match w.as_str() {
            "sang" => Some(Period::Morning),
            "trua" => Some(Period::Noon),
            "chieu" => Some(Period::Afternoon),
            "dem" => Some(Period::Night),
            "toi" => {
                // "tối" (buổi tối) vs "tôi" (bản thân) — dựa vào dấu, hoặc ngữ cảnh khi gõ không dấu
                let o = t[i].orig.to_lowercase();
                let ctx = |j: usize| {
                    let x = f(&t, j);
                    matches!(x.as_str(), "nay" | "mai" | "qua") || re_time.is_match(&x)
                };
                if o.starts_with("tối") || (is_ascii_word(&o) && (ctx(i + 1) || (i > 0 && ctx(i - 1)))) {
                    Some(Period::Evening)
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some(p) = p {
            period = Some(p);
            t[i].used = true;
            // "sáng nay", "tối mai"
            match f(&t, i + 1).as_str() {
                "nay" => {
                    day_offset = Some(0);
                    t[i + 1].used = true;
                    i += 1;
                }
                "mai" => {
                    day_offset = Some(1);
                    t[i + 1].used = true;
                    i += 1;
                }
                _ => {}
            }
            i += 1;
            continue;
        }
        // ── hôm nay / ngày mai / ngày kia / mốt
        if w == "hom" && f(&t, i + 1) == "nay" {
            day_offset = Some(0);
            t[i].used = true;
            t[i + 1].used = true;
            i += 2;
            continue;
        }
        if w == "ngay" && matches!(f(&t, i + 1).as_str(), "mai" | "kia" | "mot") {
            day_offset = Some(if f(&t, i + 1) == "mai" { 1 } else { 2 });
            t[i].used = true;
            t[i + 1].used = true;
            i += 2;
            continue;
        }
        if w == "mai" {
            // "Mai" có thể là tên người: "gọi chị Mai"
            let prev = if i > 0 { f(&t, i - 1) } else { String::new() };
            let capitalized = t[i].orig.starts_with('M');
            let is_name = HONORIFICS.contains(&prev.as_str()) || (capitalized && i > 0);
            if !is_name {
                day_offset = Some(1);
                t[i].used = true;
                i += 1;
                continue;
            }
        }
        if w == "mot" && t[i].orig.to_lowercase().starts_with("mốt") {
            day_offset = Some(2);
            t[i].used = true;
            i += 1;
            continue;
        }
        i += 1;
    }

    // Từ đệm đứng trước mốc thời gian: "lúc", "vào", "vào lúc"
    for j in (0..n).rev() {
        if !t[j].used && matches!(t[j].f.as_str(), "luc" | "vao" | "at" | "on" | "by" | "for") && t.get(j + 1).is_some_and(|x| x.used) {
            t[j].used = true;
        }
    }
    // Từ mở đầu: "nhắc tôi", "nhắc", "nhớ"
    {
        let mut j = 0;
        while j < n && t[j].used {
            j += 1;
        }
        // tiếng Anh: "remind me to", "remember to", "don't forget to"
        let lead: &[&[&str]] = &[&["remind", "me", "to"], &["remind", "me"], &["remember", "to"], &["don't", "forget", "to"], &["dont", "forget", "to"], &["please"]];
        for pat in lead {
            if j + pat.len() <= n && pat.iter().enumerate().all(|(k, w)| t[j + k].f == *w && !t[j + k].used) {
                for k in 0..pat.len() {
                    t[j + k].used = true;
                }
                break;
            }
        }
        if j < n && matches!(t[j].f.as_str(), "nhac" | "nho") {
            t[j].used = true;
            if j + 1 < n && matches!(t[j + 1].f.as_str(), "toi" | "minh" | "em" | "anh") && !t[j + 1].used {
                t[j + 1].used = true;
            }
            if j + 2 < n && t[j + 2].f == "la" {
                t[j + 2].used = true;
            }
        }
    }

    let title = {
        let s: Vec<&str> = t.iter().filter(|x| !x.used).map(|x| x.orig.as_str()).collect();
        let s = s.join(" ");
        let s = s.trim_matches(|c: char| c == ',' || c == '-' || c.is_whitespace()).to_string();
        let mut c = s.chars();
        match c.next() {
            Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
            None => input.trim().to_string(),
        }
    };

    // ── Tính thời điểm
    if let Some(r) = rel {
        return Parsed { title, due: Some(now + r) };
    }
    let has_date = date.is_some() || dom.is_some() || weekday.is_some() || day_offset.is_some() || next_week;
    if !has_date && hour.is_none() && period.is_none() {
        return Parsed { title, due: None };
    }

    let today = now.date();
    let mut d = if let Some(d) = date {
        d
    } else if let Some(dm) = dom {
        let this = NaiveDate::from_ymd_opt(today.year(), today.month(), dm);
        match this {
            Some(x) if x >= today => x,
            _ => {
                let (y, m) = if today.month() == 12 { (today.year() + 1, 1) } else { (today.year(), today.month() + 1) };
                NaiveDate::from_ymd_opt(y, m, dm).unwrap_or(today)
            }
        }
    } else if let Some(wd) = weekday {
        let cur = today.weekday().num_days_from_monday() as i64;
        let target = wd.num_days_from_monday() as i64;
        let mut delta = (target - cur + 7) % 7;
        if next_week {
            // "thứ 6 tuần sau" = thứ 6 của tuần kế tiếp
            delta = 7 - cur + target;
        }
        today + Duration::days(delta)
    } else if let Some(o) = day_offset {
        today + Duration::days(o)
    } else if next_week {
        today + Duration::days(7)
    } else {
        today
    };

    let (mut h, m) = match hour {
        Some(h) => (h, minute),
        None => match period {
            Some(Period::Morning) => (8, 0),
            Some(Period::Noon) => (12, 0),
            Some(Period::Afternoon) => (14, 0),
            Some(Period::Evening) => (19, 0),
            Some(Period::Night) => (22, 0),
            None => (9, 0),
        },
    };
    if hour.is_some() {
        match period {
            Some(Period::Afternoon) | Some(Period::Evening) if h < 12 => h += 12,
            Some(Period::Night) if (6..12).contains(&h) => h += 12,
            Some(Period::Noon) if h < 3 => h += 12,
            _ => {}
        }
    }
    let mut due = NaiveDateTime::new(d, NaiveTime::from_hms_opt(h, m, 0).unwrap());

    // Chỉ có giờ, không có ngày
    if !has_date && due <= now {
        // "3h" lúc 14:00 → hiểu là 15:00 (chiều nay) thay vì 3h sáng mai
        if period.is_none() && hour.is_some_and(|x| x < 12) {
            let pm = due + Duration::hours(12);
            if pm > now {
                due = pm;
            } else {
                due += Duration::days(1);
            }
        } else {
            due += Duration::days(1);
        }
    }
    // Có thứ mà là hôm nay và giờ đã qua → tuần sau
    if weekday.is_some() && !next_week && due <= now {
        d += Duration::days(7);
        due = NaiveDateTime::new(d, due.time());
    }
    Parsed { title, due: Some(due) }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Thứ Tư 04/11/2026 14:00
    fn now() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 11, 4).unwrap().and_hms_opt(14, 0, 0).unwrap()
    }
    fn at(d: u32, h: u32, m: u32) -> Option<NaiveDateTime> {
        NaiveDate::from_ymd_opt(2026, 11, d).unwrap().and_hms_opt(h, m, 0)
    }
    fn p(s: &str) -> Parsed {
        parse(s, now())
    }

    #[test]
    fn basics() {
        assert_eq!(p("mai 9h gọi khách"), Parsed { title: "Gọi khách".into(), due: at(5, 9, 0) });
        assert_eq!(p("mai 9h goi khach").due, at(5, 9, 0));
        assert_eq!(p("15h30 họp team"), Parsed { title: "Họp team".into(), due: at(4, 15, 30) });
        assert_eq!(p("họp team lúc 16:45").title, "Họp team");
        assert_eq!(p("họp team lúc 16:45").due, at(4, 16, 45));
        assert_eq!(p("mua sữa").due, None);
        assert_eq!(p("mua sữa").title, "Mua sữa");
    }

    #[test]
    fn relative() {
        assert_eq!(p("30 phút nữa uống nước").due, at(4, 14, 30));
        assert_eq!(p("30 phút nữa uống nước").title, "Uống nước");
        assert_eq!(p("sau 2 tiếng gọi lại").due, at(4, 16, 0));
        assert_eq!(p("2 tieng nua goi lai").due, at(4, 16, 0));
    }

    #[test]
    fn periods() {
        assert_eq!(p("tối nay 8h xem phim").due, at(4, 20, 0));
        assert_eq!(p("3h chiều mai đi ngân hàng").due, at(5, 15, 0));
        assert_eq!(p("sáng mai chạy bộ").due, at(5, 8, 0));
        assert_eq!(p("toi nay goi me").due, at(4, 19, 0));
        // "tôi" (có dấu) không phải buổi tối
        assert_eq!(p("nhắc tôi gửi hợp đồng").title, "Gửi hợp đồng");
        assert_eq!(p("nhắc tôi gửi hợp đồng").due, None);
        // 3h khi đang 14:00 → 15:00 hôm nay
        assert_eq!(p("gọi khách 3h").due, at(4, 15, 0));
        // 10h khi đang 14:00 → 22:00 hôm nay
        assert_eq!(p("đọc sách 10h").due, at(4, 22, 0));
        assert_eq!(p("9 giờ rưỡi sáng mai khám răng").due, at(5, 9, 30));
    }

    #[test]
    fn weekdays_and_dates() {
        assert_eq!(p("thứ 6 nộp báo cáo"), Parsed { title: "Nộp báo cáo".into(), due: at(6, 9, 0) });
        assert_eq!(p("thu sau nop bao cao").due, at(6, 9, 0));
        assert_eq!(p("t2 họp giao ban").due, at(9, 9, 0));
        assert_eq!(p("chủ nhật đi Trốn").due, at(8, 9, 0));
        // thứ 4 = hôm nay, 9h đã qua → thứ 4 tuần sau
        assert_eq!(p("thứ 4 9h họp").due, at(11, 9, 0));
        assert_eq!(p("thứ 6 tuần sau chốt hợp đồng").due, at(13, 9, 0));
        assert_eq!(p("20/11 mua hoa").due, at(20, 9, 0));
        assert_eq!(p("20/11 mua hoa").title, "Mua hoa");
        assert_eq!(p("ngày 15 đóng tiền nhà").due, at(15, 9, 0));
        assert_eq!(p("ngày 2 đóng tiền nhà").due, NaiveDate::from_ymd_opt(2026, 12, 2).unwrap().and_hms_opt(9, 0, 0));
        assert_eq!(p("1/1 chúc tết").due, NaiveDate::from_ymd_opt(2027, 1, 1).unwrap().and_hms_opt(9, 0, 0));
    }

    #[test]
    fn english() {
        assert_eq!(p("tomorrow 9am call client"), Parsed { title: "Call client".into(), due: at(5, 9, 0) });
        assert_eq!(p("call John at 3pm"), Parsed { title: "Call John".into(), due: at(4, 15, 0) });
        assert_eq!(p("in 30 minutes drink water"), Parsed { title: "Drink water".into(), due: at(4, 14, 30) });
        assert_eq!(p("in an hour stretch").due, at(4, 15, 0));
        assert_eq!(p("friday submit report").due, at(6, 9, 0));
        assert_eq!(p("next friday submit report"), Parsed { title: "Submit report".into(), due: at(13, 9, 0) });
        assert_eq!(p("tonight 8pm movie").due, at(4, 20, 0));
        assert_eq!(p("remind me to pay rent on nov 20"), Parsed { title: "Pay rent".into(), due: at(20, 9, 0) });
        assert_eq!(p("pay rent 20th november").due, at(20, 9, 0));
        assert_eq!(p("meeting monday 10:30").due, at(9, 10, 30));
        assert_eq!(p("lunch with team at noon tomorrow"), Parsed { title: "Lunch with team".into(), due: at(5, 12, 0) });
        assert_eq!(p("tomorrow morning gym").due, at(5, 8, 0));
        assert_eq!(p("dinner 7:30 pm").due, at(4, 19, 30));
        assert_eq!(p("12am backup").due, at(5, 0, 0));
        assert_eq!(p("buy milk").due, None);
        assert_eq!(p("read the news in the evening").due, at(4, 19, 0));
        assert_eq!(p("read the news in the evening").title, "Read the news");
    }

    #[test]
    fn decomposed_unicode_from_ime() {
        use unicode_normalization::UnicodeNormalization;
        let nfd: String = "thứ 6 nộp báo cáo lúc 3h chiều".nfd().collect();
        assert_ne!(nfd, "thứ 6 nộp báo cáo lúc 3h chiều");
        let r = p(&nfd);
        assert_eq!(r.title, "Nộp báo cáo");
        assert_eq!(r.due, at(6, 15, 0));
        let r2 = p(&"tối nay 8h xem phim".nfd().collect::<String>());
        assert_eq!(r2.due, at(4, 20, 0));
        assert_eq!(fold(&"Hợp Đồng".nfd().collect::<String>()), "hop dong");
    }

    #[test]
    fn names_are_not_dates() {
        let r = p("gọi chị Mai lúc 3h chiều");
        assert_eq!(r.title, "Gọi chị Mai");
        assert_eq!(r.due, at(4, 15, 0));
        assert_eq!(p("hẹn Mai đi cafe").title, "Hẹn Mai đi cafe");
        assert_eq!(p("hẹn Mai đi cafe").due, None);
        assert_eq!(p("sửa bài này").due, None);
        assert_eq!(p("sửa bài này").title, "Sửa bài này");
    }
}
