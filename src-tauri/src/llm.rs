//! "Bộ não" đám mây tuỳ chọn cho Asi: ChatGPT (OpenAI), Gemini (Google), Claude (Anthropic) — dùng API key
//! của chính người dùng (lưu trong Keychain, không nằm trong file cài đặt / bản sao lưu).
//!
//! Mỗi lượt: gửi chỉ dẫn + lịch sử + câu hỏi, yêu cầu trả về JSON `{reply, action, argument}` (cùng dạng với
//! Apple Intelligence — xem apple_ai.rs) để app biết có cần thêm việc / ghi chú… hay chỉ trả lời.
//! Phần dựng yêu cầu và đọc kết quả là hàm thuần (có test); phần gửi mạng dùng `ureq`.

use crate::apple_ai::{AiTurn, ACTIONS};
use serde_json::{json, Value};
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(45);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    OpenAi,
    Gemini,
    Claude,
}

pub const ALL: [Provider; 3] = [Provider::OpenAi, Provider::Gemini, Provider::Claude];

impl Provider {
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "openai" | "chatgpt" => Some(Self::OpenAi),
            "gemini" => Some(Self::Gemini),
            "claude" => Some(Self::Claude),
            _ => None,
        }
    }
    pub fn id(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Gemini => "gemini",
            Self::Claude => "claude",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::OpenAi => "ChatGPT",
            Self::Gemini => "Gemini",
            Self::Claude => "Claude",
        }
    }
    /// Mô hình mặc định: nhanh, rẻ, tiếng Việt tốt. Bấm "Kiểm tra" trong Cài đặt sẽ tự đổi sang tên mới nhất nếu tên này đã cũ.
    pub fn default_model(self) -> &'static str {
        match self {
            Self::OpenAi => "gpt-5-mini",
            Self::Gemini => "gemini-flash-latest",
            Self::Claude => "claude-haiku-4-5",
        }
    }
    /// Tên mục trong Keychain
    pub fn key_name(self) -> String {
        format!("ai_key_{}", self.id())
    }
    /// Trang lấy API key
    pub fn key_url(self) -> &'static str {
        match self {
            Self::OpenAi => "https://platform.openai.com/api-keys",
            Self::Gemini => "https://aistudio.google.com/apikey",
            Self::Claude => "https://console.anthropic.com/settings/keys",
        }
    }
}

/// Dặn mô hình trả về đúng một đối tượng JSON
pub fn json_rules(allow_actions: bool) -> String {
    let acts = ACTIONS.join(", ");
    format!(
        "\n\nOutput format: respond with ONLY one JSON object, no markdown fences: \
         {{\"reply\": string, \"action\": string, \"argument\": string}}. \
         action is one of: {acts}.{}",
        if allow_actions { "" } else { " For this turn action MUST be \"none\" and argument \"\" — just answer the question." }
    )
}

