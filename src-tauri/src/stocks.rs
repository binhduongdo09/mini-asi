//! Giá cổ phiếu Việt Nam (HOSE / HNX / UPCOM) và chỉ số (VN-Index, VN30, HNX, UPCOM) + cảnh báo giá.
//!
//! Nguồn: biểu đồ của DNSE Entrade (`services.entrade.com.vn/chart-api/v2/ohlcs/...`) — miễn phí, không cần key,
//! nến 1 phút cập nhật trong phiên. Giá cổ phiếu trả về theo **nghìn đồng** (51 = 51.000đ); chỉ số theo điểm.
//! (Thư viện quantvn cần API key của QuantVN nên không dùng.)
//!
//! - Tra giá: "giá FPT", "FPT bao nhiêu", "VN-Index hôm nay"
//! - Cảnh báo: "báo cho tôi khi FPT vượt 100k hoặc giảm dưới 50k" → 2 cảnh báo, Asi kiểm tra mỗi phút trong giờ giao dịch
//!   và báo (thông báo + tai thỏ) khi chạm ngưỡng, mỗi cảnh báo báo 1 lần.
//! Phần thuần (đọc dữ liệu, hiểu câu lệnh, kiểm tra ngưỡng) có test; phần gọi mạng ở `commands/stocks.rs`.

use chrono::{Datelike, NaiveDateTime, Timelike, Weekday};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

use crate::nlp::fold;

pub const BASE: &str = "https://services.entrade.com.vn/chart-api/v2/ohlcs";

