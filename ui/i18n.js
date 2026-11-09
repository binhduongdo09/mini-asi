/* Ngôn ngữ giao diện (Cài đặt › Chung › Ngôn ngữ): tiếng Việt (gốc) hoặc English.
 *
 * Giao diện viết bằng tiếng Việt. Khi chọn English, bộ dịch này theo dõi DOM (MutationObserver) và thay từng đoạn chữ
 * / thuộc tính title · placeholder · aria-label bằng bản dịch trong `i18n-en.js`:
 *   - `exact`: câu gốc → câu dịch (khớp nguyên văn, bỏ khoảng trắng thừa)
 *   - `patterns`: câu có chỗ trống "{}" (số, tên…) → câu dịch; phần trong chỗ trống được dịch tiếp nếu có trong `exact`
 * Nội dung của người dùng (việc, ghi chú, chat, clipboard, biên bản…) đánh dấu `translate="no"` → không bao giờ bị dịch.
 * Đổi ngôn ngữ → tải lại cửa sổ (ngôn ngữ nhớ trong localStorage để vẽ đúng ngay từ đầu).
 */
(function () {
  "use strict";
  const KEY = "asi-ui-lang";
  let lang = "vi";
  try { lang = localStorage.getItem(KEY) || "vi"; } catch (_) {}
  const D = window.I18N_EN || { exact: {}, patterns: [] };
  const norm = (s) => s.replace(/\s+/g, " ").trim();
  const exact = new Map();
  for (const [k, v] of Object.entries(D.exact || {})) exact.set(norm(k), v);
  const esc = (s) => s.replace(/[.*+?^$()|[\]\\]/g, "\\$&");
  const VN = /[àáạảãâầấậẩẫăằắặẳẵèéẹẻẽêềếệểễìíịỉĩòóọỏõôồốộổỗơờớợởỡùúụủũưừứựửữỳýỵỷỹđ]/i;
  const patterns = (D.patterns || []).map(([k, v]) => {
    const key = norm(k), parts = key.split("{}");
    return { re: new RegExp("^" + parts.map(esc).join("(.+?)") + "$", "s"), out: v, lead: key.startsWith("{}"), lit: parts.join("").length };
  }).sort((a, b) => (a.lead - b.lead) || (b.lit - a.lit)); // mở đầu bằng chữ cố định trước, rồi mẫu dài (cụ thể) trước
  const cache = new Map();

  // Phần trong chỗ trống: có trong `exact` → dịch; bắt đầu bằng số ("18 phút trước") → dịch tiếp; còn lại giữ nguyên
  // (thường là tên việc, tên mã… của người dùng).
  function fill(g) {
    const k = norm(g);
    if (exact.has(k)) return exact.get(k);
    if (/^[\d+\-−]/.test(k)) return t(k);
    return g;
  }

  /** Dịch một câu (trả về nguyên câu nếu không có bản dịch) */
  function t(s) {
    if (lang !== "en" || !s) return s;
    const k = norm(s);
    if (!k || !VN.test(k) && !exact.has(k)) return s; // không có chữ Việt → giữ nguyên
    if (cache.has(k)) return cache.get(k);
    let r = exact.get(k);
    // "45 phút · 3 việc · có ghi âm": dịch từng phần
    if (r == null && k.includes(" · ")) {
      const segs = k.split(" · "), out = segs.map(t);
      if (out.some((x, i) => x !== segs[i])) r = out.join(" · ");
    }
    if (r == null) {
      for (const p of patterns) {
        const m = k.match(p.re);
        if (!m) continue;
        // mẫu "{} việc" không được nuốt "Danh sách việc": phần đầu còn chữ Việt chưa dịch được → bỏ qua mẫu
        if (p.lead && VN.test(m[1]) && fill(m[1]) === m[1]) continue;
        let i = 0;
        r = p.out.replace(/\{\}/g, () => fill(m[++i] ?? ""));
        break;
      }
    }
    if (r == null) r = s;
    cache.set(k, r);
    return r;
  }

  const SKIP = new Set(["SCRIPT", "STYLE", "TEXTAREA", "INPUT", "CODE", "PRE"]);
  function skipped(el) {
    for (let e = el; e && e !== document.body; e = e.parentElement) {
      if (SKIP.has(e.tagName) || e.isContentEditable || e.getAttribute("translate") === "no") return true;
    }
    return false;
  }
  function doText(n) {
    const v = n.nodeValue;
    if (!v || !/\S/.test(v) || !n.parentElement || skipped(n.parentElement)) return;
    const out = t(v);
    if (out !== v) {
      const lead = v.match(/^\s*/)[0], trail = v.match(/\s*$/)[0];
      const nv = lead + out + trail;
      if (nv !== v) n.nodeValue = nv;
    }
  }
  const ATTRS = ["title", "placeholder", "aria-label", "data-tip"];
  function doAttrs(el) {
    if (!el.getAttribute || el.getAttribute("translate") === "no" || (el.parentElement && skipped(el.parentElement))) return;
    for (const a of ATTRS) {
      const v = el.getAttribute && el.getAttribute(a);
      if (v) { const out = t(v); if (out !== v) el.setAttribute(a, out); }
    }
  }
  function walk(root) {
    if (!root) return;
    if (root.nodeType === 3) return doText(root);
    if (root.nodeType !== 1 && root.nodeType !== 9 && root.nodeType !== 11) return;
    if (root.nodeType === 1) { doAttrs(root); if (skipped(root)) return; }
    const w = document.createTreeWalker(root, NodeFilter.SHOW_ELEMENT | NodeFilter.SHOW_TEXT, {
      acceptNode: (n) => {
        if (n.nodeType !== 1) return NodeFilter.FILTER_ACCEPT;
        if (n.getAttribute("translate") === "no") return NodeFilter.FILTER_REJECT;
        if (SKIP.has(n.tagName)) { doAttrs(n); return NodeFilter.FILTER_REJECT; } // ô nhập: dịch placeholder, không đụng nội dung
        return NodeFilter.FILTER_ACCEPT;
      },
    });
    for (let n = w.nextNode(); n; n = w.nextNode()) { if (n.nodeType === 3) doText(n); else doAttrs(n); }
  }
  let obs = null;
  function start() {
    if (lang !== "en" || obs) return;
    document.documentElement.lang = "en";
    if (document.title) document.title = t(document.title);
    walk(document.body);
    obs = new MutationObserver((muts) => {
      for (const m of muts) {
        if (m.type === "characterData") doText(m.target);
        else if (m.type === "attributes") doAttrs(m.target);
        else for (const n of m.addedNodes) walk(n);
      }
    });
    obs.observe(document.body, { childList: true, subtree: true, characterData: true, attributes: true, attributeFilter: ATTRS });
  }
  if (document.body) start(); else document.addEventListener("DOMContentLoaded", start, { once: true });

  window.I18N = {
    get lang() { return lang; },
    t,
    /** Gọi khi đọc được cài đặt: khác ngôn ngữ đang hiển thị → lưu và tải lại cửa sổ */
    setLang(l) {
      l = l === "en" ? "en" : "vi";
      if (l === lang) return false;
      try { localStorage.setItem(KEY, l); } catch (_) {}
      location.reload();
      return true;
    },
  };
})();
