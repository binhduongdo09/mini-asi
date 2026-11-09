//! Thông tin "đang diễn ra" (thời tiết, tin tức, tỷ giá, giá vàng, chứng khoán…).
//!
//! Mô hình trên máy (Apple Intelligence) không có Internet nên hỏi những câu này nó sẽ BỊA số liệu
//! (vd. "giá vàng 1.650.000 VND/ounce") hoặc chép lại câu trả lời cũ. Vì vậy Asi nhận ra các câu hỏi này
//! TRƯỚC khi hỏi mô hình và:
//! - thời tiết → Open-Meteo (miễn phí, không cần key), vị trí theo Cài đặt hoặc theo IP;
//! - tin tức → RSS VnExpress (mới nhất, công nghệ, kinh doanh, thể thao, thế giới, giải trí);
//! - tỷ giá → bảng tỷ giá Vietcombank;
//! - giá vàng, chứng khoán, giá xăng, tiền số… (chưa có nguồn ổn định) → nói thật là chưa có dữ liệu trực tiếp
//!   và đưa nút mở trang tìm kiếm, KHÔNG đoán số.
//! Phần nhận dạng câu hỏi & đọc dữ liệu là hàm thuần (có test); phần gọi mạng nằm ở `fetch_*`.

use crate::nlp::fold;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum Topic {
    /// `place` = địa danh nêu trong câu (nếu có); `day` = 0 hôm nay, 1 ngày mai
    Weather { place: Option<String>, day: u8 },
    /// mục RSS VnExpress
    News { cat: &'static str, label: &'static str },
    Fx,
    Gold,
    Stock,
    Fuel,
    Crypto,
    /// "thị trường hôm nay thế nào" — chưa rõ thị trường nào
    Market,
}

fn padded(s: &str) -> String {
    format!(" {} ", fold(s).replace([',', '.', '!', '?', ':', ';', '"', '(', ')'], " ").split_whitespace().collect::<Vec<_>>().join(" "))
}

fn has(f: &str, keys: &[&str]) -> bool {
    keys.iter().any(|k| f.contains(&format!(" {k} ")))
}

/// Câu hỏi có cần dữ liệu trực tiếp không. Lệnh nhắc việc / ghi chú do bên gọi loại trước.
pub fn topic(text: &str) -> Option<Topic> {
    let f = padded(text);
    let lower = text.to_lowercase();
    if has(&f, &["gia vang", "vang sjc", "vang nhan", "vang mieng", "gold price", "gia vang the gioi"]) {
        return Some(Topic::Gold);
    }
    if has(&f, &["ty gia", "gia usd", "gia do", "do la", "gia euro", "gia yen", "gia te", "exchange rate", "usd hom nay", "1 usd", "mot do"]) {
        return Some(Topic::Fx);
    }
    // "mưa" (có dấu) mới là thời tiết — "mua" không dấu thường là "mua bán"
    let rain = lower.contains("mưa") || has(&f, &["troi mua", "co mua khong", "mua khong", "mang o", "mang ao mua"]);
    if rain || has(&f, &["thoi tiet", "nhiet do", "bao nhieu do", "do am", "nong khong", "lanh khong", "nang khong", "ret khong", "weather", "temperature", "forecast", "du bao"]) {
        if !has(&f, &["du bao gia", "du bao thi truong"]) {
            let day = if has(&f, &["mai", "ngay mai", "tomorrow"]) { 1 } else { 0 };
            return Some(Topic::Weather { place: weather_place(text), day });
        }
    }
    if has(&f, &["chung khoan", "vn index", "vnindex", "vn-index", "co phieu", "san hose", "hnx", "stock market", "stocks"]) {
        return Some(Topic::Stock);
    }
    if has(&f, &["gia xang", "xang dau", "xang ron", "gia dau", "fuel price"]) {
        return Some(Topic::Fuel);
    }
    if has(&f, &["bitcoin", "btc", "crypto", "tien so", "tien ao", "ethereum", "eth"]) {
        return Some(Topic::Crypto);
    }
    let news = has(&f, &["tin tuc", "tin moi", "tin nong", "thoi su", "ban tin", "doc bao", "bao hom nay", "co tin gi", "tin gi", "tin cong nghe", "tin kinh doanh", "tin the thao", "tin the gioi", "tin giai tri", "tin bong da", "news", "headlines"]);
    if news {
        let (cat, label) = if has(&f, &["cong nghe", "so hoa", "tech", "technology", "ai", "dien thoai", "apple"]) {
            ("so-hoa", "công nghệ")
        } else if has(&f, &["kinh doanh", "kinh te", "tai chinh", "business"]) {
            ("kinh-doanh", "kinh doanh")
        } else if has(&f, &["the thao", "bong da", "sport", "sports"]) {
            ("the-thao", "thể thao")
        } else if has(&f, &["the gioi", "quoc te", "world"]) {
            ("the-gioi", "thế giới")
        } else if has(&f, &["giai tri", "showbiz", "entertainment"]) {
            ("giai-tri", "giải trí")
        } else {
            ("tin-moi-nhat", "mới nhất")
        };
        return Some(Topic::News { cat, label });
    }
    if has(&f, &["thi truong", "the market", "markets"]) && !has(&f, &["marketing", "nghien cuu thi truong", "phan tich thi truong", "thi truong muc tieu"]) {
        return Some(Topic::Market);
    }
    None
}

