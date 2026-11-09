//! Trả lời câu hỏi cần dữ liệu trực tiếp (thời tiết, tin tức, tỷ giá…) bằng nguồn thật — xem `live.rs`.
//! Không có nguồn → nói thật + nút tìm trên web, không bao giờ để mô hình đoán số.
use crate::assistant::{Card, Intent, LinkItem, Metric, Reply};
use crate::live::{self, Topic};
use crate::prelude::*;
use serde_json::Value;

const UA: &str = "Mini-Asi (desktop assistant)";

fn get_json(url: &str) -> Result<Value, String> {
    ureq::get(url).set("User-Agent", UA).timeout(Duration::from_secs(8)).call().map_err(|e| net_err(&e))?.into_json().map_err(|e| e.to_string())
}

fn get_text(url: &str) -> Result<String, String> {
    ureq::get(url).set("User-Agent", UA).timeout(Duration::from_secs(8)).call().map_err(|e| net_err(&e))?.into_string().map_err(|e| e.to_string())
}

fn net_err(e: &ureq::Error) -> String {
    match e {
        ureq::Error::Status(c, _) => format!("máy chủ trả về lỗi {c}"),
        ureq::Error::Transport(_) => "không có mạng hoặc máy chủ không phản hồi".into(),
    }
}

/// Vị trí đoán theo IP (nhớ trong phiên)
fn ip_place() -> Option<(String, f64, f64)> {
    static C: OnceLock<Mutex<Option<(String, f64, f64)>>> = OnceLock::new();
    let c = C.get_or_init(Default::default);
    if let Some(p) = c.lock().unwrap().clone() {
        return Some(p);
    }
    let v = get_json("https://ipwho.is/").ok()?;
    if v.get("success").and_then(Value::as_bool) == Some(false) {
        return None;
    }
    let p = (v.get("city")?.as_str()?.to_string(), v.get("latitude")?.as_f64()?, v.get("longitude")?.as_f64()?);
    *c.lock().unwrap() = Some(p.clone());
    Some(p)
}

fn geocode(name: &str) -> Option<(String, f64, f64)> {
    live::parse_geocode(&get_json(&live::geocode_url(name)).ok()?).or_else(|| {
        // tên có dấu không ra → thử bỏ dấu ("Đà Nẵng" → "Da Nang")
        let f = crate::nlp::fold(name);
        (f != name.to_lowercase()).then(|| live::parse_geocode(&get_json(&live::geocode_url(&f)).ok()?)).flatten()
    })
}

fn web_reply(text: String, query: &str, engine: &str) -> Reply {
    let mut r = Reply::new(Intent::Unknown, text);
    r.engine = engine.into();
    r.act("Xem trên web", "open_url", &live::search_url(query))
}

fn ago(ms: i64) -> String {
    if ms <= 0 {
        return String::new();
    }
    let m = (now_ms() - ms) / 60_000;
    if m < 1 {
        "vừa xong".into()
    } else if m < 60 {
        format!("{m} phút trước")
    } else if m < 24 * 60 {
        format!("{} giờ trước", m / 60)
    } else {
        format!("{} ngày trước", m / 1440)
    }
}

