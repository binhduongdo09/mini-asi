//! Cầu nối tới app Lời nhắc (Reminders) và Ghi chú (Notes) của macOS qua JavaScript for Automation.
//! Lần đầu macOS sẽ hỏi: "Mini-Asi muốn điều khiển Lời nhắc / Ghi chú" → bấm OK.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct RemoteReminder {
    pub id: String,
    pub title: String,
    pub due: Option<i64>,
    pub done: bool,
    pub modified: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RemoteNote {
    pub id: String,
    pub text: String,
    pub modified: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Op {
    pub op: &'static str, // create | update | delete
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub done: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub html: Option<String>,
}

impl Op {
    pub fn new(op: &'static str) -> Self {
        Self { op, key: None, id: None, title: None, due: None, done: None, html: None }
    }
}

#[cfg(target_os = "macos")]
fn jxa(script: &str, args: &[String]) -> Result<String, String> {
    let o = std::process::Command::new("osascript")
        .args(["-l", "JavaScript", "-e", script])
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if o.status.success() {
        Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
    } else {
        let err = String::from_utf8_lossy(&o.stderr).to_string();
        Err(if err.contains("-1743") || err.contains("Not authorized") {
            "Chưa cho phép Mini-Asi điều khiển app này — vào Cài đặt hệ thống › Quyền riêng tư › Tự động hoá".into()
        } else {
            format!("Lỗi khi nói chuyện với app Apple: {}", err.trim())
        })
    }
}

#[cfg(not(target_os = "macos"))]
fn jxa(_script: &str, _args: &[String]) -> Result<String, String> {
    Err("Đồng bộ với app Apple chỉ có trên macOS".into())
}

const REM_PULL: &str = r#"
function run(argv) {
  const app = Application('Reminders');
  let ls = app.lists.whose({ name: argv[0] });
  let list;
  if (ls.length === 0) { list = app.List({ name: argv[0] }); app.lists.push(list); return '[]'; }
  list = ls[0];
  const rs = list.reminders;
  const ids = rs.id(), names = rs.name(), dues = rs.dueDate(), done = rs.completed(), mods = rs.modificationDate();
  const out = [];
  for (let i = 0; i < ids.length; i++) {
    out.push({ id: ids[i], title: names[i] || '', due: dues[i] ? dues[i].getTime() : null, done: !!done[i], modified: mods[i] ? mods[i].getTime() : 0 });
  }
  return JSON.stringify(out);
}
"#;

const REM_APPLY: &str = r#"
function run(argv) {
  const app = Application('Reminders');
  const list = app.lists.whose({ name: argv[0] })[0];
  const ops = JSON.parse(argv[1]);
  const res = {};
  for (const o of ops) {
    try {
      if (o.op === 'create') {
        const p = { name: o.title };
        if (o.due) p.dueDate = new Date(o.due);
        const r = app.Reminder(p);
        list.reminders.push(r);
        let rid = null;
        try { rid = r.id(); } catch (e) {}
        if (!rid) { const m = list.reminders.whose({ name: o.title }); if (m.length) rid = m[m.length - 1].id(); }
        if (o.done) { try { (rid ? list.reminders.byId(rid) : r).completed = true; } catch (e) {} }
        if (rid) res[o.key] = rid; else res['!' + o.key] = 'không lấy được id';
      } else {
        const r = list.reminders.byId(o.id);
        if (o.op === 'delete') { app.delete(r); continue; }
        r.name = o.title;
        if (o.due) r.dueDate = new Date(o.due); else { try { r.dueDate = null; } catch (e) {} }
        r.completed = !!o.done;
      }
    } catch (e) { res['!' + (o.key || o.id)] = String(e); }
  }
  return JSON.stringify(res);
}
"#;

const NOTES_PULL: &str = r#"
function folderOf(app, name) {
  const fs = app.folders.whose({ name: name });
  if (fs.length > 0) return fs[0];
  const f = app.Folder({ name: name });
  app.defaultAccount().folders.push(f);
  return null;
}
function run(argv) {
  const app = Application('Notes');
  const f = folderOf(app, argv[0]);
  if (!f) return '[]';
  const ns = f.notes;
  const ids = ns.id(), texts = ns.plaintext(), mods = ns.modificationDate();
  const out = [];
  for (let i = 0; i < ids.length; i++) out.push({ id: ids[i], text: texts[i] || '', modified: mods[i] ? mods[i].getTime() : 0 });
  return JSON.stringify(out);
}
"#;

const NOTES_APPLY: &str = r#"
function run(argv) {
  const app = Application('Notes');
  const f = app.folders.whose({ name: argv[0] })[0];
  const ops = JSON.parse(argv[1]);
  const res = {};
  for (const o of ops) {
    try {
      if (o.op === 'create') { const n = app.Note({ body: o.html }); f.notes.push(n); res[o.key] = n.id(); }
      else if (o.op === 'delete') { app.delete(app.notes.byId(o.id)); }
      else { app.notes.byId(o.id).body = o.html; }
    } catch (e) { res['!' + (o.key || o.id)] = String(e); }
  }
  return JSON.stringify(res);
}
"#;

pub fn reminders_pull(list: &str) -> Result<Vec<RemoteReminder>, String> {
    let s = jxa(REM_PULL, &[list.to_string()])?;
    serde_json::from_str(&s).map_err(|e| format!("Dữ liệu Lời nhắc lạ: {e}"))
}

pub fn reminders_apply(list: &str, ops: &[Op]) -> Result<HashMap<String, String>, String> {
    if ops.is_empty() {
        return Ok(HashMap::new());
    }
    let s = jxa(REM_APPLY, &[list.to_string(), serde_json::to_string(ops).unwrap()])?;
    serde_json::from_str(&s).map_err(|e| e.to_string())
}

pub fn notes_pull(folder: &str) -> Result<Vec<RemoteNote>, String> {
    let s = jxa(NOTES_PULL, &[folder.to_string()])?;
    serde_json::from_str(&s).map_err(|e| format!("Dữ liệu Ghi chú lạ: {e}"))
}

pub fn notes_apply(folder: &str, ops: &[Op]) -> Result<HashMap<String, String>, String> {
    if ops.is_empty() {
        return Ok(HashMap::new());
    }
    let s = jxa(NOTES_APPLY, &[folder.to_string(), serde_json::to_string(ops).unwrap()])?;
    serde_json::from_str(&s).map_err(|e| e.to_string())
}

/// Ghi chú dạng chữ → HTML cho app Ghi chú (dòng đầu là tiêu đề)
pub fn note_html(text: &str) -> String {
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let mut lines = text.lines();
    let first = lines.next().unwrap_or("");
    let mut h = format!("<div><h1>{}</h1></div>", esc(first));
    for l in lines {
        if l.trim().is_empty() {
            h.push_str("<div><br></div>");
        } else {
            h.push_str(&format!("<div>{}</div>", esc(l)));
        }
    }
    h
}

#[cfg(test)]
mod tests {
    #[test]
    fn html() {
        assert_eq!(super::note_html("Tiêu đề\n<b>x</b>\n\ny"), "<div><h1>Tiêu đề</h1></div><div>&lt;b&gt;x&lt;/b&gt;</div><div><br></div><div>y</div>");
    }
}