/// "thời tiết Đà Nẵng hôm nay" → "Đà Nẵng"; "nhiệt độ ở Sa Pa ngày mai" → "Sa Pa"; không nêu → None
pub fn weather_place(text: &str) -> Option<String> {
    const KEYS: &[&str] = &["thời tiết", "nhiệt độ", "dự báo", "thoi tiet", "nhiet do", "du bao", "weather in", "weather", "ở", "tại", "o", "tai"];
    const STOP: &[&str] = &[
        "hôm", "nay", "ngày", "mai", "thế", "nào", "ra", "sao", "bao", "nhiêu", "độ", "không", "ko", "k", "nhỉ", "vậy", "thì", "hiện", "tại", "bây", "giờ",
        "có", "mưa", "nắng", "lạnh", "nóng", "trời", "sáng", "chiều", "tối", "đêm", "tuần", "này", "today", "tomorrow", "now", "like", "is", "the", "what", "s", "in",
        "hom", "the", "nao", "bao", "nhieu", "do", "khong", "nhi", "vay", "thi", "hien", "bay", "gio", "co", "nang", "lanh", "nong", "troi", "sang", "chieu", "toi",
        "dem", "tuan", "nay", "ở", "o", "tại", "tai", "của", "cua", "khu", "vực", "vuc", "lúc", "luc", "sẽ", "se", "đang", "dang", "thời", "tiết", "nhiệt", "dự", "báo",
    ];
    let lower = text.to_lowercase();
    let clean = lower.replace(['?', '!', ',', '.'], " ");
    let words: Vec<&str> = clean.split_whitespace().collect();
    // tìm vị trí ngay sau từ khoá thời tiết / "ở" / "tại"
    let mut start = None;
    for i in 0..words.len() {
        for k in KEYS {
            let kw: Vec<&str> = k.split(' ').collect();
            if i + kw.len() <= words.len() && words[i..i + kw.len()] == kw[..] {
                start = Some(i + kw.len());
            }
        }
    }
    let start = start?;
    let orig: Vec<&str> = text.split_whitespace().map(|w| w.trim_matches(|c: char| ",.?!".contains(c))).collect();
    let mut out = vec![];
    for (j, w) in words.iter().enumerate().skip(start) {
        if STOP.contains(w) || w.chars().any(|c| c.is_ascii_digit()) {
            if out.is_empty() {
                continue;
            }
            break;
        }
        out.push(orig.get(j).copied().unwrap_or(w));
        if out.len() >= 4 {
            break;
        }
    }
    let p = out.join(" ");
    (p.chars().filter(|c| c.is_alphabetic()).count() >= 2).then_some(p)
}

// ───────────────────────── thời tiết ─────────────────────────

