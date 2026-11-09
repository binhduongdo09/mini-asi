/* ───────── Mini-Asi — tiện ích dùng chung cho cửa sổ tai thỏ (index.html) và cửa sổ Cài đặt (settings.html) ─────────
 * - Cầu nối Tauri: `api(lệnh, tham số)` = invoke, `on(sự kiện, cb)` = listen. Mở bằng trình duyệt → tự nạp dev-mock.js.
 * - Định dạng thời gian / dung lượng tiếng Việt, escape HTML, theo dõi bộ gõ đang ghép chữ (composition).
 * Dùng: `const C = await window.AsiCommon;`
 */
window.AsiCommon = (async function () {
  "use strict";
  if (!window.__TAURI__) {
    await new Promise((ok) => {
      const s = document.createElement("script");
      s.src = "dev-mock.js";
      s.onload = ok;
      document.head.appendChild(s);
    });
    document.body.classList.add("mock");
  }
  const T = window.__TAURI__;
  // Gửi kèm lệch múi giờ để backend hiểu "9h" theo đúng giờ người dùng đang thấy
  const api = (cmd, args = {}) => T.core.invoke(cmd, { offset: -new Date().getTimezoneOffset(), ...args });
  const on = (ev, cb) => T.event.listen(ev, (e) => cb(e.payload));

  const $ = (s, r = document) => r.querySelector(s);
  const $$ = (s, r = document) => [...r.querySelectorAll(s)];
  const esc = (s) => String(s ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);

  // Bộ gõ tiếng Việt ghép chữ qua "composition": khi đang ghép thì không bắt Enter/Esc, không vẽ lại ô đang gõ
  let composing = false;
  document.addEventListener("compositionstart", () => (composing = true), true);
  document.addEventListener("compositionend", () => setTimeout(() => (composing = false), 0), true);
  const isComposing = (e) => composing || (e && (e.isComposing || e.keyCode === 229));
  const typing = () => {
    const a = document.activeElement;
    return composing || !!(a && (a.tagName === "INPUT" || a.tagName === "TEXTAREA" || a.isContentEditable));
  };
  // Tắt tự sửa chính tả của macOS — nó hay "sửa" tiếng Việt không dấu thành tiếng Anh
  document.addEventListener("focusin", (e) => {
    const el = e.target;
    if (el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable)) {
      for (const [k, v] of [["spellcheck", "false"], ["autocorrect", "off"], ["autocapitalize", "off"], ["autocomplete", "off"]]) el.setAttribute(k, v);
    }
  }, true);

  const WD = ["CN", "T2", "T3", "T4", "T5", "T6", "T7"];
  const WDN = ["Chủ nhật", "Thứ 2", "Thứ 3", "Thứ 4", "Thứ 5", "Thứ 6", "Thứ 7"];
  const pad = (n) => String(n).padStart(2, "0");
  const hm = (d) => `${pad(d.getHours())}:${pad(d.getMinutes())}`;
  const dayKey = (d) => `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`;
  /** Hạn việc ngắn gọn: "trễ 2 giờ", "15 phút nữa", "14:30", "Mai 09:00", "T6 10:00", "12/11 08:00" */
  function fmtDue(ms, now = Date.now()) {
    if (ms == null) return "";
    const d = new Date(ms);
    const diff = ms - now;
    if (diff < 0) {
      const m = Math.round(-diff / 60000);
      if (m < 60) return `trễ ${Math.max(m, 1)} phút`;
      if (m < 1440) return `trễ ${Math.round(m / 60)} giờ`;
      return `trễ ${Math.round(m / 1440)} ngày`;
    }
    if (dayKey(d) === dayKey(new Date(now))) return diff < 3600000 ? `${Math.max(1, Math.round(diff / 60000))} phút nữa` : hm(d);
    if (dayKey(d) === dayKey(new Date(now + 86400000))) return `Mai ${hm(d)}`;
    if (diff < 6 * 86400000) return `${WD[d.getDay()]} ${hm(d)}`;
    return `${pad(d.getDate())}/${pad(d.getMonth() + 1)} ${hm(d)}`;
  }
  function fmtFull(ms) {
    const d = new Date(ms);
    return `${WDN[d.getDay()]}, ${pad(d.getDate())}/${pad(d.getMonth() + 1)} · ${hm(d)}`;
  }
  function fmtAgo(ms, now = Date.now()) {
    if (!ms) return "chưa có";
    const s = Math.max(0, Math.round((now - ms) / 1000));
    if (s < 45) return "vừa xong";
    const m = Math.round(s / 60);
    if (m < 60) return `${m} phút trước`;
    const h = Math.round(m / 60);
    if (h < 24) return `${h} giờ trước`;
    return `${Math.round(h / 24)} ngày trước`;
  }
  function bytes(n) {
    const u = ["B", "KB", "MB", "GB", "TB"];
    let i = 0, v = n || 0;
    while (v >= 1000 && i < u.length - 1) { v /= 1000; i++; }
    return i === 0 ? `${Math.round(v)} B` : `${v.toFixed(v >= 100 ? 0 : 1)} ${u[i]}`;
  }
  const mins = (m) => (m >= 60 ? `${Math.floor(m / 60)} giờ${m % 60 ? ` ${m % 60} phút` : ""}` : `${m} phút`);
  const shortPath = (p) => String(p).replace(/^\/(Users|home)\/[^/]+/, "~");

  /** Thông báo nhỏ nổi ở đáy khung (một lúc chỉ 1 cái) */
  function toastIn(host) {
    return (msg) => {
      const old = host.querySelector(":scope > .toast");
      if (old) old.remove();
      const t = document.createElement("div");
      t.className = "toast";
      t.textContent = msg;
      host.appendChild(t);
      setTimeout(() => t.classList.add("out"), 2300);
      setTimeout(() => t.remove(), 2600);
    };
  }

  return { api, on, $, $$, esc, isComposing, typing, get composing() { return composing; }, pad, hm, WDN, fmtDue, fmtFull, fmtAgo, bytes, mins, shortPath, toastIn, I: window.ICON };
})();