/// Chỉ số: tên người dùng hay gõ → mã của nguồn dữ liệu
const INDEXES: &[(&str, &str)] = &[("VNINDEX", "VNINDEX"), ("VN-INDEX", "VNINDEX"), ("VN INDEX", "VNINDEX"), ("VN30", "VN30"), ("HNX30", "HNX30"), ("HNXINDEX", "HNX"), ("HNX-INDEX", "HNX"), ("HNX", "HNX"), ("UPCOM", "UPCOM"), ("UPCOMINDEX", "UPCOM")];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Kind {
    Stock,
    Index,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Symbol {
    pub code: String,
    pub kind: Kind,
}

pub fn url(sym: &Symbol, resolution: &str, from: i64, to: i64) -> String {
    let k = if sym.kind == Kind::Index { "index" } else { "stock" };
    format!("{BASE}/{k}?from={from}&to={to}&symbol={}&resolution={resolution}", sym.code)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bar {
    /// unix giây
    pub t: i64,
    pub o: f64,
    pub h: f64,
    pub l: f64,
    pub c: f64,
    pub v: f64,
}

/// {"t":[..],"o":[..],"h":[..],"l":[..],"c":[..],"v":[..]} → các nến. Lỗi ({"status":400,"message":"invalid symbol"}) → Err.
pub fn parse_bars(v: &Value) -> Result<Vec<Bar>, String> {
    if let Some(m) = v.get("message").and_then(Value::as_str) {
        return Err(if m.contains("invalid symbol") { "Không có mã này".into() } else { m.to_string() });
    }
    let arr = |k: &str| v.get(k).and_then(Value::as_array).cloned().unwrap_or_default();
    let (t, o, h, l, c, vol) = (arr("t"), arr("o"), arr("h"), arr("l"), arr("c"), arr("v"));
    let n = t.len().min(c.len());
    let f = |a: &Vec<Value>, i: usize| a.get(i).and_then(Value::as_f64).unwrap_or(f64::NAN);
    Ok((0..n)
        .map(|i| Bar { t: t[i].as_i64().unwrap_or(0), o: f(&o, i), h: f(&h, i), l: f(&l, i), c: f(&c, i), v: f(&vol, i) })
        .filter(|b| b.c.is_finite())
        .collect())
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Quote {
    pub symbol: String,
    pub index: bool,
    /// đơn vị hiển thị: VND cho cổ phiếu, điểm cho chỉ số
    pub price: f64,
    pub reference: f64,
    pub change: f64,
    pub pct: f64,
    pub high: f64,
    pub low: f64,
    pub volume: f64,
    /// unix giây của nến mới nhất
    pub at: i64,
}

/// Giá hiện tại từ nến ngày: nến cuối = phiên gần nhất (đang chạy hoặc đã đóng), nến trước = giá tham chiếu
pub fn quote_from_daily(sym: &Symbol, bars: &[Bar], last_minute: Option<&Bar>) -> Option<Quote> {
    let today = bars.last()?;
    let mul = if sym.kind == Kind::Stock { 1000.0 } else { 1.0 };
    let price = last_minute.filter(|m| m.t >= today.t).map(|m| m.c).unwrap_or(today.c);
    let reference = bars.len().checked_sub(2).and_then(|i| bars.get(i)).map(|b| b.c).unwrap_or(today.o);
    let r = |x: f64| (x * mul * 100.0).round() / 100.0;
    Some(Quote {
        symbol: sym.code.clone(),
        index: sym.kind == Kind::Index,
        price: r(price),
        reference: r(reference),
        change: r(price - reference),
        pct: if reference > 0.0 { ((price - reference) / reference * 10000.0).round() / 100.0 } else { 0.0 },
        high: r(today.h.max(price)),
        low: r(today.l.min(price)),
        volume: today.v,
        at: last_minute.map(|m| m.t).unwrap_or(today.t),
    })
}

/// 51000 → "51.000đ"; 1714.32 → "1.714,32"
pub fn fmt_price(x: f64, index: bool) -> String {
    let neg = x < 0.0;
    let x = x.abs();
    let int = x.trunc() as i64;
    let mut s = String::new();
    for (i, ch) in int.to_string().chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            s.push('.');
        }
        s.push(ch);
    }
    let mut out: String = s.chars().rev().collect();
    let frac = ((x - x.trunc()) * 100.0).round() as i64;
    if index {
        out.push_str(&format!(",{frac:02}"));
    } else if frac > 0 {
        out.push_str(&format!(",{}", format!("{frac:02}").trim_end_matches('0')));
    }
    format!("{}{}{}", if neg { "-" } else { "" }, out, if index { "" } else { "đ" })
}

pub fn quote_text(q: &Quote) -> String {
    let arrow = if q.change > 0.0 { "▲" } else if q.change < 0.0 { "▼" } else { "■" };
    let sign = if q.change > 0.0 { "+" } else { "" };
    if q.index {
        format!("{} {} điểm {arrow} {sign}{} ({sign}{:.2}%)", q.symbol, fmt_price(q.price, true), fmt_price(q.change, true), q.pct)
    } else {
        format!("{} {} {arrow} {sign}{} ({sign}{:.2}%)", q.symbol, fmt_price(q.price, false), fmt_price(q.change, false), q.pct)
    }
}

// ───────────────────────── hiểu câu lệnh ─────────────────────────

const NOT_TICKERS: &[&str] = &["VND", "USD", "EUR", "JPY", "CNY", "SJC", "PNJ_", "ETF", "IPO", "CEO", "GDP", "API", "APP", "AI", "OK", "VN", "TP", "HCM", "MAI", "THE", "AND", "TIN", "GIA", "VAO", "BAO", "KHI", "CHO"];

/// Các mã trong câu: mã viết hoa 3 ký tự (FPT, HPG, VN30F…), chỉ số (VN-Index, VN30…), hoặc mã viết thường đứng sau
/// "cổ phiếu" / "mã" / "giá" ("giá fpt").
pub fn symbols_in(text: &str) -> Vec<Symbol> {
    let mut out: Vec<Symbol> = vec![];
    let push = |out: &mut Vec<Symbol>, s: Symbol| {
        if !out.iter().any(|x| x.code == s.code) {
            out.push(s);
        }
    };
    let up = text.to_uppercase();
    let up_f = fold(&up).to_uppercase();
    for (k, code) in INDEXES {
        if up_f.split(|c: char| !c.is_ascii_alphanumeric() && c != '-').any(|w| w == *k) || (k.contains(' ') && up_f.contains(k)) {
            push(&mut out, Symbol { code: code.to_string(), kind: Kind::Index });
        }
    }
    // mã viết hoa nguyên văn (người dùng gõ FPT, HPG…)
    for w in text.split(|c: char| !c.is_ascii_alphanumeric()) {
        if w.len() == 3 && w.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()) && w.chars().next().is_some_and(|c| c.is_ascii_uppercase()) && !NOT_TICKERS.contains(&w) {
            push(&mut out, Symbol { code: w.to_string(), kind: Kind::Stock });
        }
    }
    // mã viết thường sau "cổ phiếu" / "mã" / "giá" / "cp"
    let f = fold(text);
    let words: Vec<&str> = f.split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| !w.is_empty()).collect();
    for i in 0..words.len() {
        let after = match (words[i], words.get(i + 1)) {
            ("co", Some(&"phieu")) => words.get(i + 2),
            ("ma", _) | ("cp", _) | ("gia", _) => words.get(i + 1),
            _ => None,
        };
        if let Some(w) = after {
            let w = w.to_uppercase();
            if w.len() == 3 && w.chars().all(|c| c.is_ascii_alphanumeric()) && !NOT_TICKERS.contains(&w.as_str()) && !["CUA", "HOM", "NAY", "VANG", "XANG", "USD", "DO"].contains(&w.as_str()) {
                push(&mut out, Symbol { code: w, kind: Kind::Stock });
            }
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Op {
    Above,
    Below,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Alert {
    pub id: u64,
    pub symbol: String,
    pub index: bool,
    pub op: Op,
    /// VND (cổ phiếu) hoặc điểm (chỉ số)
    pub target: f64,
    pub created: i64,
    #[serde(default)]
    pub fired: Option<i64>,
    #[serde(default)]
    pub fired_price: Option<f64>,
}

impl Alert {
    pub fn describe(&self) -> String {
        let op = if self.op == Op::Above { "vượt" } else { "giảm dưới" };
        format!("{} {op} {}", self.symbol, if self.index { format!("{} điểm", fmt_price(self.target, true)) } else { fmt_price(self.target, false) })
    }
}

/// "100k", "100.000", "100,5k", "1tr2", "100" (cổ phiếu: < 1000 hiểu là nghìn đồng), "1.800 điểm"
pub fn parse_amount(s: &str, index: bool) -> Option<f64> {
    let t = fold(s).replace(' ', "");
    let num_end = t.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == ',')).unwrap_or(t.len());
    if num_end == 0 {
        return None;
    }
    let (num, unit) = t.split_at(num_end);
    // "100.000" / "100,000" = phân cách nghìn; "100,5" / "1.5" = thập phân
    let groups: Vec<&str> = num.split(['.', ',']).collect();
    let v: f64 = if groups.len() > 1 && groups[1..].iter().all(|g| g.len() == 3) {
        groups.concat().parse().ok()?
    } else {
        num.replace(',', ".").parse().ok()?
    };
    // chỉ nhận đơn vị tiền / điểm — "3h chiều", "2 người"… không phải giá
    const UNITS: &[&str] = &["", "k", "nghin", "ngan", "tr", "trieu", "m", "d", "đ", "dong", "vnd", "diem", "dong."];
    if !UNITS.iter().any(|u| unit == *u || (u.len() > 1 && unit.starts_with(u) && !unit[u.len()..].starts_with(|c: char| c.is_ascii_alphabetic()))) {
        return None;
    }
    let unit = unit.trim_start_matches(['d', 'đ']);
    let mul = if unit.starts_with('k') || unit.starts_with("nghin") || unit.starts_with("ngan") {
        1_000.0
    } else if unit.starts_with("tr") || unit.starts_with("trieu") || unit.starts_with('m') {
        1_000_000.0
    } else if index || v >= 1000.0 {
        1.0
    } else {
        1_000.0 // "FPT 100" = 100 nghìn đồng như cách nói trên sàn
    };
    let r = v * mul;
    (r > 0.0).then_some(r)
}

const ABOVE: &[&str] = &["vuot", "len tren", "tren", "cao hon", "lon hon", "len", "tang len", "tang qua", "vuot qua", "above", "over", ">=", ">", "cham", "cham moc", "cham muc"];
const BELOW: &[&str] = &["giam duoi", "xuong duoi", "duoi", "thap hon", "nho hon", "xuong", "giam xuong", "giam con", "rot xuong", "roi xuong", "below", "under", "<=", "<", "thung"];

/// Câu có phải lệnh đặt cảnh báo giá không: "báo cho tôi khi FPT vượt 100k hoặc giảm dưới 50k".
/// Trả về (mã, các (điều kiện, ngưỡng)). Điều kiện mơ hồ ("chạm 60k") để `Op::Above`, bên gọi so với giá hiện tại.
pub fn parse_alert_command(text: &str) -> Option<(Symbol, Vec<(Op, f64, bool)>)> {
    let f = format!(" {} ", fold(text).replace([',', ';'], " , "));
    let asks = ["bao cho", "bao toi", "bao minh", "bao t ", "bao em", "bao anh", "thong bao", "canh bao", "nhac toi", "nhac minh", "nhac t ", "nhac khi", "bao khi", "alert", "notify", "dat canh bao", "theo doi gia"];
    if !asks.iter().any(|k| f.contains(k)) {
        return None;
    }
    let sym = symbols_in(text).into_iter().next()?;
    let index = sym.kind == Kind::Index;
    // tách theo "hoặc" / "hay" / "," / "và" rồi tìm điều kiện + con số trong từng vế
    let mut out = vec![];
    for part in f.split([',']).flat_map(|p| p.split(" hoac ")).flat_map(|p| p.split(" hay ")).flat_map(|p| p.split(" va ")) {
        let p = format!(" {} ", part.trim());
        // vị trí con số đầu tiên đứng sau từ chỉ điều kiện
        let find = |keys: &[&str]| keys.iter().filter_map(|k| p.find(&format!(" {k} ")).or_else(|| if k.starts_with(['<', '>']) { p.find(k) } else { None }).map(|i| (i, k.len()))).min_by_key(|x| x.0);
        let (a, b) = (find(BELOW), find(ABOVE));
        let (op, at, explicit) = match (a, b) {
            (Some(x), Some(y)) if x.0 <= y.0 + 1 => (Op::Below, x.0 + x.1 + 1, true),
            (Some(x), None) => (Op::Below, x.0 + x.1 + 1, true),
            (_, Some(y)) => (Op::Above, y.0 + y.1 + 1, !p.contains(" cham ")),
            _ => continue,
        };
        let rest = p.get(at.min(p.len())..).unwrap_or("").trim_start();
        let tok: String = rest.split_whitespace().take(2).collect::<Vec<_>>().join("");
        if let Some(v) = parse_amount(&tok, index) {
            out.push((op, v, explicit));
        }
    }
    (!out.is_empty()).then_some((sym, out))
}

/// Ngưỡng bị chạm chưa: dùng cả giá cao/thấp của các nến gần nhất để không lỡ cú vọt nhanh
pub fn triggered(a: &Alert, last: f64, high: f64, low: f64) -> bool {
    match a.op {
        Op::Above => last >= a.target || high >= a.target,
        Op::Below => last <= a.target || low <= a.target,
    }
}

/// Giờ giao dịch (giờ Việt Nam, thứ 2–6, 9:00–15:00 — gồm phiên ATC)
pub fn trading_now(vn: NaiveDateTime) -> bool {
    let wd = vn.weekday();
    if wd == Weekday::Sat || wd == Weekday::Sun {
        return false;
    }
    let m = vn.hour() * 60 + vn.minute();
    (9 * 60..15 * 60 + 5).contains(&m)
}

// ───────────────────────── lưu cảnh báo ─────────────────────────

pub fn load_alerts(dir: &Path) -> Vec<Alert> {
    std::fs::read_to_string(dir.join("stock_alerts.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub fn save_alerts(dir: &Path, v: &[Alert]) -> Result<(), String> {
    let tmp = dir.join("stock_alerts.json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(v).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, dir.join("stock_alerts.json")).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn sym(c: &str) -> Symbol {
        Symbol { code: c.into(), kind: Kind::Stock }
    }

    #[test]
    fn parse_and_quote() {
        let v: Value = serde_json::from_str(r#"{"t":[1793664000,1793750400,1794009600],"o":[50,50.5,50.8],"h":[51,51,51.5],"l":[49.8,50.1,50.6],"c":[50.2,50.8,51],"v":[100,200,300],"nextTime":0}"#).unwrap();
        let b = parse_bars(&v).unwrap();
        assert_eq!(b.len(), 3);
        let q = quote_from_daily(&sym("FPT"), &b, None).unwrap();
        assert_eq!((q.price, q.reference, q.change), (51000.0, 50800.0, 200.0));
        assert_eq!(q.pct, 0.39);
        assert_eq!(quote_text(&q), "FPT 51.000đ ▲ +200đ (+0.39%)");
        let minute = Bar { t: 1794009600 + 3600, o: 51.2, h: 51.4, l: 51.2, c: 51.3, v: 1.0 };
        assert_eq!(quote_from_daily(&sym("FPT"), &b, Some(&minute)).unwrap().price, 51300.0);
        let e: Value = serde_json::from_str(r#"{"status":400,"code":"BAD_REQUEST","message":"invalid symbol"}"#).unwrap();
        assert_eq!(parse_bars(&e).unwrap_err(), "Không có mã này");
        let idx = Symbol { code: "VNINDEX".into(), kind: Kind::Index };
        let q = quote_from_daily(&idx, &parse_bars(&serde_json::json!({"t":[1,2],"c":[1725.36,1714.32],"o":[1,1],"h":[1,1],"l":[1,1],"v":[0,0]})).unwrap(), None).unwrap();
        assert_eq!(quote_text(&q), "VNINDEX 1.714,32 điểm ▼ -11,04 (-0.64%)");
        assert!(url(&idx, "1D", 1, 2).ends_with("/index?from=1&to=2&symbol=VNINDEX&resolution=1D"));
    }

    #[test]
    fn find_symbols() {
        let c = |t: &str| symbols_in(t).into_iter().map(|s| s.code).collect::<Vec<_>>();
        assert_eq!(c("giá FPT hôm nay"), vec!["FPT"]);
        assert_eq!(c("giá cổ phiếu fpt"), vec!["FPT"]);
        assert_eq!(c("HPG với VNM thế nào"), vec!["HPG", "VNM"]);
        assert_eq!(c("VN-Index hôm nay"), vec!["VNINDEX"]);
        assert_eq!(c("vnindex và VN30"), vec!["VNINDEX", "VN30"]);
        assert_eq!(c("tỷ giá USD"), Vec::<String>::new());
        assert_eq!(c("giá vàng SJC"), Vec::<String>::new());
    }

    #[test]
    fn alert_commands() {
        let (s, v) = parse_alert_command("báo cho t khi giá cổ phiếu FPT vượt 100k hoặc giảm dưới 50k").unwrap();
        assert_eq!(s.code, "FPT");
        assert_eq!(v, vec![(Op::Above, 100_000.0, true), (Op::Below, 50_000.0, true)]);
        let (_, v) = parse_alert_command("thông báo khi HPG xuống dưới 25.500").unwrap();
        assert_eq!(v, vec![(Op::Below, 25_500.0, true)]);
        let (s, v) = parse_alert_command("cảnh báo khi VN-Index lên trên 1.800 điểm").unwrap();
        assert_eq!((s.code.as_str(), v), ("VNINDEX", vec![(Op::Above, 1800.0, true)]));
        let (_, v) = parse_alert_command("báo mình khi VNM chạm 70").unwrap();
        assert_eq!(v, vec![(Op::Above, 70_000.0, false)]);
        assert!(parse_alert_command("giá FPT hôm nay").is_none());
        assert!(parse_alert_command("báo cho tôi khi có tin mới").is_none());
        assert_eq!(parse_amount("1tr", false), Some(1_000_000.0));
        assert_eq!(parse_amount("3hchieu", false), None, "giờ không phải giá");
        assert_eq!(parse_amount("100nghin", false), Some(100_000.0));
        assert!(parse_alert_command("nhắc tôi 3h chiều gọi FPT").is_none());
        assert_eq!(parse_amount("100k", false), Some(100_000.0));
        assert_eq!(parse_amount("100,5k", false), Some(100_500.0));
        assert_eq!(parse_amount("1.800", true), Some(1800.0));
    }

    #[test]
    fn trigger_and_hours() {
        let a = Alert { id: 1, symbol: "FPT".into(), index: false, op: Op::Above, target: 100_000.0, created: 0, fired: None, fired_price: None };
        assert!(!triggered(&a, 99_000.0, 99_900.0, 98_000.0));
        assert!(triggered(&a, 99_000.0, 100_100.0, 98_000.0), "vọt qua rồi giảm trong phút vẫn tính");
        let b = Alert { op: Op::Below, target: 50_000.0, ..a.clone() };
        assert!(triggered(&b, 50_100.0, 51_000.0, 49_900.0));
        assert_eq!(a.describe(), "FPT vượt 100.000đ");
        let d = |h, m| NaiveDate::from_ymd_opt(2026, 11, 9).unwrap().and_hms_opt(h, m, 0).unwrap();
        assert!(trading_now(d(10, 0)) && trading_now(d(14, 46)) && !trading_now(d(8, 59)) && !trading_now(d(15, 10)));
        assert!(!trading_now(NaiveDate::from_ymd_opt(2026, 11, 8).unwrap().and_hms_opt(10, 0, 0).unwrap()), "Chủ nhật");
        assert_eq!(fmt_price(51_300.0, false), "51.300đ");
        assert_eq!(fmt_price(25_550.5, false), "25.550,5đ");
    }
}