/// Mã thời tiết WMO → mô tả tiếng Việt
pub fn wmo_vi(code: i64) -> &'static str {
    match code {
        0 => "trời quang",
        1 => "ít mây",
        2 => "có mây",
        3 => "nhiều mây",
        45 | 48 => "sương mù",
        51 | 53 | 55 | 56 | 57 => "mưa phùn",
        61 | 80 => "mưa nhỏ",
        63 | 81 => "mưa vừa",
        65 | 82 => "mưa to",
        66 | 67 => "mưa lạnh",
        71 | 73 | 75 | 77 | 85 | 86 => "tuyết",
        95 => "dông",
        96 | 99 => "dông kèm mưa đá",
        _ => "không rõ",
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Weather {
    pub place: String,
    pub time: String,
    pub temp: f64,
    pub feels: f64,
    pub humidity: f64,
    pub wind: f64,
    pub desc: String,
    /// (mô tả, thấp nhất, cao nhất, % khả năng mưa, UV) cho hôm nay [0], ngày mai [1]
    pub days: Vec<(String, f64, f64, f64, f64)>,
}

pub fn weather_url(lat: f64, lon: f64) -> String {
    format!(
        "https://api.open-meteo.com/v1/forecast?latitude={lat:.4}&longitude={lon:.4}&current=temperature_2m,apparent_temperature,relative_humidity_2m,weather_code,wind_speed_10m&daily=weather_code,temperature_2m_max,temperature_2m_min,precipitation_probability_max,uv_index_max&timezone=auto&forecast_days=3"
    )
}

pub fn parse_weather(v: &Value, place: &str) -> Option<Weather> {
    let c = v.get("current")?;
    let num = |x: &Value, k: &str| x.get(k).and_then(Value::as_f64);
    let d = v.get("daily");
    let arr = |k: &str, i: usize| d.and_then(|d| d.get(k)).and_then(|a| a.get(i)).and_then(Value::as_f64).unwrap_or(f64::NAN);
    let mut days = vec![];
    for i in 0..2 {
        if d.and_then(|d| d.get("time")).and_then(|t| t.get(i)).is_some() {
            days.push((wmo_vi(arr("weather_code", i) as i64).to_string(), arr("temperature_2m_min", i), arr("temperature_2m_max", i), arr("precipitation_probability_max", i), arr("uv_index_max", i)));
        }
    }
    Some(Weather {
        place: place.to_string(),
        time: c.get("time").and_then(Value::as_str).map(|t| t.split('T').nth(1).unwrap_or(t).to_string()).unwrap_or_default(),
        temp: num(c, "temperature_2m")?,
        feels: num(c, "apparent_temperature").unwrap_or(f64::NAN),
        humidity: num(c, "relative_humidity_2m").unwrap_or(f64::NAN),
        wind: num(c, "wind_speed_10m").unwrap_or(f64::NAN),
        desc: wmo_vi(num(c, "weather_code").unwrap_or(-1.0) as i64).to_string(),
        days,
    })
}

/// Câu trả lời ngắn gọn (đọc to được)
pub fn weather_text(w: &Weather, day: u8) -> String {
    let r = |x: f64| x.round() as i64;
    if day == 1 {
        if let Some((desc, lo, hi, rain, _)) = w.days.get(1) {
            let mut s = format!("Ngày mai ở {}: {desc}, {}–{}°C", w.place, r(*lo), r(*hi));
            if rain.is_finite() {
                s.push_str(&format!(", khả năng mưa {}%", r(*rain)));
            }
            s.push('.');
            if *rain >= 50.0 {
                s.push_str(" Nhớ mang ô nhé.");
            }
            return s;
        }
    }
    let mut s = format!("{} lúc {}: {}°C, {}", w.place, w.time, r(w.temp), w.desc);
    if w.feels.is_finite() && (w.feels - w.temp).abs() >= 2.0 {
        s.push_str(&format!(" (cảm giác như {}°C)", r(w.feels)));
    }
    s.push('.');
    if let Some((_, lo, hi, rain, _)) = w.days.first() {
        s.push_str(&format!(" Hôm nay {}–{}°C", r(*lo), r(*hi)));
        if rain.is_finite() {
            s.push_str(&format!(", khả năng mưa {}%", r(*rain)));
        }
        s.push('.');
        if *rain >= 50.0 {
            s.push_str(" Nhớ mang ô nhé.");
        }
    }
    s
}

pub fn geocode_url(name: &str) -> String {
    format!("https://geocoding-api.open-meteo.com/v1/search?name={}&count=5&language=vi&format=json", enc(name))
}

/// Kết quả tìm địa danh → (tên, vĩ độ, kinh độ), ưu tiên Việt Nam
pub fn parse_geocode(v: &Value) -> Option<(String, f64, f64)> {
    let rs = v.get("results")?.as_array()?;
    let pick = rs.iter().find(|r| r.get("country_code").and_then(Value::as_str) == Some("VN")).or_else(|| rs.first())?;
    Some((pick.get("name")?.as_str()?.to_string(), pick.get("latitude")?.as_f64()?, pick.get("longitude")?.as_f64()?))
}

pub fn enc(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => o.push(b as char),
            b' ' => o.push_str("%20"),
            _ => o.push_str(&format!("%{b:02X}")),
        }
    }
    o
}

