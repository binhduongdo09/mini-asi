//! Tra giá cổ phiếu / chỉ số và cảnh báo giá — xem `stocks.rs` (nguồn: DNSE Entrade, miễn phí, nến 1 phút).
use crate::assistant::{Card, Intent, Metric, Reply};
use crate::prelude::*;
use crate::stocks::{self, Alert, Kind, Op, Quote, Symbol};

fn get_bars(sym: &Symbol, res: &str, secs_back: i64) -> Result<Vec<stocks::Bar>, String> {
    let to = chrono::Utc::now().timestamp();
    let v: serde_json::Value = ureq::get(&stocks::url(sym, res, to - secs_back, to))
        .set("User-Agent", "Mozilla/5.0 (Macintosh) Mini-Asi")
        .timeout(Duration::from_secs(8))
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(400, r) => {
                let body = r.into_string().unwrap_or_default();
                if body.contains("invalid symbol") { "Không có mã này".to_string() } else { "Nguồn dữ liệu báo lỗi".into() }
            }
            ureq::Error::Status(c, _) => format!("Nguồn dữ liệu báo lỗi {c}"),
            ureq::Error::Transport(_) => "Không kết nối được nguồn giá (kiểm tra mạng)".into(),
        })?
        .into_json()
        .map_err(|e| e.to_string())?;
    stocks::parse_bars(&v)
}

/// Giá hiện tại: nến ngày (tham chiếu) + nến phút gần nhất (giá khớp mới nhất)
pub(crate) fn fetch_quote(sym: &Symbol) -> Result<Quote, String> {
    let daily = get_bars(sym, "1D", 14 * 86_400)?;
    let minute = get_bars(sym, "1", 3 * 3600).ok().and_then(|v| v.last().copied());
    stocks::quote_from_daily(sym, &daily, minute.as_ref()).ok_or_else(|| format!("Chưa có dữ liệu giá {}", sym.code))
}

fn vn_now() -> chrono::NaiveDateTime {
    (chrono::Utc::now() + chrono::Duration::hours(7)).naive_utc()
}

fn quote_metric(q: &Quote) -> Metric {
    let sign = if q.change > 0.0 { "+" } else { "" };
    Metric {
        label: q.symbol.clone(),
        value: format!("{} {sign}{:.2}%", stocks::fmt_price(q.price, q.index), q.pct),
        level: if q.change > 0.0 { 0 } else if q.change < 0.0 { 2 } else { 1 },
        ratio: None,
    }
}

