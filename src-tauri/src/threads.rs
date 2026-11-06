//! Kết nối Threads (Meta) qua Threads API chính thức — graph.threads.net/v1.0
//!
//! • Đăng nhập: OAuth (App ID + App Secret của bạn) hoặc dán access token
//! • Đọc: hồ sơ, bài gần đây + chỉ số, trả lời dưới bài, lượt nhắc đến bạn
//! • Ghi: CHỈ đăng bài / trả lời khi người dùng bấm "Đăng" và xác nhận
//! • Token & App Secret lưu trong Keychain (macOS) / Credential Manager (Windows)

use serde::{Deserialize, Serialize};
use serde_json::Value;

const GRAPH: &str = "https://graph.threads.net";
const VER: &str = "v1.0";
pub const SCOPES: &str = "threads_basic,threads_content_publish,threads_read_replies,threads_manage_replies,threads_manage_insights,threads_manage_mentions";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Profile {
    pub id: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub threads_biography: Option<String>,
    #[serde(default)]
    pub is_verified: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Metrics {
    pub views: u64,
    pub likes: u64,
    pub replies: u64,
    pub reposts: u64,
    pub quotes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Post {
    pub id: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub timestamp: Option<String>,
    #[serde(default)]
    pub permalink: Option<String>,
    #[serde(default)]
    pub media_type: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub metrics: Option<Metrics>,
    /// Số trả lời mới (chưa xem) dưới bài này
    #[serde(default)]
    pub unread: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Reply {
    pub id: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub timestamp: Option<String>,
    #[serde(default)]
    pub permalink: Option<String>,
    #[serde(default)]
    pub is_reply_owned_by_me: Option<bool>,
    #[serde(default)]
    pub has_replies: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Overview {
    pub followers: Option<u64>,
    pub views: u64,
    pub likes: u64,
    pub replies: u64,
}

fn err_of(e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(code, resp) => {
            let body: Value = resp.into_json().unwrap_or(Value::Null);
            let msg = body["error"]["message"].as_str().unwrap_or("").to_string();
            let api_code = body["error"]["code"].as_i64().unwrap_or(0);
            match code {
                _ if api_code == 190 => "Phiên đăng nhập Threads đã hết hạn — đăng nhập lại nhé".into(),
                401 => "Phiên đăng nhập Threads đã hết hạn — đăng nhập lại nhé".into(),
                _ if msg.contains("permission") || msg.contains("scope") => format!("Thiếu quyền Threads: {msg}"),
                _ => format!("Threads báo lỗi {code}: {msg}"),
            }
        }
        ureq::Error::Transport(t) => format!("Không kết nối được Threads: {t}"),
    }
}

fn get(path: &str, token: &str, params: &[(&str, &str)]) -> Result<Value, String> {
    let url = if path.starts_with("http") { path.to_string() } else { format!("{GRAPH}/{VER}/{path}") };
    let mut req = ureq::get(&url).timeout(std::time::Duration::from_secs(20)).query("access_token", token);
    for (k, v) in params {
        req = req.query(k, v);
    }
    req.call().map_err(err_of)?.into_json().map_err(|e| e.to_string())
}

fn post_form(path: &str, form: &[(&str, &str)]) -> Result<Value, String> {
    let url = if path.starts_with("http") { path.to_string() } else { format!("{GRAPH}/{VER}/{path}") };
    ureq::post(&url).timeout(std::time::Duration::from_secs(30)).send_form(form).map_err(err_of)?.into_json().map_err(|e| e.to_string())
}

// ───────────────────────── Đăng nhập ─────────────────────────

pub fn auth_url(app_id: &str, redirect: &str, state: &str) -> String {
    let enc = |s: &str| s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect::<String>();
    format!(
        "https://threads.net/oauth/authorize?client_id={}&redirect_uri={}&scope={}&response_type=code&state={}",
        enc(app_id),
        enc(redirect),
        enc(SCOPES),
        enc(state)
    )
}

/// Lấy `code` từ địa chỉ mà trình duyệt chuyển tới sau khi đăng nhập (người dùng dán vào).
pub fn extract_code(pasted: &str) -> Option<String> {
    let s = pasted.trim();
    if !s.contains("code=") {
        // dán thẳng mã code
        let bare = !s.is_empty() && !s.contains(' ') && !s.contains("://") && !s.contains('=');
        return bare.then(|| s.trim_end_matches("#_").to_string());
    }
    let q = s.split_once('?').map(|x| x.1).unwrap_or(s);
    for kv in q.split('&') {
        if let Some(v) = kv.strip_prefix("code=") {
            let v = v.split('#').next().unwrap_or(v);
            return Some(percent_decode(v));
        }
    }
    None
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// code → token ngắn hạn → token dài hạn (60 ngày). Trả về (token, số giây còn hạn)
pub fn login_with_code(app_id: &str, secret: &str, redirect: &str, code: &str) -> Result<(String, u64), String> {
    let v = post_form(
        &format!("{GRAPH}/oauth/access_token"),
        &[("client_id", app_id), ("client_secret", secret), ("code", code), ("grant_type", "authorization_code"), ("redirect_uri", redirect)],
    )?;
    let short = v["access_token"].as_str().ok_or("Threads không trả về token")?.to_string();
    long_lived(secret, &short)
}

pub fn long_lived(secret: &str, short: &str) -> Result<(String, u64), String> {
    let v = ureq::get(&format!("{GRAPH}/access_token"))
        .query("grant_type", "th_exchange_token")
        .query("client_secret", secret)
        .query("access_token", short)
        .call()
        .map_err(err_of)?
        .into_json::<Value>()
        .map_err(|e| e.to_string())?;
    Ok((v["access_token"].as_str().ok_or("Không đổi được token dài hạn")?.to_string(), v["expires_in"].as_u64().unwrap_or(5_184_000)))
}

pub fn refresh(token: &str) -> Result<(String, u64), String> {
    let v = ureq::get(&format!("{GRAPH}/refresh_access_token"))
        .query("grant_type", "th_refresh_token")
        .query("access_token", token)
        .call()
        .map_err(err_of)?
        .into_json::<Value>()
        .map_err(|e| e.to_string())?;
    Ok((v["access_token"].as_str().ok_or("Không gia hạn được token")?.to_string(), v["expires_in"].as_u64().unwrap_or(5_184_000)))
}

// ───────────────────────── Đọc ─────────────────────────

pub fn me(token: &str) -> Result<Profile, String> {
    let v = get("me", token, &[("fields", "id,username,name,threads_biography,is_verified")])?;
    serde_json::from_value(v).map_err(|e| e.to_string())
}

pub fn posts(token: &str, limit: u32) -> Result<Vec<Post>, String> {
    let l = limit.to_string();
    let v = get("me/threads", token, &[("fields", "id,text,timestamp,permalink,media_type,username"), ("limit", &l)])?;
    Ok(serde_json::from_value::<Vec<Post>>(v["data"].clone()).unwrap_or_default().into_iter().filter(|p| p.media_type.as_deref() != Some("REPOST_FACADE")).collect())
}

/// Số liệu: hỗ trợ cả dạng `values[0].value` lẫn `total_value.value`
pub fn parse_metrics(v: &Value) -> Metrics {
    let mut m = Metrics::default();
    for item in v["data"].as_array().cloned().unwrap_or_default() {
        let val = item["total_value"]["value"]
            .as_u64()
            .or_else(|| item["values"].as_array().map(|a| a.iter().filter_map(|x| x["value"].as_u64()).sum()))
            .unwrap_or(0);
        match item["name"].as_str().unwrap_or("") {
            "views" => m.views = val,
            "likes" => m.likes = val,
            "replies" => m.replies = val,
            "reposts" => m.reposts = val,
            "quotes" => m.quotes = val,
            _ => {}
        }
    }
    m
}

pub fn post_metrics(token: &str, id: &str) -> Result<Metrics, String> {
    Ok(parse_metrics(&get(&format!("{id}/insights"), token, &[("metric", "views,likes,replies,reposts,quotes")])?))
}

pub fn overview(token: &str) -> Result<Overview, String> {
    let v = get("me/threads_insights", token, &[("metric", "views,likes,replies")])?;
    let m = parse_metrics(&v);
    let followers = get("me/threads_insights", token, &[("metric", "followers_count")])
        .ok()
        .and_then(|f| f["data"][0]["total_value"]["value"].as_u64());
    Ok(Overview { followers, views: m.views, likes: m.likes, replies: m.replies })
}

pub fn replies(token: &str, id: &str) -> Result<Vec<Reply>, String> {
    let v = get(&format!("{id}/replies"), token, &[("fields", "id,text,username,timestamp,permalink,is_reply_owned_by_me,has_replies"), ("reverse", "true")])?;
    Ok(serde_json::from_value(v["data"].clone()).unwrap_or_default())
}

pub fn mentions(token: &str) -> Result<Vec<Post>, String> {
    let v = get("me/mentions", token, &[("fields", "id,text,username,timestamp,permalink"), ("limit", "20")])?;
    Ok(serde_json::from_value(v["data"].clone()).unwrap_or_default())
}

// ───────────────────────── Đăng ─────────────────────────

/// Đăng bài chữ (hoặc trả lời nếu có `reply_to`). Trả về id bài đã đăng.
pub fn publish_text(token: &str, text: &str, reply_to: Option<&str>) -> Result<String, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("Chưa có nội dung".into());
    }
    if text.chars().count() > 500 {
        return Err("Threads giới hạn 500 ký tự".into());
    }
    let mut form = vec![("media_type", "TEXT"), ("text", text), ("access_token", token)];
    if let Some(r) = reply_to {
        form.push(("reply_to_id", r));
    }
    let c = post_form("me/threads", &form)?;
    let cid = c["id"].as_str().ok_or("Threads không tạo được bài")?.to_string();
    let p = post_form("me/threads_publish", &[("creation_id", cid.as_str()), ("access_token", token)])?;
    Ok(p["id"].as_str().unwrap_or(&cid).to_string())
}

// ───────────────────────── Kho bí mật ─────────────────────────

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub mod secret {
    const SERVICE: &str = "Mini-Asi";
    pub fn set(key: &str, val: &str, _dir: &std::path::Path) -> Result<(), String> {
        keyring::Entry::new(SERVICE, key).and_then(|e| e.set_password(val)).map_err(|e| format!("Không lưu được vào Keychain: {e}"))
    }
    pub fn get(key: &str, _dir: &std::path::Path) -> Option<String> {
        keyring::Entry::new(SERVICE, key).ok()?.get_password().ok()
    }
    pub fn delete(key: &str, _dir: &std::path::Path) {
        if let Ok(e) = keyring::Entry::new(SERVICE, key) {
            let _ = e.delete_credential();
        }
    }
}

/// Linux: file riêng chỉ chủ máy đọc được (chmod 600)
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub mod secret {
    use std::path::Path;
    fn file(dir: &Path, key: &str) -> std::path::PathBuf {
        dir.join("secrets").join(key)
    }
    pub fn set(key: &str, val: &str, dir: &Path) -> Result<(), String> {
        let p = file(dir, key);
        std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(&p, val).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }
    pub fn get(key: &str, dir: &Path) -> Option<String> {
        std::fs::read_to_string(file(dir, key)).ok().filter(|s| !s.is_empty())
    }
    pub fn delete(key: &str, dir: &Path) {
        let _ = std::fs::remove_file(file(dir, key));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oauth_helpers() {
        let u = auth_url("123", "https://localhost/cb", "xyz");
        assert!(u.starts_with("https://threads.net/oauth/authorize?client_id=123&redirect_uri=https%3A%2F%2Flocalhost%2Fcb&scope=threads_basic%2C"));
        assert!(u.ends_with("&response_type=code&state=xyz"));
        assert_eq!(extract_code("https://localhost/cb?code=AQB%2Fx_y&state=xyz#_").as_deref(), Some("AQB/x_y"));
        assert_eq!(extract_code("AQBabc#_").as_deref(), Some("AQBabc"));
        assert_eq!(extract_code("https://localhost/cb?error=access_denied"), None);
    }

    #[test]
    fn metrics_both_shapes() {
        let v: Value = serde_json::from_str(
            r#"{"data":[{"name":"views","period":"lifetime","values":[{"value":120},{"value":30}]},
                       {"name":"likes","total_value":{"value":9}},{"name":"replies","values":[{"value":4}]},
                       {"name":"reposts","total_value":{"value":1}}]}"#,
        )
        .unwrap();
        let m = parse_metrics(&v);
        assert_eq!((m.views, m.likes, m.replies, m.reposts, m.quotes), (150, 9, 4, 1, 0));
    }

    #[test]
    fn posts_json() {
        let v: Value = serde_json::from_str(r#"[{"id":"1","text":"Xin chào","timestamp":"2026-11-05T02:00:00+0000","media_type":"TEXT_POST"},{"id":"2","media_type":"REPOST_FACADE"}]"#).unwrap();
        let p: Vec<Post> = serde_json::from_value(v).unwrap();
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].text.as_deref(), Some("Xin chào"));
    }
}