/// Trang tìm kiếm cho chủ đề chưa có nguồn dữ liệu
pub fn search_url(q: &str) -> String {
    format!("https://www.google.com/search?q={}", enc(q))
}

// ───────────────────────── tin tức (RSS) ─────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct Headline {
    pub title: String,
    pub url: String,
    /// unix ms (0 = không rõ)
    pub at: i64,
}

pub fn rss_url(cat: &str) -> String {
    format!("https://vnexpress.net/rss/{cat}.rss")
}

fn tag<'a>(s: &'a str, t: &str) -> Option<&'a str> {
    let a = s.find(&format!("<{t}"))?;
    let a = a + s[a..].find('>')? + 1;
    let b = a + s[a..].find(&format!("</{t}>"))?;
    Some(&s[a..b])
}

fn unxml(s: &str) -> String {
    let s = s.trim();
    let s = s.strip_prefix("<![CDATA[").and_then(|x| x.strip_suffix("]]>")).unwrap_or(s);
    s.replace("&quot;", "\"").replace("&#39;", "'").replace("&apos;", "'").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&").trim().to_string()
}

pub fn parse_rss(xml: &str, max: usize) -> Vec<Headline> {
    let mut out = vec![];
    let mut rest = xml;
    while let Some(i) = rest.find("<item") {
        let Some(j) = rest[i..].find("</item>") else { break };
        let item = &rest[i..i + j];
        rest = &rest[i + j + 7..];
        let title = tag(item, "title").map(unxml).unwrap_or_default();
        let url = tag(item, "link").map(unxml).unwrap_or_default();
        if title.is_empty() || !url.starts_with("https://") {
            continue;
        }
        let at = tag(item, "pubDate").map(unxml).and_then(|d| {
            chrono::DateTime::parse_from_rfc2822(&d)
                .or_else(|_| chrono::DateTime::parse_from_str(d.split_once(',').map(|x| x.1).unwrap_or(&d).trim(), "%d %b %Y %H:%M:%S %z"))
                .ok()
        });
        let at = at.map(|d| d.timestamp_millis()).unwrap_or(0);
        out.push(Headline { title, url, at });
        if out.len() >= max {
            break;
        }
    }
    out
}

// ───────────────────────── tỷ giá (Vietcombank) ─────────────────────────

pub const FX_URL: &str = "https://portal.vietcombank.com.vn/Usercontrols/TVPortal.TyGia/pXML.aspx";

#[derive(Debug, Clone, PartialEq)]
pub struct Rate {
    pub code: String,
    pub buy: String,
    pub sell: String,
}

fn attr(s: &str, name: &str) -> String {
    s.find(&format!("{name}=\""))
        .and_then(|i| {
            let a = i + name.len() + 2;
            s[a..].find('"').map(|j| s[a..a + j].trim().to_string())
        })
        .unwrap_or_default()
}