/// Câu về cổ phiếu / cảnh báo giá → câu trả lời. None = không phải (để trợ lý xử lý tiếp).
pub(crate) fn stock_reply(st: &AppState, text: &str) -> Option<Reply> {
    let f = format!(" {} ", nlp::fold(text));
    // 1. Xem / huỷ cảnh báo giá
    if f.contains("canh bao gia") || f.contains("canh bao co phieu") || (f.contains("canh bao") && !stocks::symbols_in(text).is_empty() && (f.contains(" huy ") || f.contains(" xoa ") || f.contains(" bo "))) {
        if f.contains(" huy ") || f.contains(" xoa ") || f.contains(" bo ") || f.contains(" tat ") {
            return Some(cancel_alerts(st, text));
        }
        if f.contains(" xem ") || f.contains("danh sach") || f.contains(" nhung ") || f.contains(" cac ") || f.contains(" dang ") {
            return Some(list_alerts_reply(st));
        }
    }
    // 2. Đặt cảnh báo
    if let Some((sym, conds)) = stocks::parse_alert_command(text) {
        return Some(add_alerts(st, sym, conds));
    }
    // 3. Tra giá
    let topic = crate::live::topic(text);
    let mut syms = stocks::symbols_in(text);
    let stock_ctx = matches!(topic, Some(crate::live::Topic::Stock)) || [" gia ", " bao nhieu ", " co phieu ", " ma ", " cp ", " tang ", " giam ", " the nao ", " hom nay "].iter().any(|k| f.contains(k));
    if syms.is_empty() {
        if !matches!(topic, Some(crate::live::Topic::Stock)) {
            return None;
        }
        syms = vec![Symbol { code: "VNINDEX".into(), kind: Kind::Index }, Symbol { code: "VN30".into(), kind: Kind::Index }];
    } else if !stock_ctx {
        return None;
    }
    let mut quotes = vec![];
    let mut errs = vec![];
    for s in syms.iter().take(6) {
        match fetch_quote(s) {
            Ok(q) => quotes.push(q),
            Err(e) => errs.push((s.code.clone(), e)),
        }
    }
    if quotes.is_empty() {
        // mã đoán nhầm (chữ viết hoa bình thường) và câu không rõ là hỏi giá → để trợ lý trả lời
        if errs.iter().all(|(_, e)| e.contains("Không có mã")) && !f.contains(" co phieu ") && !f.contains(" ma ") && !matches!(topic, Some(crate::live::Topic::Stock)) {
            return None;
        }
        let msg = errs.iter().map(|(c, e)| format!("{c}: {e}")).collect::<Vec<_>>().join("; ");
        let mut r = Reply::new(Intent::Unknown, format!("Asi chưa lấy được giá — {msg}."));
        r.engine = "DNSE Entrade".into();
        r.mood = "worried".into();
        return Some(r.act("Xem trên web", "open_url", &crate::live::search_url(&format!("giá cổ phiếu {}", syms[0].code))));
    }
    let open = stocks::trading_now(vn_now());
    let at = chrono::DateTime::from_timestamp(quotes[0].at, 0).map(|d| (d + chrono::Duration::hours(7)).format("%H:%M %d/%m").to_string()).unwrap_or_default();
    let text_main = quotes.iter().map(stocks::quote_text).collect::<Vec<_>>().join(" · ");
    let mut r = Reply::new(Intent::Unknown, format!("{text_main}{}", if open { String::new() } else { format!(" — giá đóng cửa phiên gần nhất ({at})") }));
    r.speak = quotes.iter().map(|q| format!("{} {}", q.symbol, stocks::fmt_price(q.price, q.index).replace('đ', " đồng"))).collect::<Vec<_>>().join(", ");
    r.cards.push(Card::Metrics { items: quotes.iter().map(quote_metric).collect() });
    if let Some(q) = quotes.iter().find(|q| !q.index) {
        r.cards.push(Card::Text { text: format!("{}: cao nhất {} · thấp nhất {} · tham chiếu {} · KL {}", q.symbol, stocks::fmt_price(q.high, false), stocks::fmt_price(q.low, false), stocks::fmt_price(q.reference, false), stocks::fmt_price(q.volume, true).trim_end_matches(",00")) });
    }
    for (c, e) in &errs {
        r.cards.push(Card::Text { text: format!("{c}: {e}") });
    }
    r.engine = format!("DNSE Entrade · {}", if open { format!("cập nhật {at}") } else { "ngoài giờ giao dịch".into() });
    if let Some(q) = quotes.iter().find(|q| !q.index) {
        let up = (q.price * 1.05 / 100.0).round() * 100.0;
        r = r.act(&format!("Báo khi {} > {}", q.symbol, stocks::fmt_price(up, false)), "ask", &format!("báo cho tôi khi {} vượt {}", q.symbol, up as i64));
    }
    Some(r)
}

fn add_alerts(st: &AppState, sym: Symbol, conds: Vec<(Op, f64, bool)>) -> Reply {
    let cur = fetch_quote(&sym);
    if let Err(e) = &cur {
        if e.contains("Không có mã") {
            let mut r = Reply::new(Intent::Unknown, format!("Không tìm thấy mã {} trên HOSE / HNX / UPCOM.", sym.code));
            r.mood = "worried".into();
            r.engine = "DNSE Entrade".into();
            return r;
        }
    }
    let price = cur.as_ref().ok().map(|q| q.price);
    let mut list = stocks::load_alerts(&st.data_dir);
    let mut made = vec![];
    for (op, target, explicit) in conds {
        // "chạm 60k": so với giá hiện tại để biết là chờ tăng hay chờ giảm
        let op = if !explicit { price.map(|p| if target >= p { Op::Above } else { Op::Below }).unwrap_or(op) } else { op };
        let a = Alert { id: now_ms() as u64 + made.len() as u64, symbol: sym.code.clone(), index: sym.kind == Kind::Index, op, target, created: now_ms(), fired: None, fired_price: None };
        list.retain(|x| !(x.fired.is_none() && x.symbol == a.symbol && x.op == a.op && (x.target - a.target).abs() < 0.01));
        made.push(a.describe());
        list.push(a);
    }
    let _ = stocks::save_alerts(&st.data_dir, &list);
    let now_txt = cur.as_ref().map(|q| format!(" Hiện tại: {}.", stocks::quote_text(q))).unwrap_or_default();
    let mut r = Reply::new(Intent::Unknown, format!("Asi sẽ báo khi {}.{now_txt} Asi kiểm tra mỗi phút trong giờ giao dịch (9:00–15:00, thứ 2–6).", made.join(" hoặc ")));
    r.speak = format!("Được, Asi sẽ báo khi {}.", made.join(" hoặc ").replace('đ', " đồng"));
    r.engine = "Cảnh báo giá".into();
    r.act("Xem các cảnh báo", "ask", "xem cảnh báo giá").act(&format!("Huỷ cảnh báo {}", sym.code), "ask", &format!("huỷ cảnh báo giá {}", sym.code))
}