pub(crate) fn live_reply(st: &AppState, t: Topic, text: &str) -> Reply {
    match t {
        Topic::Weather { place, day } => {
            let set_place = st.store.lock().unwrap().settings.weather_place.trim().to_string();
            let loc = match place.as_deref() {
                Some(p) => geocode(p),
                None if !set_place.is_empty() => geocode(&set_place),
                None => ip_place(),
            };
            let Some((name, lat, lon)) = loc.or_else(|| place.is_none().then(|| ("Hà Nội".to_string(), 21.0285, 105.8542))) else {
                return web_reply(format!("Asi không tìm thấy địa danh “{}”.", place.unwrap_or_default()), text, "Open-Meteo");
            };
            match get_json(&live::weather_url(lat, lon)).ok().and_then(|v| live::parse_weather(&v, &name)) {
                Some(w) => {
                    let txt = live::weather_text(&w, day);
                    let mut r = Reply::new(Intent::Unknown, txt.clone());
                    r.speak = txt;
                    let r0 = |x: f64| if x.is_finite() { format!("{}", x.round() as i64) } else { "–".into() };
                    let d = w.days.get(day as usize).cloned();
                    let mut items = vec![];
                    if day == 0 {
                        items.push(Metric { label: "Bây giờ".into(), value: format!("{}°C", r0(w.temp)), level: 0, ratio: None });
                        items.push(Metric { label: "Độ ẩm".into(), value: format!("{}%", r0(w.humidity)), level: 0, ratio: None });
                    }
                    if let Some((_, lo, hi, rain, uv)) = d {
                        items.push(Metric { label: if day == 0 { "Hôm nay" } else { "Ngày mai" }.into(), value: format!("{}–{}°C", r0(lo), r0(hi)), level: 0, ratio: None });
                        items.push(Metric { label: "Khả năng mưa".into(), value: format!("{}%", r0(rain)), level: if rain >= 60.0 { 1 } else { 0 }, ratio: rain.is_finite().then_some(rain / 100.0) });
                        if day == 0 && uv.is_finite() {
                            items.push(Metric { label: "UV".into(), value: r0(uv), level: if uv >= 8.0 { 2 } else if uv >= 6.0 { 1 } else { 0 }, ratio: None });
                        }
                    }
                    r.cards.push(Card::Metrics { items });
                    r.mood = if d.is_some_and(|x| x.3 >= 60.0) { "worried".into() } else { "happy".into() };
                    r.engine = "Open-Meteo".into();
                    if place.is_none() && set_place.is_empty() {
                        r = r.act("Đổi nơi xem thời tiết", "open_tab", "ai");
                    }
                    r
                }
                None => web_reply("Asi chưa lấy được dữ liệu thời tiết lúc này (không có mạng hoặc máy chủ bận).".into(), &format!("thời tiết {name}"), "Open-Meteo"),
            }
        }
        Topic::News { cat, label } => match get_text(&live::rss_url(cat)).map(|x| live::parse_rss(&x, 6)) {
            Ok(h) if !h.is_empty() => {
                let first = h[0].title.clone();
                let mut r = Reply::new(Intent::Unknown, format!("Tin {label} trên VnExpress:"));
                r.speak = format!("Tin {label} mới nhất: {first}.");
                r.cards.push(Card::Links { items: h.into_iter().map(|x| LinkItem { meta: ago(x.at), title: x.title, url: x.url }).collect() });
                r.engine = "VnExpress".into();
                r
            }
            Ok(_) => web_reply(format!("Asi chưa đọc được tin {label} lúc này."), &format!("tin {label} hôm nay"), "VnExpress"),
            Err(e) => web_reply(format!("Asi chưa lấy được tin {label}: {e}."), &format!("tin {label} hôm nay"), "VnExpress"),
        },
        Topic::Fx => match get_text(live::FX_URL).map(|x| live::parse_fx(&x)) {
            Ok((when, mut rates)) if !rates.is_empty() => {
                if let Some(c) = live::fx_focus(text) {
                    if let Some(i) = rates.iter().position(|r| r.code == c) {
                        let x = rates.remove(i);
                        rates.insert(0, x);
                    }
                }
                let top = &rates[0];
                let txt = format!("Tỷ giá Vietcombank{}: {} mua {} – bán {} VND.", if when.is_empty() { String::new() } else { format!(" ({when})") }, top.code, top.buy, top.sell);
                let mut r = Reply::new(Intent::Unknown, txt);
                r.speak = format!("Một {} Vietcombank bán ra {} đồng.", top.code, top.sell.split('.').next().unwrap_or(&top.sell));
                r.cards.push(Card::Metrics { items: rates.iter().take(6).map(|x| Metric { label: x.code.clone(), value: format!("{} / {}", x.buy, x.sell), level: 0, ratio: None }).collect() });
                r.engine = "Vietcombank".into();
                r
            }
            _ => web_reply("Asi chưa lấy được bảng tỷ giá lúc này.".into(), "tỷ giá vietcombank hôm nay", "Vietcombank"),
        },
        Topic::Gold => web_reply("Asi chưa có nguồn giá vàng trực tiếp nên không đoán con số — bấm để xem giá SJC / nhẫn mới nhất.".into(), "giá vàng hôm nay", "Không có dữ liệu trực tiếp"),
        Topic::Stock => web_reply("Asi chưa có dữ liệu chứng khoán trực tiếp nên không đoán con số — bấm để xem VN-Index mới nhất.".into(), "VN-Index hôm nay", "Không có dữ liệu trực tiếp"),
        Topic::Fuel => web_reply("Asi chưa có nguồn giá xăng dầu trực tiếp — bấm để xem giá mới nhất.".into(), "giá xăng hôm nay", "Không có dữ liệu trực tiếp"),
        Topic::Crypto => web_reply("Asi chưa có dữ liệu tiền số trực tiếp — bấm để xem giá mới nhất.".into(), &format!("{text} giá"), "Không có dữ liệu trực tiếp"),
        Topic::Market => {
            let mut r = Reply::new(Intent::Unknown, "Bạn muốn xem thị trường nào? Asi chỉ đưa số liệu có nguồn thật, không đoán.");
            r.engine = "Lệnh cơ bản".into();
            r.act("Tỷ giá", "ask", "tỷ giá hôm nay")
                .act("Giá vàng", "open_url", &live::search_url("giá vàng hôm nay"))
                .act("Chứng khoán", "open_url", &live::search_url("VN-Index hôm nay"))
                .act("Tin kinh doanh", "ask", "tin kinh doanh hôm nay")
        }
    }
}