pub struct HttpRequest {
    pub url: String,
    pub headers: Vec<(&'static str, String)>,
    pub body: Value,
}

/// Dựng yêu cầu gửi cho từng nhà cung cấp
pub fn build_request(p: Provider, key: &str, model: &str, system: &str, prompt: &str) -> HttpRequest {
    match p {
        Provider::OpenAi => HttpRequest {
            url: "https://api.openai.com/v1/chat/completions".into(),
            headers: vec![("Authorization", format!("Bearer {key}"))],
            body: json!({
                "model": model,
                "messages": [{ "role": "system", "content": system }, { "role": "user", "content": prompt }],
                "response_format": { "type": "json_object" },
            }),
        },
        Provider::Gemini => HttpRequest {
            url: format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent"),
            headers: vec![("x-goog-api-key", key.to_string())],
            body: json!({
                "systemInstruction": { "parts": [{ "text": system }] },
                "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
                "generationConfig": { "responseMimeType": "application/json", "temperature": 0.6 },
            }),
        },
        Provider::Claude => HttpRequest {
            url: "https://api.anthropic.com/v1/messages".into(),
            headers: vec![("x-api-key", key.to_string()), ("anthropic-version", "2023-06-01".into())],
            body: json!({
                "model": model,
                "max_tokens": 1500,
                "system": system,
                "messages": [{ "role": "user", "content": prompt }],
            }),
        },
    }
}

/// Lấy chữ mô hình trả lời từ phản hồi JSON của từng nhà cung cấp
pub fn extract_text(p: Provider, v: &Value) -> Result<String, String> {
    if let Some(e) = api_error(v) {
        return Err(e);
    }
    let text = match p {
        Provider::OpenAi => v["choices"][0]["message"]["content"].as_str().map(str::to_string),
        Provider::Gemini => v["candidates"][0]["content"]["parts"]
            .as_array()
            .map(|parts| parts.iter().filter(|x| x["thought"].as_bool() != Some(true)).filter_map(|x| x["text"].as_str()).collect::<Vec<_>>().join("")),
        Provider::Claude => v["content"].as_array().map(|c| c.iter().filter(|x| x["type"] == "text").filter_map(|x| x["text"].as_str()).collect::<Vec<_>>().join("")),
    };
    match text {
        Some(t) if !t.trim().is_empty() => Ok(t),
        _ if p == Provider::Gemini && v["candidates"][0]["finishReason"] == "SAFETY" => Err("Gemini từ chối trả lời câu này (bộ lọc an toàn)".into()),
        _ => Err(format!("{} không trả về nội dung", p.name())),
    }
}

fn api_error(v: &Value) -> Option<String> {
    let e = &v["error"];
    if e.is_null() {
        return None;
    }
    Some(e["message"].as_str().or_else(|| e.as_str()).unwrap_or("Lỗi không rõ").to_string())
}

/// Thông báo lỗi dễ hiểu theo mã HTTP
pub fn explain_status(p: Provider, code: u16, body: &str) -> String {
    let detail = serde_json::from_str::<Value>(body).ok().and_then(|v| api_error(&v)).unwrap_or_default();
    let head = match code {
        400 if detail.to_lowercase().contains("api key") => format!("API key {} không hợp lệ", p.name()),
        401 | 403 => format!("API key {} không hợp lệ hoặc chưa được cấp quyền", p.name()),
        404 => format!("Không tìm thấy mô hình — bấm Kiểm tra trong Cài đặt để chọn lại mô hình {}", p.name()),
        429 => format!("{} báo hết hạn mức / quá nhiều yêu cầu — kiểm tra tài khoản thanh toán", p.name()),
        500..=599 => format!("Máy chủ {} đang lỗi, thử lại sau", p.name()),
        _ => format!("{} trả lỗi {code}", p.name()),
    };
    if detail.is_empty() { head } else { format!("{head} ({})", detail.chars().take(160).collect::<String>()) }
}

/// Gửi câu hỏi, trả về chữ mô hình trả lời
pub fn ask(p: Provider, key: &str, model: &str, system: &str, prompt: &str) -> Result<String, String> {
    ask_with_timeout(p, key, model, system, prompt, TIMEOUT)
}

/// Như `ask` nhưng chờ lâu hơn (tóm tắt cả cuộc họp dài)
pub fn ask_with_timeout(p: Provider, key: &str, model: &str, system: &str, prompt: &str, timeout: Duration) -> Result<String, String> {
    let r = build_request(p, key, model, system, prompt);
    let mut req = ureq::post(&r.url).timeout(timeout).set("Content-Type", "application/json");
    for (k, v) in &r.headers {
        req = req.set(k, v);
    }
    let v: Value = match req.send_json(r.body) {
        Ok(resp) => resp.into_json().map_err(|e| format!("Không đọc được phản hồi {}: {e}", p.name()))?,
        Err(ureq::Error::Status(code, resp)) => return Err(explain_status(p, code, &resp.into_string().unwrap_or_default())),
        Err(e) => return Err(format!("Không kết nối được {} — kiểm tra mạng ({e})", p.name())),
    };
    extract_text(p, &v)
}

/// Danh sách mô hình dùng được với key này (để chọn trong Cài đặt)
pub fn list_models(p: Provider, key: &str) -> Result<Vec<String>, String> {
    let (url, headers): (String, Vec<(&str, String)>) = match p {
        Provider::OpenAi => ("https://api.openai.com/v1/models".into(), vec![("Authorization", format!("Bearer {key}"))]),
        Provider::Gemini => ("https://generativelanguage.googleapis.com/v1beta/models?pageSize=200".into(), vec![("x-goog-api-key", key.to_string())]),
        Provider::Claude => ("https://api.anthropic.com/v1/models?limit=100".into(), vec![("x-api-key", key.to_string()), ("anthropic-version", "2023-06-01".into())]),
    };
    let mut req = ureq::get(&url).timeout(Duration::from_secs(15));
    for (k, v) in &headers {
        req = req.set(k, v);
    }
    let v: Value = match req.call() {
        Ok(resp) => resp.into_json().map_err(|e| e.to_string())?,
        Err(ureq::Error::Status(code, resp)) => return Err(explain_status(p, code, &resp.into_string().unwrap_or_default())),
        Err(e) => return Err(format!("Không kết nối được {} — kiểm tra mạng ({e})", p.name())),
    };
    Ok(parse_models(p, &v))
}

/// Chỉ giữ mô hình trò chuyện (bỏ mô hình ảnh, âm thanh, nhúng…)
pub fn parse_models(p: Provider, v: &Value) -> Vec<String> {
    let bad = ["embed", "audio", "realtime", "tts", "transcribe", "image", "dall", "whisper", "moderation", "search", "computer", "imagen", "veo", "aqa", "babbage", "davinci", "codex", "live"];
    let ok = |id: &str| !bad.iter().any(|b| id.to_lowercase().contains(b));
    match p {
        Provider::OpenAi => {
            let mut ids: Vec<String> = v["data"].as_array().into_iter().flatten().filter_map(|m| m["id"].as_str()).filter(|id| (id.starts_with("gpt-") || id.starts_with('o')) && ok(id)).map(str::to_string).collect();
            ids.sort();
            ids.reverse();
            ids
        }
        Provider::Gemini => v["models"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|m| m["supportedGenerationMethods"].as_array().is_some_and(|a| a.iter().any(|x| x == "generateContent")))
            .filter_map(|m| m["name"].as_str())
            .map(|n| n.trim_start_matches("models/").to_string())
            .filter(|id| id.starts_with("gemini") && ok(id))
            .collect(),
        // Anthropic trả về mới nhất trước
        Provider::Claude => v["data"].as_array().into_iter().flatten().filter_map(|m| m["id"].as_str()).filter(|id| ok(id)).map(str::to_string).collect(),
    }
}

/// Chọn mô hình hợp lý nhất trong danh sách khi tên đang dùng không còn (ưu tiên loại nhanh & rẻ)
pub fn pick_model(p: Provider, current: &str, list: &[String]) -> Option<String> {
    if list.iter().any(|m| m == current) {
        return Some(current.to_string());
    }
    let want: &[&str] = match p {
        Provider::OpenAi => &["gpt-5-mini", "mini"],
        Provider::Gemini => &["gemini-flash-latest", "flash"],
        Provider::Claude => &["haiku", "sonnet"],
    };
    for w in want {
        if let Some(m) = list.iter().find(|m| m.as_str() == *w).or_else(|| list.iter().find(|m| m.contains(w) && !m.contains("lite") && !m.contains("preview"))) {
            return Some(m.clone());
        }
    }
    list.first().cloned()
}

/// Đọc JSON mô hình trả về (chịu được ```json … ``` hay chữ thừa). Không phải JSON → coi cả đoạn là câu trả lời.
pub fn parse_turn(raw: &str) -> AiTurn {
    let t = raw.trim();
    let json_part = match (t.find('{'), t.rfind('}')) {
        (Some(a), Some(b)) if b > a => Some(&t[a..=b]),
        _ => None,
    };
    if let Some(Ok(v)) = json_part.map(serde_json::from_str::<Value>) {
        let s = |k: &str| v[k].as_str().unwrap_or("").trim().to_string();
        let mut turn = AiTurn { reply: s("reply"), action: s("action").to_lowercase(), argument: s("argument") };
        if !ACTIONS.contains(&turn.action.as_str()) {
            turn.action = "none".into();
        }
        if !turn.reply.is_empty() || turn.action != "none" {
            return turn;
        }
    }
    let clean = t.trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();
    AiTurn { reply: clean.to_string(), action: "none".into(), argument: String::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_have_key_model_and_json_mode() {
        let r = build_request(Provider::OpenAi, "sk-x", "gpt-5-mini", "SYS", "Hỏi");
        assert_eq!(r.headers[0], ("Authorization", "Bearer sk-x".to_string()));
        assert_eq!(r.body["model"], "gpt-5-mini");
        assert_eq!(r.body["messages"][0]["content"], "SYS");
        assert_eq!(r.body["response_format"]["type"], "json_object");
        let g = build_request(Provider::Gemini, "AIza", "gemini-flash-latest", "SYS", "Hỏi");
        assert!(g.url.ends_with("models/gemini-flash-latest:generateContent"));
        assert_eq!(g.body["generationConfig"]["responseMimeType"], "application/json");
        let c = build_request(Provider::Claude, "sk-ant", "claude-haiku-4-5", "SYS", "Hỏi");
        assert!(c.headers.iter().any(|h| h.0 == "anthropic-version"));
        assert_eq!(c.body["system"], "SYS");
    }

    #[test]
    fn extract_text_each_provider() {
        let o = json!({"choices":[{"message":{"content":"{\"reply\":\"A\"}"}}]});
        assert_eq!(extract_text(Provider::OpenAi, &o).unwrap(), "{\"reply\":\"A\"}");
        let g = json!({"candidates":[{"content":{"parts":[{"text":"nghĩ…","thought":true},{"text":"B"}]}}]});
        assert_eq!(extract_text(Provider::Gemini, &g).unwrap(), "B");
        let c = json!({"content":[{"type":"text","text":"C"}]});
        assert_eq!(extract_text(Provider::Claude, &c).unwrap(), "C");
        let e = json!({"error":{"message":"Incorrect API key"}});
        assert_eq!(extract_text(Provider::OpenAi, &e).unwrap_err(), "Incorrect API key");
    }

    #[test]
    fn parse_turn_is_lenient() {
        let t = parse_turn("```json\n{\"reply\":\"Thủ đô Pháp là Paris.\",\"action\":\"none\",\"argument\":\"\"}\n```");
        assert_eq!(t.reply, "Thủ đô Pháp là Paris.");
        assert_eq!(t.action, "none");
        let t = parse_turn("{\"reply\":\"Ok\",\"action\":\"ADD_TASK\",\"argument\":\"mai 9h họp\"}");
        assert_eq!((t.action.as_str(), t.argument.as_str()), ("add_task", "mai 9h họp"));
        let t = parse_turn("Chỉ là chữ thường, không có JSON.");
        assert_eq!((t.reply.as_str(), t.action.as_str()), ("Chỉ là chữ thường, không có JSON.", "none"));
        assert_eq!(parse_turn("{\"reply\":\"x\",\"action\":\"rm_rf\"}").action, "none");
    }

    #[test]
    fn models_filtered_and_picked() {
        let o = json!({"data":[{"id":"gpt-5-mini"},{"id":"gpt-4o-realtime"},{"id":"text-embedding-3"},{"id":"o4-mini"},{"id":"dall-e-3"}]});
        let ids = parse_models(Provider::OpenAi, &o);
        assert!(ids.contains(&"gpt-5-mini".to_string()) && ids.contains(&"o4-mini".to_string()));
        assert!(!ids.iter().any(|i| i.contains("realtime") || i.contains("embedding") || i.contains("dall")));
        let g = json!({"models":[{"name":"models/gemini-3-flash","supportedGenerationMethods":["generateContent"]},{"name":"models/text-embedding-004","supportedGenerationMethods":["embedContent"]}]});
        assert_eq!(parse_models(Provider::Gemini, &g), vec!["gemini-3-flash".to_string()]);
        let list = vec!["claude-sonnet-5".to_string(), "claude-haiku-5-5".to_string()];
        assert_eq!(pick_model(Provider::Claude, "claude-haiku-4-5", &list).as_deref(), Some("claude-haiku-5-5"), "tên cũ → haiku mới nhất");
        assert_eq!(pick_model(Provider::Claude, "claude-sonnet-5", &list).as_deref(), Some("claude-sonnet-5"), "giữ lựa chọn của người dùng");
    }

    #[test]
    fn status_messages_are_vietnamese() {
        assert!(explain_status(Provider::OpenAi, 401, "{\"error\":{\"message\":\"bad\"}}").contains("không hợp lệ"));
        assert!(explain_status(Provider::Gemini, 429, "").contains("hạn mức"));
        assert!(explain_status(Provider::Claude, 404, "").contains("Kiểm tra"));
    }
}