fn list_alerts_reply(st: &AppState) -> Reply {
    let list = stocks::load_alerts(&st.data_dir);
    let active: Vec<&Alert> = list.iter().filter(|a| a.fired.is_none()).collect();
    if active.is_empty() {
        let mut r = Reply::new(Intent::Unknown, "Chưa có cảnh báo giá nào. Thử nói: \"báo cho tôi khi FPT vượt 100k\".");
        r.engine = "Cảnh báo giá".into();
        return r;
    }
    let mut r = Reply::new(Intent::Unknown, format!("Đang theo dõi {} cảnh báo:", active.len()));
    r.cards.push(Card::Text { text: active.iter().map(|a| format!("- {}", a.describe())).collect::<Vec<_>>().join("\n") });
    r.engine = "Cảnh báo giá".into();
    r.act("Huỷ tất cả", "ask", "huỷ tất cả cảnh báo giá")
}

fn cancel_alerts(st: &AppState, text: &str) -> Reply {
    let syms: Vec<String> = stocks::symbols_in(text).into_iter().map(|s| s.code).collect();
    let mut list = stocks::load_alerts(&st.data_dir);
    let before = list.iter().filter(|a| a.fired.is_none()).count();
    list.retain(|a| a.fired.is_some() || !(syms.is_empty() || syms.contains(&a.symbol)));
    let removed = before - list.iter().filter(|a| a.fired.is_none()).count();
    let _ = stocks::save_alerts(&st.data_dir, &list);
    let mut r = Reply::new(Intent::Unknown, if removed == 0 { "Không có cảnh báo nào để huỷ.".to_string() } else { format!("Đã huỷ {removed} cảnh báo giá{}.", if syms.is_empty() { String::new() } else { format!(" của {}", syms.join(", ")) }) });
    r.engine = "Cảnh báo giá".into();
    r
}

#[derive(Serialize, Clone)]
pub(crate) struct StockAlertFired {
    alert: Alert,
    price: f64,
    /// "FPT vượt 100.000đ"
    title: String,
    text: String,
}

/// Luồng nền: trong giờ giao dịch, mỗi phút kiểm tra các cảnh báo đang chờ
pub(crate) fn stock_alert_loop(app: AppHandle, st: Arc<AppState>) {
    std::thread::sleep(Duration::from_secs(30));
    loop {
        let active: Vec<Alert> = stocks::load_alerts(&st.data_dir).into_iter().filter(|a| a.fired.is_none()).collect();
        if active.is_empty() || !stocks::trading_now(vn_now()) {
            std::thread::sleep(Duration::from_secs(if active.is_empty() { 60 } else { 120 }));
            continue;
        }
        let mut codes: Vec<(String, bool)> = active.iter().map(|a| (a.symbol.clone(), a.index)).collect();
        codes.sort();
        codes.dedup();
        let mut fired = vec![];
        for (code, index) in codes {
            let sym = Symbol { code: code.clone(), kind: if index { Kind::Index } else { Kind::Stock } };
            let Ok(bars) = get_bars(&sym, "1", 6 * 60) else { continue };
            let Some(last) = bars.last() else { continue };
            let mul = if index { 1.0 } else { 1000.0 };
            let recent = bars.iter().rev().take(3);
            let (hi, lo) = recent.fold((f64::MIN, f64::MAX), |(h, l), b| (h.max(b.h), l.min(b.l)));
            for a in active.iter().filter(|a| a.symbol == code) {
                if stocks::triggered(a, last.c * mul, hi * mul, lo * mul) {
                    fired.push((a.id, last.c * mul));
                }
            }
        }
        if !fired.is_empty() {
            let mut list = stocks::load_alerts(&st.data_dir);
            for (id, price) in &fired {
                if let Some(a) = list.iter_mut().find(|a| a.id == *id && a.fired.is_none()) {
                    a.fired = Some(now_ms());
                    a.fired_price = Some(*price);
                    let text = tl(&app, &format!("Giá hiện tại {}", stocks::fmt_price(*price, a.index)), &format!("Current price {}", stocks::fmt_price(*price, a.index)));
                    notify(&app, &format!("📈 {}", a.describe()), &text);
                    let _ = app.emit("stock-alert", StockAlertFired { title: format!("{} rồi", a.describe()), alert: a.clone(), price: *price, text });
                }
            }
            let _ = stocks::save_alerts(&st.data_dir, &list);
        }
        std::thread::sleep(Duration::from_secs(60));
    }
}

#[tauri::command]
pub(crate) fn stock_alerts(st: S) -> Vec<Alert> {
    stocks::load_alerts(&st.data_dir)
}

#[tauri::command]
pub(crate) fn stock_alert_delete(st: S, id: u64) -> Result<(), String> {
    let mut list = stocks::load_alerts(&st.data_dir);
    list.retain(|a| a.id != id);
    stocks::save_alerts(&st.data_dir, &list)
}