/// (thời điểm cập nhật, các tỷ giá theo thứ tự hay dùng)
pub fn parse_fx(xml: &str) -> (String, Vec<Rate>) {
    const WANT: &[&str] = &["USD", "EUR", "JPY", "GBP", "AUD", "SGD", "CNY", "KRW", "THB"];
    let when = tag(xml, "DateTime").map(unxml).unwrap_or_default();
    let mut all = vec![];
    for part in xml.split("<Exrate").skip(1) {
        let code = attr(part, "CurrencyCode");
        if WANT.contains(&code.as_str()) {
            all.push(Rate { code, buy: attr(part, "Buy"), sell: attr(part, "Sell") });
        }
    }
    all.sort_by_key(|r| WANT.iter().position(|w| *w == r.code).unwrap_or(99));
    (when, all)
}

/// Câu hỏi nhắc tới đồng tiền nào (để đưa lên đầu)
pub fn fx_focus(text: &str) -> Option<&'static str> {
    let f = padded(text);
    [("USD", &["usd", "do", "do la", "dola"][..]), ("EUR", &["euro", "eur"]), ("JPY", &["yen", "jpy"]), ("CNY", &["te", "nhan dan te", "cny"]), ("KRW", &["won", "krw"]), ("GBP", &["bang anh", "gbp"]), ("THB", &["baht", "thb"])]
        .iter()
        .find(|(_, ks)| has(&f, ks))
        .map(|(c, _)| *c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topics_from_screenshot() {
        assert_eq!(topic("nhiệt độ hôm nay thế nào"), Some(Topic::Weather { place: None, day: 0 }));
        assert_eq!(topic("nhiệt độ"), Some(Topic::Weather { place: None, day: 0 }));
        assert_eq!(topic("thị trường hôm nay thế nào"), Some(Topic::Market));
        assert_eq!(topic("giá vàng hôm nay thế nào"), Some(Topic::Gold));
        assert_eq!(topic("Tin công nghệ hôm nay có gì ko"), Some(Topic::News { cat: "so-hoa", label: "công nghệ" }));
        assert_eq!(topic("Công việc hôm nay thế nào"), None, "câu về việc của mình → để trợ lý trả lời");
    }

    #[test]
    fn more_topics() {
        assert_eq!(topic("mai có mưa không"), Some(Topic::Weather { place: None, day: 1 }));
        assert_eq!(topic("thời tiết Đà Nẵng ngày mai"), Some(Topic::Weather { place: Some("Đà Nẵng".into()), day: 1 }));
        assert_eq!(topic("tỷ giá đô hôm nay"), Some(Topic::Fx));
        assert_eq!(topic("VN-Index hôm nay"), Some(Topic::Stock));
        assert_eq!(topic("giá xăng tuần này"), Some(Topic::Fuel));
        assert_eq!(topic("bitcoin bao nhiêu rồi"), Some(Topic::Crypto));
        assert_eq!(topic("có tin gì mới không"), Some(Topic::News { cat: "tin-moi-nhat", label: "mới nhất" }));
        assert_eq!(topic("tin bóng đá hôm nay"), Some(Topic::News { cat: "the-thao", label: "thể thao" }));
        assert_eq!(topic("tỷ giá đô bao nhiêu đô"), Some(Topic::Fx));
        assert_eq!(topic("làm sao để có năng lượng cả ngày"), None);
        assert_eq!(topic("tin thể thao"), Some(Topic::News { cat: "the-thao", label: "thể thao" }));
        // không nhầm
        assert_eq!(topic("mua vàng ở đâu rẻ"), None);
        assert_eq!(topic("viết kế hoạch marketing cho thị trường mục tiêu"), None);
        assert_eq!(topic("dịch giúp mình câu này sang tiếng Anh"), None);
        assert_eq!(topic("mua sữa cho con"), None);
    }

    #[test]
    fn place_extraction() {
        assert_eq!(weather_place("thời tiết Đà Nẵng hôm nay thế nào"), Some("Đà Nẵng".into()));
        assert_eq!(weather_place("nhiệt độ ở Sa Pa ngày mai"), Some("Sa Pa".into()));
        assert_eq!(weather_place("thời tiết tại TP Hồ Chí Minh"), Some("TP Hồ Chí Minh".into()));
        assert_eq!(weather_place("nhiệt độ hôm nay thế nào"), None);
        assert_eq!(weather_place("mai có mưa không"), None);
        assert_eq!(weather_place("weather in Tokyo"), Some("Tokyo".into()));
    }

    #[test]
    fn weather_parse_and_text() {
        let v: Value = serde_json::from_str(r#"{"current":{"time":"2026-11-09T14:00","temperature_2m":27.4,"apparent_temperature":30.1,"relative_humidity_2m":70,"weather_code":3,"wind_speed_10m":9.5},
          "daily":{"time":["2026-11-09","2026-11-10","2026-11-11"],"weather_code":[3,63,1],"temperature_2m_max":[29.2,26,27],"temperature_2m_min":[22.1,21,20],"precipitation_probability_max":[20,80,5],"uv_index_max":[6,3,7]}}"#).unwrap();
        let w = parse_weather(&v, "Hà Nội").unwrap();
        assert_eq!(w.desc, "nhiều mây");
        assert_eq!(weather_text(&w, 0), "Hà Nội lúc 14:00: 27°C, nhiều mây (cảm giác như 30°C). Hôm nay 22–29°C, khả năng mưa 20%.");
        assert_eq!(weather_text(&w, 1), "Ngày mai ở Hà Nội: mưa vừa, 21–26°C, khả năng mưa 80%. Nhớ mang ô nhé.");
        assert!(parse_weather(&serde_json::json!({"error": true}), "x").is_none());
        let g: Value = serde_json::from_str(r#"{"results":[{"name":"Da Nang","latitude":1.0,"longitude":2.0,"country_code":"US"},{"name":"Đà Nẵng","latitude":16.07,"longitude":108.22,"country_code":"VN"}]}"#).unwrap();
        assert_eq!(parse_geocode(&g), Some(("Đà Nẵng".into(), 16.07, 108.22)));
        assert_eq!(parse_geocode(&serde_json::json!({"generationtime_ms": 1})), None);
        assert_eq!(enc("Đà Nẵng"), "%C4%90%C3%A0%20N%E1%BA%B5ng");
    }

    #[test]
    fn rss_and_fx() {
        let xml = r#"<rss><channel><title>VnExpress</title>
          <item><title><![CDATA[Apple ra mắt chip M6 &amp; MacBook mới]]></title><description><![CDATA[<a href="x"><img src="y"></a>mô tả]]></description>
          <pubDate>Sun, 09 Nov 2026 14:05:00 +0700</pubDate><link>https://vnexpress.net/apple-ra-mat-123.html</link></item>
          <item><title>Tin thứ hai</title><link>https://vnexpress.net/b.html</link></item>
          <item><title>không có link</title></item></channel></rss>"#;
        let h = parse_rss(xml, 5);
        assert_eq!(h.len(), 2);
        assert_eq!(h[0].title, "Apple ra mắt chip M6 & MacBook mới");
        assert_eq!(h[0].url, "https://vnexpress.net/apple-ra-mat-123.html");
        assert!(h[0].at > 0);
        assert_eq!(h[1].at, 0);
        let fx = r#"<ExrateList><DateTime>11/9/2026 2:30:00 PM</DateTime><Exrate CurrencyCode="AUD" CurrencyName="AUSTRALIAN DOLLAR" Buy="16,885.29" Transfer="17,045.75" Sell="17,560.89" /><Exrate CurrencyCode="USD" CurrencyName="US DOLLAR" Buy="25,890.00" Transfer="25,920.00" Sell="26,260.00" /><Exrate CurrencyCode="XYZ" Buy="1" Sell="2"/></ExrateList>"#;
        let (when, rates) = parse_fx(fx);
        assert_eq!(when, "11/9/2026 2:30:00 PM");
        assert_eq!(rates.iter().map(|r| r.code.as_str()).collect::<Vec<_>>(), vec!["USD", "AUD"]);
        assert_eq!((rates[0].buy.as_str(), rates[0].sell.as_str()), ("25,890.00", "26,260.00"));
        assert_eq!(fx_focus("tỷ giá yên Nhật"), Some("JPY"));
        assert_eq!(fx_focus("tỷ giá đô hôm nay"), Some("USD"));
    }
}
