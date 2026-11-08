/* ───────────────────────── Mini-Asi — logic giao diện ─────────────────────────
 * Trạng thái hòn đảo: collapsed → peek (rê chuột) → expanded (bấm / phím tắt)
 *                     alert (nhắc việc, nghỉ mắt) · drop (kéo file vào) · listen / reply (lệnh giọng nói)
 * Backend Rust lo dữ liệu + thời gian; UI chỉ hiển thị và gọi lệnh (`api(cmd, args)` = Tauri invoke).
 *
 * Bản đồ file (tìm theo dòng "// ───────── <tên> ─────────"):
 *   Cầu nối Tauri · Định dạng · Hình học (kích thước từng trạng thái, 3 kiểu hiển thị) · Trạng thái tổng
 *   (màu, tâm trạng linh vật, thanh thu gọn/chế độ chờ) · Kéo giãn bảng · Tương tác hòn đảo (bong bóng kéo thả)
 *   · Giọng nói (dictate) · Báo động · Kéo thả file · Các tab (mọi view: chat, việc, ghi chú, file, dọn máy,
 *   sức khoẻ, cài đặt) · Cài đặt › Giọng đọc · Vòng cập nhật.
 * File khác: extra.js (Clipboard, Nhật ký, Threads) · vocab.js (Học từ mới) · vnkey.js (bộ gõ Telex/VNI)
 *            · icons.js · dev-mock.js (giả lập backend khi mở index.html bằng trình duyệt).
 * Các file phụ dùng chung tiện ích qua `window.ASI` (khai báo cuối file này).
 */
(async function () {
  "use strict";

  // ───────── Cầu nối Tauri (hoặc bản giả lập khi mở bằng trình duyệt) ─────────
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
  const I = window.ICON;
  // ── Bộ gõ tiếng Việt (Telex/VNI của macOS, EVKey, OpenKey…) ghép chữ qua "composition".
  // Khi đang ghép chữ: không bắt phím Enter/Esc, không vẽ lại ô đang gõ.
  let composing = false;
  document.addEventListener("compositionstart", () => (composing = true), true);
  document.addEventListener("compositionend", () => setTimeout(() => (composing = false), 0), true);
  const isComposing = (e) => composing || e.isComposing || e.keyCode === 229;
  // Người dùng đang gõ trong một ô nhập → hoãn mọi lần vẽ lại toàn bộ
  const typing = () => {
    const a = document.activeElement;
    return composing || (a && (a.tagName === "INPUT" || a.tagName === "TEXTAREA" || a.isContentEditable));
  };
  // Tắt tự sửa chính tả / tự viết hoa của macOS — nó hay "sửa" chữ tiếng Việt không dấu thành tiếng Anh
  document.addEventListener("focusin", (e) => {
    const el = e.target;
    if (el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable)) {
      el.setAttribute("spellcheck", "false");
      el.setAttribute("autocorrect", "off");
      el.setAttribute("autocapitalize", "off");
      el.setAttribute("autocomplete", "off");
    }
  }, true);
  const island = $("#island");
  const root = document.documentElement;

  // ───────── Định dạng ─────────
  const WD = ["CN", "T2", "T3", "T4", "T5", "T6", "T7"];
  const pad = (n) => String(n).padStart(2, "0");
  const hm = (d) => `${pad(d.getHours())}:${pad(d.getMinutes())}`;
  const dayKey = (d) => `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`;
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
    const today = new Date(now);
    const tmr = new Date(now + 86400000);
    if (dayKey(d) === dayKey(today)) return diff < 3600000 ? `${Math.max(1, Math.round(diff / 60000))} phút nữa` : hm(d);
    if (dayKey(d) === dayKey(tmr)) return `Mai ${hm(d)}`;
    if (diff < 6 * 86400000) return `${WD[d.getDay()]} ${hm(d)}`;
    return `${pad(d.getDate())}/${pad(d.getMonth() + 1)} ${hm(d)}`;
  }
  function fmtFull(ms) {
    const d = new Date(ms);
    const names = ["Chủ nhật", "Thứ 2", "Thứ 3", "Thứ 4", "Thứ 5", "Thứ 6", "Thứ 7"];
    return `${names[d.getDay()]}, ${pad(d.getDate())}/${pad(d.getMonth() + 1)} · ${hm(d)}`;
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
    let i = 0;
    let v = n;
    while (v >= 1000 && i < u.length - 1) { v /= 1000; i++; }
    return i === 0 ? `${n} B` : `${v.toFixed(v >= 100 ? 0 : 1)} ${u[i]}`;
  }
  const mins = (m) => (m >= 60 ? `${Math.floor(m / 60)} giờ ${m % 60 ? (m % 60) + " phút" : ""}` : `${m} phút`);
  const shortPath = (p) => String(p).replace(/^\/(Users|home)\/[^/]+/, "~");

  // ───────── Hình học: kích thước từng trạng thái ─────────
  const URL_PARAMS = new URLSearchParams(location.search);
  const IS_SETTINGS_WIN = URL_PARAMS.get("window") === "settings";
  const INIT_SETTINGS_SEC = URL_PARAMS.get("sec") || URL_PARAMS.get("section") || "";
  if (IS_SETTINGS_WIN) document.body.classList.add("settings-win");

  const geo = await api("get_geometry");
  const early = await api("get_settings");
  // Kiểu hiển thị: tai thỏ · viên thuốc nổi · bong bóng tròn
  let DISP = ["notch", "pill", "bubble"].includes(early.display_mode) ? early.display_mode : "notch";
  document.body.classList.add("disp-" + DISP);
  const ON_NOTCH = DISP === "notch" && geo.has_notch;
  let settingsCache = { ...early };
  let WING = 76;
  let BAR = Math.round(ON_NOTCH ? geo.notch_height : DISP === "notch" ? 30 : 34);
  let BASE_W = Math.round(ON_NOTCH ? geo.notch_width + WING * 2 : 210);
  if (!ON_NOTCH) island.classList.add("virtual");
  if (early.bubble_x > 0 || early.bubble_y > 0) document.body.classList.add("floating-pos");

  function applyIconSize(sz) {
    const small = (sz || settingsCache.icon_size || "large") === "small";
    document.body.classList.toggle("icons-sm", small);
    document.body.classList.toggle("icons-lg", !small);
  }
  applyIconSize(early.icon_size);

  const WING_PRESETS = { hidden: 0, xs: 28, s: 56, m: 88, l: 128 };
  function currentWingPx(st = settingsCache) {
    const ws = st.wing_size || "s";
    if (ws === "hidden") return 0;
    if (ws === "custom") return Math.max(0, Math.min(180, Math.round(Number(st.notch_w) || 0)));
    if (ws in WING_PRESETS) return WING_PRESETS[ws];
    return typeof st.notch_w === "number" && st.notch_w > 0 ? Math.max(0, Math.min(180, Math.round(st.notch_w))) : 56;
  }

  const MARGIN_X = 18, MARGIN_B = 20;
  const MAX_W = Math.max(600, Math.round(geo.screen_width - 60));
  const MAX_H = Math.max(420, Math.round((geo.screen_height || 900) - 40));
  const clampW = (w) => Math.round(Math.min(Math.max(w, Math.max(BASE_W + 40, 460)), MAX_W));
  const clampH = (h) => Math.round(Math.min(Math.max(h, 360), MAX_H));
  let peekCam = { active: false, stream: null, lastSnap: "" };
  let showPeekSizer = false;
  let mode = IS_SETTINGS_WIN ? "expanded" : "collapsed";
  let pinned = false;
  let hovering = false;
  let modeTimer = null;
  let dropBefore = null;
  let suppressHoverUntil = 0;
  let lastFrameKey = "";

  const SIZES = {
    collapsed: [BASE_W, BAR],
    peek: [520, BAR + 214],
    alert: [460, BAR + 120],
    drop: [500, BAR + 132],
    snap: [560, BAR + 122],
    expanded: [740, 540],
    listen: [440, BAR + 84],
    reply: [500, BAR + 250],
  };

  // Nhúng Threads vào bảng: đang thử nghiệm (macOS) — mặc định dùng cửa sổ Threads riêng
  const EMBED_OK = geo.platform === "macos" || new URLSearchParams(location.search).has("embed");
  const threadsInline = () => EMBED_OK && DISP !== "bubble" && tab === "threads" && (settingsCache.threads_embed_beta || new URLSearchParams(location.search).has("embed")) && !window.__thApi;

  // Kích thước bảng: theo tab, có thể kéo giãn & được nhớ lại
  function sizeOf(m) {
    if (m === "peek") {
      const baseH = SIZES.peek[1] + (showPeekSizer ? 44 : 0) + (peekCam.active ? 138 : 0);
      return [SIZES.peek[0], baseH];
    }
    if (m !== "expanded") return SIZES[m] || SIZES.collapsed;
    if (threadsInline()) {
      return [clampW(settingsCache.threads_w || Math.max(BASE_W + 120, 470)), clampH(settingsCache.threads_h || Math.min(MAX_H, 860))];
    }
    return [clampW(settingsCache.panel_w || SIZES.expanded[0]), clampH(settingsCache.panel_h || SIZES.expanded[1])];
  }

  function sendFrameIfChanged(w, h, force = false) {
    if (IS_SETTINGS_WIN) return Promise.resolve();
    const rw = Math.round(w + MARGIN_X * 2);
    const rh = Math.round(h + MARGIN_B);
    const k = `${rw}x${rh}`;
    if (!force && k === lastFrameKey) return Promise.resolve();
    lastFrameKey = k;
    return api("set_frame", { width: rw, height: rh });
  }

  function applyNotchGeometry(st = settingsCache, resizeNow = false) {
    WING = currentWingPx(st);
    const defaultBar = Math.round(ON_NOTCH ? geo.notch_height : DISP === "notch" ? 30 : 34);
    const customH = Math.round(Number(st.notch_h) || 0);
    BAR = WING === 0 && ON_NOTCH ? Math.max(18, Math.round(geo.notch_height - 2)) : customH >= 20 ? Math.min(54, customH) : defaultBar;
    BASE_W = DISP === "bubble"
      ? 58
      : ON_NOTCH
        ? Math.round(geo.notch_width + WING * 2)
        : Math.max(90, Math.round(130 + WING * 1.4));
    const hiddenBehindNotch = DISP !== "bubble" && WING <= 2;
    island.classList.toggle("notch-hidden", hiddenBehindNotch);
    root.style.setProperty("--bar-h", (IS_SETTINGS_WIN ? 48 : BAR) + "px");
    root.style.setProperty("--gap-w", (ON_NOTCH && !IS_SETTINGS_WIN ? Math.round(geo.notch_width) : 0) + "px");
    const smIcons = (st.icon_size || "large") === "small";
    SIZES.collapsed = DISP === "bubble" ? [58, 58] : [BASE_W, BAR];
    SIZES.peek = [Math.max(BASE_W + 110, smIcons ? 490 : 532), BAR + (smIcons ? 174 : 218)];
    SIZES.alert = [Math.max(BASE_W + 80, 460), BAR + 120];
    SIZES.drop = [Math.max(BASE_W + 130, 500), BAR + 132];
    SIZES.snap = [Math.max(BASE_W + 250, 560), BAR + 122];
    SIZES.expanded = [Math.max(BASE_W + 150, 740), 540];
    SIZES.listen = [Math.max(BASE_W + 80, 440), BAR + 84];
    SIZES.reply = [Math.max(BASE_W + 140, 500), BAR + 250];
    if (IS_SETTINGS_WIN) return;
    const [cw, ch] = sizeOf(mode);
    root.style.setProperty("--w", cw + "px");
    root.style.setProperty("--h", ch + "px");
    if (resizeNow) {
      sendFrameIfChanged(cw, ch, true);
    }
  }
  applyNotchGeometry(early, !IS_SETTINGS_WIN);

  let freshOpen = true;
  let frameTimer = null;
  async function syncImeInteractive(on, keepFocus) {
    const V = window.VNKey;
    if (!on) {
      if (V) { V.mode = null; V.suspended = false; }
      if (!IS_SETTINGS_WIN) await api("set_interactive", { on: false, keepFocus: keepFocus ?? null });
      updateBar();
      return null;
    }
    const m = await api("set_interactive", { on: true, keepFocus: keepFocus ?? null });
    if (V) {
      V.mode = m || (V.userOn ? V.userOn : null);
      V.suspended = false;
    }
    updateBar();
    return V ? V.mode : m;
  }

  async function openSettingsWindow(section = null) {
    if (IS_SETTINGS_WIN) {
      tab = "settings";
      await viewSettings();
      if (section) scrollToSettingsSection(section);
      return;
    }
    try {
      await api("open_settings_window", { section: section || null });
      if (mode !== "collapsed") setMode("collapsed");
    } catch (err) {
      toast(String(err));
    }
  }

  async function setMode(next, opts = {}) {
    if (IS_SETTINGS_WIN) {
      mode = "expanded";
      tab = "settings";
      island.className = island.className.replace(/mode-\w+/, "mode-expanded");
      updateBar();
      await viewSettings();
      return;
    }
    if (next === mode && !opts.force) return;
    clearTimeout(modeTimer);
    clearTimeout(frameTimer);
    if (next === "collapsed" && mode !== "collapsed") {
      suppressHoverUntil = Date.now() + 500;
    }
    const [w, h] = sizeOf(next);
    const [cw, ch] = sizeOf(mode === next ? "collapsed" : mode);
    const growing = w >= cw || h >= ch;
    const frame = () => sendFrameIfChanged(w, h, !!opts.force);
    if (growing) await frame(); // nở cửa sổ đồng bộ trên main thread trước rồi mới chạy hoạt cảnh
    island.className = island.className.replace(/mode-\w+/, "mode-" + next);
    root.style.setProperty("--w", w + "px");
    root.style.setProperty("--h", h + "px");
    const prev = mode;
    mode = next;
    if (!growing) frameTimer = setTimeout(frame, 420); // dùng frameTimer riêng để hover không bao giờ huỷ lệnh thu cửa sổ
    if (next !== "peek" && peekCam.active) stopPeekCam(false);
    if (next !== "expanded" || tab !== "threads") hideEmbed();
    if (next === "expanded" && prev !== "expanded") {
      freshOpen = true; // lần mở mới → Học từ mới đổi sang 2–3 từ khác
      await syncImeInteractive(true); // chuyển bàn phím hệ thống sang ABC & bật VNKey trước khi focus ô nhập
      renderView();
    } else if (next === "expanded") {
      renderView();
    } else if (prev === "expanded") {
      syncImeInteractive(false);
    }
    if (next === "peek") {
      renderPeek();
      refreshMediaQuick();
    }
    updateBar();
  }

  // ───────── Trạng thái tổng ─────────
  let status = null;
  let tasks = [];
  let tab = IS_SETTINGS_WIN ? "settings" : "all";
  let moodOverride = null;
  let moodTimer = null;

  function flashMood(m, ms = 2200) {
    moodOverride = m;
    clearTimeout(moodTimer);
    moodTimer = setTimeout(() => { moodOverride = null; applyMood(); }, ms);
    applyMood();
  }

  function currentMood() {
    if (moodOverride) return moodOverride;
    if (mode === "drop" || mode === "snap") return "drop";
    if (!status) return "idle";
    // Có việc trễ → Asi tức giận (mắt đỏ), càng trễ càng giận
    // Đang hẹn giờ tập trung: Asi giữ yên (không giận, không nhấp nháy) để bạn không bị phân tâm
    if (status.focus_until) return "focus";
    if (status.lateness.level >= 1 && !status.focus_mode) return "angry";
    if (status.disk && status.disk.level === "critical") return "worried";
    if (status.focus_until) return "focus";
    if (status.session_minutes >= status.break_minutes) return "tired";
    if (status.sleepy) return "sleepy";
    return "idle";
  }
  let angerTimer = null;
  function applyMood() {
    const m = currentMood();
    island.className = island.className.replace(/mood-\w+/, "mood-" + m);
    const lvl = m === "angry" && status ? status.lateness.level : 0;
    island.dataset.anger = String(lvl);
    // Giận nặng (mức 3): cứ ~20 giây lại dậm chân một cái cho chủ nhân để ý
    clearInterval(angerTimer);
    if (lvl >= 3) {
      angerTimer = setInterval(() => {
        if (currentMood() !== "angry" || mode === "expanded") return;
        island.classList.add("stomp");
        setTimeout(() => island.classList.remove("stomp"), 700);
      }, 20000);
    }
  }

  // ───────── Màu sắc hòn đảo ─────────
  // Clean & Clear: Luôn giữ nền đen/xám trung tính như Apple Dynamic Island
  // Chỉ báo màu qua điểm/mắt linh vật hoặc accent bar mỏng ở mép đáy
  function applyColor() {
    const s = status;
    let bg = "#000000", glow = "transparent", accent = null;
    island.classList.remove("pulse");
    const calm = s && (s.focus_mode || s.focus_until);
    if (s && s.disk && s.disk.level !== "ok") accent = "var(--amber)";
    if (s && s.session_minutes >= s.break_minutes && !calm) accent = "var(--blue)";
    if (s && s.lateness.level > 0 && !calm) {
      accent = "var(--red)";
      if (s.lateness.level >= 3) island.classList.add("pulse");
    }
    if (calm) accent = null;
    root.style.setProperty("--bg", bg);
    root.style.setProperty("--bg-x", "#161618");
    root.style.setProperty("--glow", glow);
    const a = $("#accent");
    a.classList.toggle("on", !!accent);
    if (accent) root.style.setProperty("--accent", accent);
  }

  function updateBar() {
    const s = status;
    const rw = $("#rightWing");
    const titleText = IS_SETTINGS_WIN ? "Cài đặt Mini-Asi" : mode === "expanded" ? "Mini-Asi" : "";
    const btEl = $("#barTitle");
    if (btEl.textContent !== titleText) btEl.textContent = titleText;
    if (mode === "expanded") {
      const V = window.VNKey;
      const ime = V && V.mode ? `${V.mode === "vni" ? "VNI" : "Telex"}` : "EN";
      const hasCustomPos = !IS_SETTINGS_WIN && (settingsCache.bubble_x > 0 || settingsCache.bubble_y > 0);
      const expKey = `exp:${IS_SETTINGS_WIN ? "win" : "notch"}:${V && V.mode ? V.mode : "en"}:${hasCustomPos ? 1 : 0}:${pinned ? 1 : 0}`;
      if (rw.dataset.keys === expKey) return;
      rw.dataset.keys = expKey;
      if (IS_SETTINGS_WIN) {
        rw.innerHTML = `<button class="ime-pill ${V && V.mode ? "on" : ""}" data-act="ime" title="Bộ gõ tiếng Việt của Asi — bấm để bật/tắt">${V && V.mode ? "VI · " : ""}${ime}</button>
          <span class="chip green">Cửa sổ độc lập</span>`;
        return;
      }
      rw.innerHTML = `<button class="ime-pill ${V && V.mode ? "on" : ""}" data-act="ime" title="Bộ gõ tiếng Việt của Asi — bấm để bật/tắt">${V && V.mode ? "VI · " : ""}${ime}</button>
        ${hasCustomPos ? `<button class="icon-btn on" data-act="reset-pos" title="Đang ghim vị trí tuỳ chọn — bấm để đặt về vị trí mặc định">${I.location}</button>` : ""}
        <button class="icon-btn ${pinned ? "on" : ""}" data-act="pin" title="Ghim mở popup (không tự đóng khi bấm ra ngoài)">${I.pin}</button>
        <button class="icon-btn" data-act="close" title="Thu gọn (Esc)">${I.x}</button>`;
      return;
    }
    const items = idleItems();
    const showMic = items.includes("mic");
    const micHtml = showMic ? `<button class="wing-mic ${mode === "listen" ? "rec" : ""}" data-act="vmic" title="Ra lệnh cho Asi bằng giọng nói">${I.mic}</button>` : "";
    if (!s) {
      if (rw.dataset.keys !== "nos") { rw.innerHTML = micHtml; rw.dataset.keys = "nos"; }
      return;
    }
    if (DISP === "bubble" && mode === "collapsed") {
      const bKey = `bub:${s.lateness.overdue && !s.focus_mode ? s.lateness.overdue : 0}`;
      if (rw.dataset.keys !== bKey) {
        rw.innerHTML = s.lateness.overdue && !s.focus_mode ? `<span class="b-dot">${s.lateness.overdue}</span>` : "";
        rw.dataset.keys = bKey;
      }
      return;
    }
    // [khoá, màu, biểu tượng, chữ] theo thứ tự ưu tiên; chỉ hiện những thứ bạn chọn trong Cài đặt › Chế độ chờ
    const now = Date.now(), b = [];
    const want = (k) => items.includes(k);
    if (want("focus") && s.focus_until) {
      const left = Math.max(0, s.focus_until * 1000 - now);
      b.push(["focus", "violet", I.target, `${pad(Math.floor(left / 60000))}:${pad(Math.floor((left % 60000) / 1000))}`]);
    }
    if (want("overdue") && s.lateness.overdue > 0 && !s.focus_mode && !s.focus_until) b.push(["overdue", "red", "", String(s.lateness.overdue)]);
    if (want("event") && s.next_event && s.next_event.start - now < 15 * 60000 && s.next_event.start > now) b.push(["event", "blue", I.cal, hm(new Date(s.next_event.start))]);
    if (want("next") && s.next && s.next.due - now < 3600000) b.push(["next", "", I.clock, hm(new Date(s.next.due))]);
    if (want("time")) b.push(["time", "ghost", "", hm(new Date(now))]);
    if (want("tasks") && s.open_count > 0) b.push(["tasks", "ghost", I.check, String(s.open_count)]);
    if (want("notes") && s.note_count > 0) b.push(["notes", "ghost", I.note, String(s.note_count)]);
    if (want("clips") && s.clip_count > 0) b.push(["clips", "ghost", I.clip, String(s.clip_count)]);
    if (want("disk") && s.disk) b.push(["disk", s.disk.level !== "ok" ? "amber" : "ghost", I.disk, shortBytes(s.disk.free_bytes)]);
    const keys = b.map((x) => x[0]).join(",") + (showMic ? "|mic" + (mode === "listen" ? "R" : "") : "");
    if (rw.dataset.keys === keys) {
      // cùng các mục như trước → chỉ đổi chữ tại chỗ, không bao giờ vẽ lại DOM (không nhấp nháy)
      for (const [k, , , t] of b) { const el = rw.querySelector(`[data-k="${k}"] .bt`); if (el && el.textContent !== t) el.textContent = t; }
      return;
    }
    rw.innerHTML = b.map(([k, c, ic, t]) => `<span class="badge steady ${c}" data-k="${k}">${ic}<span class="bt">${t}</span></span>`).join("") + micHtml;
    rw.dataset.keys = keys;
    // Không đủ chỗ (cánh phải cạnh tai thỏ hẹp) → bỏ bớt mục ít ưu tiên ở cuối
    const shown = [...rw.querySelectorAll(".badge")];
    const overflow = () => rw.firstElementChild && rw.firstElementChild.getBoundingClientRect().left < rw.getBoundingClientRect().left - 0.5;
    while (shown.length > 1 && overflow()) { shown.pop().remove(); }
  }
  function shortBytes(n) {
    if (!n && n !== 0) return "";
    const g = n / 1e9;
    return g >= 100 ? `${Math.round(g)}G` : g >= 1 ? `${g.toFixed(g >= 10 ? 0 : 1)}G` : `${Math.round(n / 1e6)}M`;
  }
  const IDLE_DEFAULT = ["focus", "overdue", "event", "next", "tasks", "mic"];
  function idleItems() {
    const v = settingsCache.idle_items || early.idle_items;
    return Array.isArray(v) ? v : IDLE_DEFAULT;
  }

  let mediaQuick = { title: "", artist: "", app: "", playing: false, output_volume: 60, output_muted: false, mic_volume: 80, mic_muted: false };

  function peekTopHtml(s) {
    const main = s.lateness.overdue > 0
      ? `<span class="chip red">${s.lateness.overdue} việc trễ</span><span class="t">${esc((tasks.find((t) => !t.done_at && t.due && t.due < Date.now()) || {}).title || "")}</span>`
      : s.next_event && (!s.next || s.next_event.start < s.next.due)
        ? `<span class="chip blue">${I.cal}${s.next_event.start <= Date.now() ? "Đang diễn ra" : fmtDue(s.next_event.start)}</span><span class="t">${esc(s.next_event.title)}</span>`
      : s.next
        ? `<span class="chip">${I.clock}${fmtDue(s.next.due)}</span><span class="t">${esc(s.next.title)}</span>`
        : `<span class="t">${s.open_count ? `${s.open_count} việc chưa hạn` : "Hôm nay thảnh thơi"}</span>`;
    const disk = s.disk ? `<span>${I.disk}${bytes(s.disk.free_bytes)} trống</span>` : "";
    return `<div class="peek-main">${main}</div>
      <div class="peek-sub">${disk}<span>${I.clock}${mins(s.session_minutes)} liên tục</span>
      ${s.focus_until ? `<span>${I.target}đang tập trung</span>` : ""}</div>`;
  }

  function peekBarHtml() {
    const m = mediaQuick;
    const s = status || {};
    const trackTitle = m.title ? esc(m.title) : (m.app ? esc(m.app) : "Không có nhạc đang phát");
    const trackSub = m.title
      ? esc([m.artist, m.app].filter(Boolean).join(" · ") || "Đang phát")
      : "Bấm ▶ để phát nhạc từ Spotify / Apple Music";
    const micMuted = !!m.mic_muted;
    const camOn = !!peekCam.active;
    const focusOn = !!s.focus_until;
    return `<div class="ios-media-card">
        <div class="ios-art ${m.playing ? "playing" : ""}" title="${esc(m.app || "Media")}">${I.music}</div>
        <div class="peek-track" title="${esc(m.title ? `${m.title} — ${m.artist || m.app}` : "Điều khiển trình phát nhạc/video hệ thống")}">
          <b>${trackTitle}</b>
          <span>${trackSub}</span>
        </div>
        <div class="peek-mbtns">
          <button class="peek-mbtn" data-peek="prev" title="Bài trước">${I.prev}</button>
          <button class="peek-mbtn play" data-peek="play" title="${m.playing ? "Tạm dừng" : "Phát tiếp"}">${m.playing ? I.pause : I.play}</button>
          <button class="peek-mbtn" data-peek="next" title="Bài tiếp theo">${I.next}</button>
          <button class="peek-mbtn ${m.output_muted ? "muted" : ""}" data-peek="mute-out" title="${m.output_muted ? "Loa đang tắt tiếng — bấm để bật" : `Âm lượng loa ${m.output_volume}% — bấm để tắt tiếng`}">${m.output_muted ? I.volOff : I.vol}</button>
        </div>
      </div>
      <div class="ios-grid">
        <button class="ios-tile ${micMuted ? "mic-off" : "mic-on"}" data-peek="mic" title="${micMuted ? "Micro hệ thống đang tắt tiếng — bấm để bật lại" : `Micro đang bật (${m.mic_volume}%) — bấm để tắt tiếng Micro`}">
          <span class="ios-ic">${micMuted ? I.micOff : I.mic}</span>
          <span class="ios-meta"><b>Micro</b><span>${micMuted ? "Đang tắt" : `Đang bật · ${m.mic_volume}%`}</span></span>
        </button>
        <button class="ios-tile ${camOn ? "cam-on" : "cam-off"}" data-peek="cam" title="${camOn ? "Camera đang bật trên tai thỏ — bấm để tắt" : "Bật gương Camera nhanh trên tai thỏ"}">
          <span class="ios-ic">${camOn ? I.camOff : I.cam}</span>
          <span class="ios-meta"><b>Camera</b><span>${camOn ? "Đang soi gương" : "Gương nhanh"}</span></span>
        </button>
        <button class="ios-tile airdrop" data-peek="airdrop" title="Chọn file gửi AirDrop (hoặc kéo thả file vào nửa trái tai thỏ)">
          <span class="ios-ic">${I.airdrop}</span>
          <span class="ios-meta"><b>AirDrop</b><span>Gửi / Thả file</span></span>
        </button>
        <button class="ios-tile ${focusOn ? "focus-on" : "focus-off"}" data-peek="focus" title="${focusOn ? "Đang bật chế độ Tập trung — bấm để dừng" : "Bật nhanh Tập trung 25 phút"}">
          <span class="ios-ic">${I.target}</span>
          <span class="ios-meta"><b>Tập trung</b><span>${focusOn ? "Đang chạy" : "25 phút"}</span></span>
        </button>
      </div>
      <div class="peek-snap-row" title="Chia nhanh cửa sổ đang dùng (hoặc kéo thanh tiêu đề cửa sổ vào tai thỏ)">
        <span class="peek-snap-lbl">Chia màn hình:</span>
        <button class="peek-sbtn" data-peek="snap-left" title="Nửa trái"><i class="snap-pv pv-left"></i><span>Trái</span></button>
        <button class="peek-sbtn" data-peek="snap-right" title="Nửa phải"><i class="snap-pv pv-right"></i><span>Phải</span></button>
        <button class="peek-sbtn accent" data-peek="snap-split2" title="Chia đôi 2 app"><i class="snap-pv pv-split2"></i><span>Đôi</span></button>
        <button class="peek-sbtn" data-peek="snap-max" title="Toàn màn hình"><i class="snap-pv pv-max"></i><span>Toàn màn</span></button>
        <button class="peek-sbtn" data-peek="snap-top" title="Nửa trên"><i class="snap-pv pv-top"></i></button>
        <button class="peek-sbtn" data-peek="snap-bottom" title="Nửa dưới"><i class="snap-pv pv-bottom"></i></button>
        <button class="peek-sbtn" data-peek="snap-center" title="Giữa màn"><i class="snap-pv pv-center"></i></button>
      </div>`;
  }

  function peekSizerHtml() {
    if (!showPeekSizer) return "";
    const curW = currentWingPx(settingsCache);
    return `<div class="peek-size-bar">
      <span class="muted">↔ Cỡ tai thỏ:</span>
      <button class="peek-pill-btn ${curW === 0 ? "on" : ""}" data-peek="wing-preset" data-w="hidden" title="Thu nhỏ hoàn toàn vào sau tai thỏ vật lý">Ẩn sau tai thỏ</button>
      <input type="range" id="peekWingRange" min="0" max="180" step="2" value="${curW}" title="Kéo về 0 để ẩn hoàn toàn sau tai thỏ" />
      <span id="peekWingVal" style="min-width:52px;text-align:right;font-variant-numeric:tabular-nums">${curW === 0 ? "Ẩn kín" : curW + " px"}</span>
    </div>`;
  }

  function renderPeek() {
    const s = status;
    if (!s) return;
    const pk = $("#peek");
    const smIcons = (settingsCache.icon_size || "large") === "small";
    const layoutKey = `${smIcons ? 1 : 0}:${showPeekSizer ? 1 : 0}:${peekCam.active ? 1 : 0}`;
    const topEl = pk.querySelector(".peek-top");
    const barEl = pk.querySelector(".peek-bar");
    if (pk.dataset.layoutKey === layoutKey && topEl && barEl) {
      // Chỉ cập nhật tại chỗ khi nội dung thật sự thay đổi → tuyệt đối không nhấp nháy
      const nextTop = peekTopHtml(s);
      if (topEl.dataset.lastHtml !== nextTop) {
        topEl.dataset.lastHtml = nextTop;
        topEl.innerHTML = nextTop;
      }
      const nextBar = peekBarHtml();
      if (barEl.dataset.lastHtml !== nextBar) {
        barEl.dataset.lastHtml = nextBar;
        barEl.innerHTML = nextBar;
      }
      return;
    }
    pk.dataset.layoutKey = layoutKey;
    const initTop = peekTopHtml(s);
    const initBar = peekBarHtml();
    pk.innerHTML = `<div class="peek-head">
        <div class="peek-top" title="Bấm để mở bảng điều khiển">${initTop}</div>
      </div>
      <div class="peek-bar">${initBar}</div>
      ${peekSizerHtml()}
      ${peekCam.active ? `<div class="peek-cam-wrap" id="peekCamWrap">
        <video id="peekCamVideo" autoplay playsinline muted></video>
        <div class="peek-cam-bar">
          <span class="peek-cam-badge">Camera đang bật</span>
          <div class="row" style="gap:4px">
            <button class="peek-btn" data-peek="cam-snap" title="Chụp ảnh lưu vào Clipboard">Chụp</button>
            <button class="peek-btn danger" data-peek="cam" title="Tắt Camera">Đóng</button>
          </div>
        </div>
      </div>` : ""}`;
    const newTop = pk.querySelector(".peek-top");
    const newBar = pk.querySelector(".peek-bar");
    if (newTop) newTop.dataset.lastHtml = initTop;
    if (newBar) newBar.dataset.lastHtml = initBar;
    const rng = $("#peekWingRange");
    if (rng) {
      rng.addEventListener("input", (e) => {
        const val = Number(e.target.value);
        settingsCache.wing_size = val <= 2 ? "hidden" : "custom";
        settingsCache.notch_w = val;
        const lbl = $("#peekWingVal");
        if (lbl) lbl.textContent = val <= 2 ? "Ẩn kín" : val + " px";
        applyNotchGeometry(settingsCache, false);
        updateBar();
      });
      rng.addEventListener("change", async () => {
        applyNotchGeometry(settingsCache, true);
        await api("save_settings", { settings: settingsCache });
        toast(currentWingPx() === 0 ? "Đã ẩn hoàn toàn sau tai thỏ — rê chuột lên mép trên để mở" : "Đã lưu kích thước tai thỏ");
      });
    }
    if (peekCam.active && peekCam.stream) {
      const v = $("#peekCamVideo");
      if (v && v.srcObject !== peekCam.stream) v.srcObject = peekCam.stream;
    }
  }

  async function refreshMediaQuick() {
    try {
      const m = await api("media_status");
      if (m) {
        mediaQuick = m;
        const barEl = $("#peek .peek-bar");
        if (barEl && mode === "peek") {
          const nextBar = peekBarHtml();
          if (barEl.dataset.lastHtml !== nextBar) {
            barEl.dataset.lastHtml = nextBar;
            barEl.innerHTML = nextBar;
          }
        }
      }
    } catch (_) {}
  }

  function stopPeekCam(resize = true) {
    if (peekCam.stream) {
      try { peekCam.stream.getTracks().forEach((t) => t.stop()); } catch (_) {}
    }
    peekCam.active = false;
    peekCam.stream = null;
    const wrap = $("#peekCamWrap");
    if (wrap) wrap.remove();
    if (resize && mode === "peek") setMode("peek", { force: true });
  }

  async function togglePeekCam() {
    if (peekCam.active) {
      stopPeekCam(true);
      toast("📷 Đã tắt Camera");
      return;
    }
    const ok = await api("ensure_camera_perm").catch(() => true);
    if (!ok) {
      toast("Chưa cấp quyền Camera — hãy bật quyền Camera cho Mini-Asi trong Cài đặt hệ thống");
      return;
    }
    if (!navigator.mediaDevices || !navigator.mediaDevices.getUserMedia) {
      toast("WebView chưa hỗ trợ mở luồng Camera trực tiếp");
      return;
    }
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ video: { width: { ideal: 640 }, height: { ideal: 360 }, facingMode: "user" }, audio: false });
      peekCam.active = true;
      peekCam.stream = stream;
      await setMode("peek", { force: true });
      const v = $("#peekCamVideo");
      if (v) v.srcObject = stream;
    } catch (err) {
      toast("Không mở được Camera: " + String(err.message || err));
    }
  }

  async function handlePeekAction(act) {
    if (["prev", "play", "next", "mute-out"].includes(act)) {
      const cmd = act === "mute-out" ? "mute_out" : act;
      mediaQuick = await api("media_control", { action: cmd });
      const barEl = $("#peek .peek-bar");
      if (barEl) barEl.innerHTML = peekBarHtml();
      if (act === "mute-out") toast(mediaQuick.output_muted ? "🔇 Đã tắt tiếng loa" : `🔊 Đã bật loa (${mediaQuick.output_volume}%)`);
      return;
    }
    if (act === "mic") {
      mediaQuick = await api("toggle_mic_mute");
      const barEl = $("#peek .peek-bar");
      if (barEl) barEl.innerHTML = peekBarHtml();
      toast(mediaQuick.mic_muted ? "🔇 Đã tắt tiếng Micro hệ thống" : `🎙 Đã bật Micro (${mediaQuick.mic_volume}%)`);
      return;
    }
    if (act === "cam") {
      await togglePeekCam();
      return;
    }
    if (act === "cam-snap") {
      const v = $("#peekCamVideo");
      if (!v || !v.videoWidth) return;
      const cv = document.createElement("canvas");
      cv.width = v.videoWidth;
      cv.height = v.videoHeight;
      const ctx = cv.getContext("2d");
      ctx.translate(cv.width, 0);
      ctx.scale(-1, 1);
      ctx.drawImage(v, 0, 0);
      const dataUrl = cv.toDataURL("image/png");
      peekCam.lastSnap = dataUrl;
      settingsCache.widget_images = { ...(settingsCache.widget_images || {}), camera: dataUrl };
      api("save_settings", { settings: settingsCache });
      await api("clip_copy_image", { dataUrl });
      confetti(16);
      toast("📸 Đã chụp vào Clipboard & cập nhật lên Widget Camera!");
      return;
    }
    if (act === "focus") {
      if (status && status.focus_until) {
        await api("focus_stop");
        toast("Đã dừng hẹn giờ Tập trung");
      } else {
        await api("focus_start", { minutes: 25 });
        toast("🎯 Đã bật Tập trung 25 phút");
      }
      status = await api("get_status");
      renderPeek();
      updateBar();
      return;
    }
    if (act === "icon-size") {
      settingsCache.icon_size = (settingsCache.icon_size || "large") === "small" ? "large" : "small";
      applyIconSize(settingsCache.icon_size);
      applyNotchGeometry(settingsCache);
      await api("save_settings", { settings: settingsCache });
      await setMode("peek", { force: true });
      toast(settingsCache.icon_size === "small" ? "Đã chuyển sang Icon iOS Nhỏ" : "Đã chuyển sang Icon iOS To");
      return;
    }
    if (act === "toggle-sizer") {
      showPeekSizer = !showPeekSizer;
      await setMode("peek", { force: true });
      return;
    }
    if (act === "wing-preset") {
      settingsCache.wing_size = "hidden";
      settingsCache.notch_w = 0;
      applyNotchGeometry(settingsCache, true);
      await api("save_settings", { settings: settingsCache });
      renderPeek();
      updateBar();
      toast("Đã ẩn hoàn toàn sau tai thỏ — rê chuột vào tai thỏ để mở");
      return;
    }
    if (act && act.startsWith("snap-")) {
      const snapAct = act.slice(5);
      try {
        const msg = await api("window_snap", { action: snapAct, pid: null, prevPid: null });
        flashMood("happy", 1400);
        toast("🪟 " + msg);
        setTimeout(() => { if (mode === "peek") setMode("collapsed"); }, 500);
      } catch (err) {
        toast("⚠ " + String(err));
      }
      return;
    }
    if (act === "airdrop") {
      suppressHoverUntil = Date.now() + 2000;
      const paths = await api("pick_files");
      if (!paths || !paths.length) return;
      try {
        await api("file_airdrop", { paths });
        setMode("collapsed");
      } catch (err) {
        toast(String(err));
      }
    }
  }

  // ───────── Kéo giãn bảng & chỉnh kích thước Tai thỏ ─────────
  (function grips() {
    for (const cls of ["grip", "grip-corner l", "grip-corner r"]) {
      const g = document.createElement("div");
      g.className = cls;
      g.title = "Kéo để đổi kích thước";
      island.appendChild(g);
    }
    for (const side of ["l", "r"]) {
      const ne = document.createElement("div");
      ne.className = `notch-edge ${side}`;
      ne.title = "Kéo cạnh tai thỏ để chỉnh độ rộng (kéo sát vào giữa để ẩn hoàn toàn sau tai thỏ)";
      island.appendChild(ne);
    }
    let drag = null, notchDrag = null, raf = 0;
    island.addEventListener("pointerdown", (e) => {
      const ne = e.target.closest(".notch-edge");
      if (ne && (mode === "collapsed" || mode === "peek") && DISP !== "bubble") {
        e.preventDefault();
        e.stopPropagation();
        ne.setPointerCapture(e.pointerId);
        const startWing = currentWingPx();
        notchDrag = { ne, x: e.screenX, side: ne.classList.contains("l") ? -1 : 1, startWing, lastWing: startWing };
        island.classList.add("resizing");
        const [, ch] = sizeOf(mode);
        api("set_frame", { width: Math.max(600, (geo.notch_width || 185) + 380) + MARGIN_X * 2, height: ch + MARGIN_B });
        return;
      }
      const g = e.target.closest(".grip, .grip-corner");
      if (!g || mode !== "expanded") return;
      e.preventDefault();
      g.setPointerCapture(e.pointerId);
      const [w, h] = sizeOf("expanded");
      drag = { g, x: e.screenX, y: e.screenY, w, h, corner: g.classList.contains("grip-corner"), side: g.classList.contains("l") ? -1 : 1 };
      island.classList.add("resizing");
      // nở sẵn cửa sổ tới cỡ tối đa để kéo mượt, thả tay mới thu về đúng cỡ
      api("set_frame", { width: MAX_W + MARGIN_X * 2, height: MAX_H + MARGIN_B });
    });
    island.addEventListener("pointermove", (e) => {
      if (notchDrag) {
        const delta = notchDrag.side * (e.screenX - notchDrag.x);
        const nw = Math.max(0, Math.min(180, Math.round(notchDrag.startWing + delta)));
        notchDrag.lastWing = nw;
        settingsCache.notch_w = nw;
        settingsCache.wing_size = nw <= 2 ? "hidden" : "custom";
        cancelAnimationFrame(raf);
        raf = requestAnimationFrame(() => {
          applyNotchGeometry(settingsCache, false);
          updateBar();
        });
        return;
      }
      if (!drag) return;
      const nh = clampH(drag.h + (e.screenY - drag.y));
      const nw = drag.corner ? clampW(drag.w + 2 * drag.side * (e.screenX - drag.x)) : drag.w;
      drag.nw = nw; drag.nh = nh;
      cancelAnimationFrame(raf);
      raf = requestAnimationFrame(() => {
        root.style.setProperty("--w", nw + "px");
        root.style.setProperty("--h", nh + "px");
        if (tab === "threads" && embedShown) placeEmbed();
      });
    });
    const end = async () => {
      if (notchDrag) {
        const nw = notchDrag.lastWing;
        notchDrag = null;
        island.classList.remove("resizing");
        settingsCache.notch_w = nw;
        settingsCache.wing_size = nw <= 2 ? "hidden" : "custom";
        applyNotchGeometry(settingsCache, true);
        updateBar();
        await api("save_settings", { settings: settingsCache });
        toast(nw <= 2 ? "Đã ẩn hoàn toàn sau tai thỏ — rê chuột lên mép trên để mở lại" : `Độ rộng cánh tai thỏ: ${nw} px`);
        return;
      }
      if (!drag) return;
      const { nw, nh } = drag;
      drag = null;
      island.classList.remove("resizing");
      if (nw && nh) {
        if (threadsInline()) { settingsCache.threads_w = nw; settingsCache.threads_h = nh; }
        else { settingsCache.panel_w = nw; settingsCache.panel_h = nh; }
        api("save_settings", { settings: settingsCache });
      }
      const [w, h] = sizeOf("expanded");
      await api("set_frame", { width: w + MARGIN_X * 2, height: h + MARGIN_B });
      if (tab === "threads" && embedShown) setTimeout(placeEmbed, 60);
    };
    island.addEventListener("pointerup", end);
    island.addEventListener("pointercancel", end);
    island.addEventListener("dblclick", (e) => {
      if (e.target.closest(".notch-edge")) {
        settingsCache.notch_w = currentWingPx() <= 2 ? 56 : 0;
        settingsCache.wing_size = settingsCache.notch_w === 0 ? "hidden" : "s";
        applyNotchGeometry(settingsCache, true);
        updateBar();
        api("save_settings", { settings: settingsCache });
        toast(settingsCache.notch_w === 0 ? "Đã ẩn hoàn toàn sau tai thỏ" : "Đã khôi phục kích thước tai thỏ");
        return;
      }
      if (!e.target.closest(".grip, .grip-corner")) return;
      // bấm đúp vào tay kéo → về kích thước mặc định
      if (threadsInline()) { settingsCache.threads_w = 0; settingsCache.threads_h = 0; } else { settingsCache.panel_w = 0; settingsCache.panel_h = 0; }
      api("save_settings", { settings: settingsCache });
      setMode("expanded", { force: true });
    });
  })();
  on("perm-prompt", (low) => {
    if (IS_SETTINGS_WIN) return;
    if (low && mode !== "collapsed") setMode("collapsed");
    else if (!low) {
      const [w, h] = sizeOf(mode);
      sendFrameIfChanged(w, h, true);
    }
  });
  on("ime-warn", () => toast("Không chuyển được sang bàn phím ABC — hãy bật thêm bàn phím ABC trong Cài đặt hệ thống › Bàn phím › Nguồn nhập để Asi tự gõ tiếng Việt"));
  on("window-blur", () => {
    if (IS_SETTINGS_WIN || mode !== "expanded") return;
    if (pinned) {
      // Đang ghim mở popup mà bấm sang app khác → trả lại bộ gõ hệ thống cho app kia
      syncImeInteractive(false, true);
      return;
    }
    setTimeout(() => { if (!hovering && !document.hasFocus() && !voice.active) setMode("collapsed"); }, 350);
  });

  // ───────── Tương tác hòn đảo ─────────
  function scheduleHoverSync(inside) {
    if (IS_SETTINGS_WIN) return;
    hovering = inside;
    clearTimeout(modeTimer);
    if (island.classList.contains("resizing") || island.classList.contains("dragging")) return;
    if (inside && mode === "collapsed" && DISP !== "bubble" && Date.now() >= suppressHoverUntil) {
      modeTimer = setTimeout(() => {
        if (hovering && mode === "collapsed" && Date.now() >= suppressHoverUntil) setMode("peek");
      }, 130);
    } else if (!inside && mode === "peek") {
      modeTimer = setTimeout(() => {
        if (!hovering && mode === "peek") setMode("collapsed");
      }, peekCam.active ? 1600 : 360);
    }
  }
  on("hover", (inside) => scheduleHoverSync(inside));
  island.addEventListener("mouseenter", () => { if (document.body.classList.contains("mock")) scheduleHoverSync(true); else hovering = true; });
  island.addEventListener("mouseleave", () => { if (document.body.classList.contains("mock")) scheduleHoverSync(false); });
  // Kéo & ghim vị trí bất kỳ trên màn hình (áp dụng cho cả Tai thỏ, Viên thuốc, Bong bóng và bảng Popup mở rộng)
  let justDragged = false;
  async function resetPosition() {
    await api("move_bubble", { x: 0, y: 0, save: true });
    settingsCache.bubble_x = 0;
    settingsCache.bubble_y = 0;
    document.body.classList.remove("floating-pos");
    updateBar();
    if (tab === "settings" && mode === "expanded") viewSettings();
    toast("Đã đặt về vị trí mặc định trên màn hình");
  }
  if (!IS_SETTINGS_WIN) {
    let d = null, raf = 0;
    island.addEventListener("pointerdown", (e) => {
      if (e.button !== 0) return;
      // Bỏ qua khi bấm vào các nút chức năng, tay kéo giãn, thẻ widget hoặc ô nhập liệu
      if (e.target.closest("[data-act], [data-widget-id], [data-lib-id], .grip, .grip-corner, .notch-edge, button, input, select, textarea, a, label")) return;
      // Khi đang mở bảng (expanded / alert / reply / listen): kéo ở thanh tiêu đề trên cùng (#bar) để di chuyển popup
      if (mode !== "collapsed" && mode !== "peek" && !e.target.closest("#bar")) return;
      const curW = sizeOf(mode)[0] + MARGIN_X * 2;
      const sx0 = geo.screen_x || 0, sy0 = geo.screen_y || 0, sw0 = geo.screen_width || 1440;
      const hasCustom = (settingsCache.bubble_x > 0 || settingsCache.bubble_y > 0);
      const wx = hasCustom
        ? (DISP === "bubble" ? settingsCache.bubble_x : settingsCache.bubble_x - curW / 2)
        : (DISP === "bubble" ? sx0 + sw0 - 140 : sx0 + (sw0 - curW) / 2);
      const wy = hasCustom
        ? settingsCache.bubble_y
        : (DISP === "bubble" ? sy0 + 90 : DISP === "pill" ? sy0 + (geo.notch_height || 30) + 6 : sy0);
      d = { sx: e.screenX, sy: e.screenY, wx, wy, dockedNotch: DISP === "notch" && !hasCustom, moved: false, pid: e.pointerId };
    });
    island.addEventListener("pointermove", (e) => {
      if (!d) return;
      const dx = e.screenX - d.sx, dy = e.screenY - d.sy;
      const minMove = d.dockedNotch ? 16 : 6;
      if (!d.moved && Math.hypot(dx, dy) < minMove) return;
      if (!d.moved) {
        try { island.setPointerCapture(d.pid); } catch (_) {} // chỉ bắt chuột khi đã thật sự kéo, để cú bấm thường vẫn mở được bảng
        clearTimeout(modeTimer);
      }
      d.moved = true;
      island.classList.add("dragging");
      document.body.classList.add("floating-pos");
      cancelAnimationFrame(raf);
      const x = Math.max(0, d.wx + dx), y = Math.max(0, d.wy + dy);
      raf = requestAnimationFrame(() => api("move_bubble", { x, y, save: false }));
      d.last = [x, y];
    });
    const endDrag = () => {
      if (d && d.moved && d.last) {
        const [nx, ny] = d.last;
        const curW = sizeOf(mode)[0] + MARGIN_X * 2;
        const scrCenter = (geo.screen_x || 0) + (geo.screen_width || 1440) / 2;
        const nearTopNotch = DISP === "notch" && ny < 36 && Math.abs((nx + curW / 2) - scrCenter) < 60;
        api("move_bubble", { x: nx, y: ny, save: true });
        if (nearTopNotch) {
          settingsCache.bubble_x = 0;
          settingsCache.bubble_y = 0;
          document.body.classList.remove("floating-pos");
        } else {
          settingsCache.bubble_x = DISP === "bubble" ? nx : Math.round(nx + curW / 2);
          settingsCache.bubble_y = ny;
          document.body.classList.add("floating-pos");
        }
        updateBar();
        if (tab === "threads" && embedShown) setTimeout(placeEmbed, 40);
        if (tab === "settings" && mode === "expanded") viewSettings();
        justDragged = true;
        setTimeout(() => (justDragged = false), 80);
      }
      island.classList.remove("dragging");
      d = null;
    };
    island.addEventListener("pointerup", endDrag);
    island.addEventListener("pointercancel", endDrag);
  }
  $("#bar").addEventListener("click", async (e) => {
    if (justDragged) return;
    const act = e.target.closest("[data-act]")?.dataset.act;
    if (act === "reset-pos") { e.stopPropagation(); resetPosition(); return; }
    if (act === "pin") { pinned = !pinned; toast(pinned ? "Đã ghim mở popup — không tự đóng khi bấm ra ngoài" : "Đã bỏ ghim mở popup"); updateBar(); return; }
    if (act === "vmic") { e.stopPropagation(); voiceCommand(); return; }
    if (act === "ime") {
      e.stopPropagation();
      const V = window.VNKey;
      const cur = V && V.mode ? V.mode : null;
      let nextEngine;
      if (e.altKey) {
        nextEngine = cur === "vni" ? "telex" : "vni";
      } else if (cur) {
        V.lastPref = cur;
        nextEngine = "off";
      } else {
        nextEngine = (V && V.lastPref) || (settingsCache.viet_engine === "vni" ? "vni" : "telex");
      }
      settingsCache.viet_engine = nextEngine;
      if (V) V.userOn = nextEngine === "off" ? null : nextEngine;
      await api("save_settings", { settings: settingsCache });
      const activeMode = await syncImeInteractive(true);
      api("threads_embed_ime", { ime: activeMode });
      toast(activeMode ? `⌨️ Bộ gõ tiếng Việt: ${activeMode === "vni" ? "VNI" : "Telex"} (Giữ ⌥ bấm để đổi Telex/VNI)` : "⌨️ Đã chuyển sang gõ tiếng Anh (EN)");
      if (tab === "settings" && mode === "expanded") viewSettings();
      return;
    }
    if (act === "close") { if (IS_SETTINGS_WIN) window.close(); else setMode("collapsed"); return; }
    if (!IS_SETTINGS_WIN && (mode === "collapsed" || mode === "peek")) setMode("expanded");
  });
  $("#peek").addEventListener("click", async (e) => {
    const btn = e.target.closest("[data-peek]");
    if (btn) {
      e.stopPropagation();
      await handlePeekAction(btn.dataset.peek);
      return;
    }
    if (e.target.closest(".peek-cam-wrap") || e.target.closest(".peek-bar")) return;
    setMode("expanded");
  });
  window.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && !isComposing(e)) {
      if (IS_SETTINGS_WIN) window.close();
      else if (mode !== "collapsed") setMode("collapsed");
    }
  });
  // Chỉ thu gọn khi người dùng thật sự rời đi (chuột ở ngoài tai thỏ) — tránh bị đóng khi
  // bộ gõ tiếng Việt / bảng chọn emoji / đổi ngôn ngữ bàn phím làm cửa sổ mất focus thoáng qua
  window.addEventListener("blur", () => {
    if (IS_SETTINGS_WIN || mode !== "expanded") return;
    if (pinned) {
      syncImeInteractive(false, true);
      return;
    }
    modeTimer = setTimeout(() => {
      if (!document.hasFocus() && !hovering && !voice.active) setMode("collapsed");
    }, 450);
  });
  window.addEventListener("focus", () => {
    clearTimeout(modeTimer);
    if (mode === "expanded") {
      syncImeInteractive(true, false);
    }
  });
  on("toggle", () => {
    if (IS_SETTINGS_WIN) return;
    mode === "expanded" ? setMode("collapsed") : setMode("expanded");
  });
  on("open-tab", (id) => {
    if (IS_SETTINGS_WIN) return;
    if (id === "settings") {
      openSettingsWindow();
      return;
    }
    tab = { open: tab, clean: "clean", clips: "clips", journal: "journal", threads: "threads", all: "all" }[id] || tab;
    setMode("expanded", { force: true });
  });

  // Mắt chớp & liếc ngẫu nhiên — linh vật "sống"
  (function life() {
    const face = $("#face");
    const blink = () => {
      if (!["sleepy", "happy", "love", "focus"].includes(currentMood())) {
        face.classList.add("blink");
        setTimeout(() => face.classList.remove("blink"), 140);
      }
      setTimeout(blink, 2500 + Math.random() * 4000);
    };
    const look = () => {
      const x = currentMood() === "idle" ? Math.round((Math.random() - 0.5) * 4) : 0;
      face.style.transform = `translateX(${x}px)`;
      setTimeout(look, 1800 + Math.random() * 3500);
    };
    setTimeout(blink, 1500);
    setInterval(() => {
      const b = document.getElementById("bigFace");
      if (b && Math.random() < 0.5) { b.classList.add("blink"); setTimeout(() => b.classList.remove("blink"), 150); }
    }, 2600);
    setTimeout(look, 2500);
  })();

  function confetti(n = 26) {
    const box = $("#confetti");
    const colors = ["#ffd60a", "#32d74b", "#4fb3ff", "#ff6b81", "#bf8cff"];
    for (let i = 0; i < n; i++) {
      const c = document.createElement("i");
      c.style.left = 30 + Math.random() * 30 + "px";
      c.style.top = BAR / 2 + "px";
      c.style.background = colors[i % colors.length];
      c.style.setProperty("--dx", (Math.random() * 160 - 40) + "px");
      c.style.setProperty("--dy", (Math.random() * 90 + 10) + "px");
      c.style.setProperty("--rot", Math.random() * 540 + "deg");
      box.appendChild(c);
      setTimeout(() => c.remove(), 1200);
    }
  }
  function toast(msg) {
    const t = document.createElement("div");
    t.className = "toast";
    t.textContent = msg;
    island.appendChild(t);
    setTimeout(() => t.remove(), 2600);
  }

  // ───────── Giọng nói (tiếng Việt / tiếng Anh) ─────────
  settingsCache = await api("get_settings");
  const EXTRA_VIEWS = {};
  const voice = { active: null };
  function langPill() {
    const en = (settingsCache.voice_lang || "vi-VN").startsWith("en");
    return `<button class="lang-pill" data-voice-lang title="Đổi ngôn ngữ giọng nói">${en ? "EN" : "VI"}</button>`;
  }
  function micBtn(id) {
    return `<button class="mic" id="${id}" title="Nói để nhập (bấm lần nữa để dừng)">${I.mic}<span class="bars"><i></i><i></i><i></i><i></i></span></button>`;
  }
  // Bấm mic: bắt đầu thu; bấm lần nữa (hoặc im lặng 2 giây) → nhận dạng → onText(chuỗi)
  // opts.lang: ngôn ngữ nhận dạng (mặc định theo Cài đặt) · opts.keep: giữ bản ghi để nghe lại (học phát âm)
  async function dictate(btn, onText, opts = {}) {
    if (voice.active) { await stopDictation(); return; }
    try {
      await api("voice_start");
    } catch (err) {
      toast(String(err));
      return;
    }
    voice.active = { btn, onText, lang: opts.lang, keep: !!opts.keep };
    btn.classList.add("rec");
    moodOverride = "listen";
    applyMood();
  }
  async function stopDictation() {
    const v = voice.active;
    if (!v) return;
    voice.active = null;
    v.btn.classList.remove("rec");
    v.btn.classList.add("busy");
    moodOverride = "focus";
    applyMood();
    try {
      const text = await api("voice_stop", { lang: v.lang || settingsCache.voice_lang, keep: v.keep });
      if (text) { v.onText(text); flashMood("happy", 1200); }
    } catch (err) {
      toast(String(err));
      flashMood("worried", 1600);
    } finally {
      v.btn.classList.remove("busy");
      if (moodOverride === "focus") { moodOverride = null; applyMood(); }
      // lệnh giọng nói không nhận được gì → tự thu gọn
      setTimeout(() => { if (mode === "listen" && !voice.active) setMode("collapsed"); }, 2500);
    }
  }
  on("voice-level", ([level]) => {
    if (!voice.active) return;
    $$("i", voice.active.btn).forEach((b, k) => (b.style.height = 3 + Math.round(level * (8 + k * 3) * (0.6 + Math.random() * 0.6)) + "px"));
    island.style.setProperty("--lvl", level.toFixed(2));
  });
  on("voice-ended", () => stopDictation());
  document.addEventListener("click", async (e) => {
    if (!e.target.closest("[data-voice-lang]")) return;
    settingsCache.voice_lang = (settingsCache.voice_lang || "vi-VN").startsWith("en") ? "vi-VN" : "en-US";
    await api("save_settings", { settings: settingsCache });
    $$("[data-voice-lang]").forEach((b) => (b.textContent = settingsCache.voice_lang.startsWith("en") ? "EN" : "VI"));
    toast(settingsCache.voice_lang.startsWith("en") ? "🎙 Giọng nói: English" : "🎙 Giọng nói: Tiếng Việt");
  });

  // ───────── Báo động ─────────
  const alertQueue = [];
  let alertTimer = null;
  function showAlert(a) {
    alertQueue.push(a);
    if (mode !== "alert") nextAlert();
  }
  function nextAlert() {
    clearTimeout(alertTimer);
    const a = alertQueue.shift();
    if (!a) { setMode(mode === "alert" ? "collapsed" : mode); return; }
    const el = $("#alert");
    if (a.kind === "reminder") {
      el.innerHTML = `<h3>⏰ ${esc(a.task.title)}</h3><p>Đến giờ rồi!</p>
        <div class="row">
          <button class="btn green" data-a="done">${I.check}Xong</button>
          <button class="btn" data-a="s10">10 phút</button>
          <button class="btn" data-a="s60">1 giờ</button>
          <button class="btn" data-a="stm">Mai</button>
          <button class="btn ghost" data-a="dismiss">Đóng</button>
        </div>`;
      flashMood("alarm", 6000);
      island.classList.add("shake");
      setTimeout(() => island.classList.remove("shake"), 1600);
    } else if (a.kind === "break") {
      el.innerHTML = `<h3>${I.eye} Bạn đã dùng máy ${a.minutes} phút liên tục</h3>
        <p>Nhìn ra xa khoảng 6 mét trong 20 giây để mắt thư giãn nhé.</p>
        <div class="row">
          <button class="btn blue" data-a="rest">Nghỉ 20 giây</button>
          <button class="btn" data-a="later">Để sau 10 phút</button>
          <button class="btn ghost" data-a="dismiss">Đóng</button>
        </div>`;
      flashMood("tired", 8000);
    }
    if (a.kind === "event") {
      const mins = Math.max(1, Math.round((a.event.start - Date.now()) / 60000));
      el.innerHTML = `<h3>${I.cal} ${esc(a.event.title)}</h3><p>${mins} phút nữa · ${hm(new Date(a.event.start))}${a.event.location ? " · " + esc(a.event.location) : ""}</p>
        <div class="row"><button class="btn blue" data-a="dismiss">Đã biết</button></div>`;
      flashMood("alarm", 3000);
    }
    if (a.kind === "journal") {
      el.innerHTML = `<h3>${I.book} Hôm nay bạn thấy thế nào?</h3><p>3 câu hỏi nhỏ, trả lời 2–3 câu thôi — Asi ghi nhật ký giúp bạn.</p>
        <div class="row"><button class="btn primary" data-a="jwrite">${I.note}Viết</button><button class="btn" data-a="jtalk">${I.mic}Nói</button>
        <button class="btn" data-a="jlater">30 phút nữa</button><button class="btn ghost" data-a="jskip">Bỏ qua hôm nay</button></div>`;
      flashMood("love", 3000);
    }
    el.dataset.id = a.task ? a.task.id : "";
    el.dataset.kind = a.kind;
    if (mode === "expanded") { /* đang mở bảng: không chen ngang, chỉ báo nhỏ */
      toast(a.kind === "reminder" ? `${I.clock} ${a.task.title}` : `${I.eye} Đến giờ nghỉ mắt`);
      return;
    }
    setMode("alert", { force: true });
    alertTimer = setTimeout(nextAlert, 45000);
  }
  $("#alert").addEventListener("click", async (e) => {
    const a = e.target.closest("[data-a]")?.dataset.a;
    if (!a) return;
    const el = $("#alert");
    const id = Number(el.dataset.id);
    if (a === "done") { await api("toggle_task", { id }); confetti(); flashMood("happy"); }
    if (a === "s10") await api("snooze_task", { id, minutes: 10 });
    if (a === "s60") await api("snooze_task", { id, minutes: 60 });
    if (a === "stm") await api("snooze_task", { id, minutes: -1 });
    if (a === "later") await api("health_snooze", { minutes: 10 });
    if (a === "rest") { restCountdown(); return; }
    if (a === "jwrite" || a === "jtalk") {
      alertQueue.length = 0;
      tab = "journal";
      window.__asiJournalTalk = a === "jtalk";
      setMode("expanded", { force: true });
      return;
    }
    if (a === "jlater") await api("journal_snooze", { minutes: 30 });
    if (a === "jskip") await api("journal_skip_today");
    await refreshTasks();
    nextAlert();
  });
  function restCountdown() {
    clearTimeout(alertTimer);
    let n = 20;
    moodOverride = "sleepy";
    applyMood();
    const el = $("#alert");
    const tick = () => {
      el.innerHTML = `<div class="row"><div class="countdown">${n}</div><div><h3>Nhìn xa nào…</h3><p style="margin:0">Thả lỏng vai, chớp mắt vài lần</p></div></div>`;
      if (n-- <= 0) {
        api("health_break_done");
        moodOverride = null;
        flashMood("happy");
        confetti(14);
        nextAlert();
        return;
      }
      alertTimer = setTimeout(tick, 1000);
    };
    tick();
  }
  on("reminder", (task) => { refreshTasks(); showAlert({ kind: "reminder", task }); });
  on("break", (minutes) => showAlert({ kind: "break", minutes }));
  on("event-soon", (event) => showAlert({ kind: "event", event }));
  on("journal-ask", () => showAlert({ kind: "journal" }));
  let syncDirty = false;
  on("synced", () => {
    if (typing()) { syncDirty = true; return; } // đang gõ → cập nhật sau
    refreshTasks();
    if (mode === "expanded" && (tab === "notes" || tab === "tasks")) renderView();
  });
  document.addEventListener("focusout", () => setTimeout(() => {
    if (syncDirty && !typing()) { syncDirty = false; refreshTasks(); if (mode === "expanded" && tab === "notes") renderView(); }
  }, 300));
  on("water", () => { flashMood("happy", 2500); if (mode === "expanded") toast("💧 Uống ngụm nước nhé"); });
  on("focus-done", () => { confetti(); flashMood("love", 4000); });

  // ───────── Kéo thả file (nửa trái = AirDrop ngay · nửa phải = Khay File / Nén / PDF) ─────────
  let dropZone = "airdrop";
  let dropPoll = null;
  function setDropZone(z) {
    dropZone = z === "files" ? "files" : "airdrop";
    $("#dropAirdrop")?.classList.toggle("active", dropZone === "airdrop");
    $("#dropTray")?.classList.toggle("active", dropZone === "files");
  }
  $("#dropLayer")?.addEventListener("dragover", (e) => {
    e.preventDefault();
    setDropZone(e.clientX < window.innerWidth / 2 ? "airdrop" : "files");
  });
  $("#dropLayer")?.addEventListener("pointermove", (e) => {
    setDropZone(e.clientX < window.innerWidth / 2 ? "airdrop" : "files");
  });
  on("tauri://drag-over", (p) => {
    const pos = p && p.position;
    if (pos && typeof pos.x === "number") {
      const dpr = window.devicePixelRatio || 1;
      const half = window.innerWidth / 2;
      // Hỗ trợ cả toạ độ logical và physical trên màn Retina
      const lx = pos.x > window.innerWidth ? pos.x / dpr : pos.x;
      setDropZone(lx < half ? "airdrop" : "files");
    }
  });
  on("tauri://drag-enter", () => {
    if (mode === "drop") return;
    dropBefore = mode;
    setDropZone("airdrop");
    if (mode !== "expanded") setMode("drop");
    clearInterval(dropPoll);
    dropPoll = setInterval(async () => {
      if (mode !== "drop") { clearInterval(dropPoll); return; }
      try {
        const side = await api("cursor_side");
        setDropZone(side === "right" ? "files" : "airdrop");
      } catch (_) {}
    }, 90);
  });
  on("tauri://drag-leave", () => {
    clearInterval(dropPoll);
    if (mode === "drop") setMode(dropBefore === "expanded" ? "expanded" : "collapsed");
  });
  on("tauri://drag-drop", async (p) => {
    clearInterval(dropPoll);
    const paths = (p && p.paths) || [];
    if (!paths.length) return;
    let side = dropZone;
    if (mode === "drop") {
      try {
        const s = await api("cursor_side");
        side = s === "right" ? "files" : "airdrop";
      } catch (_) {}
    }
    if (mode === "drop" && side === "airdrop") {
      await setMode("collapsed");
      try {
        await api("file_airdrop", { paths });
      } catch (err) {
        await addFiles(paths);
        tab = "files";
        await setMode("expanded", { force: true });
        toast(String(err));
      }
      return;
    }
    await addFiles(paths);
    tab = "files";
    setMode("expanded", { force: true });
  });

  // ───────── Kéo cửa sổ ứng dụng vào tai thỏ để chia màn hình (Window Snap) ─────────
  let activeSnapAction = null;
  let lastSnapMeta = { pid: 0, prev_pid: 0, app_name: "", prev_name: "" };
  function highlightSnapZone(action) {
    activeSnapAction = action;
    $$("#snapGrid .snap-card").forEach((c) => c.classList.toggle("active", c.dataset.snap === action));
  }
  function pickSnapZoneAt(x, y) {
    const cards = $$("#snapGrid .snap-card");
    if (!cards.length) return activeSnapAction || "max";
    if (typeof x === "number" && typeof y === "number" && x > 0 && y > 0) {
      const hit = document.elementFromPoint(x, y)?.closest?.(".snap-card");
      if (hit && hit.dataset.snap) return hit.dataset.snap;
      // Khi con trỏ còn ở mép trên thanh tai thỏ (#bar) hoặc khe giữa các ô: dóng theo cột ngang X gần nhất
      let best = null;
      let bestDist = Infinity;
      for (const c of cards) {
        const r = c.getBoundingClientRect();
        if (r.width <= 0) continue;
        const cx = r.left + r.width / 2;
        const dx = x >= r.left && x <= r.right ? 0 : Math.min(Math.abs(x - r.left), Math.abs(x - r.right), Math.abs(x - cx));
        if (dx < bestDist) {
          bestDist = dx;
          best = c.dataset.snap;
        }
      }
      if (best) return best;
    }
    return activeSnapAction || "max";
  }
  $("#snapLayer")?.addEventListener("pointermove", (e) => {
    const card = e.target.closest(".snap-card");
    if (card) highlightSnapZone(card.dataset.snap);
  });
  $("#snapLayer")?.addEventListener("click", async (e) => {
    const card = e.target.closest(".snap-card");
    if (!card) return;
    const action = card.dataset.snap;
    await setMode("collapsed");
    try {
      await api("window_snap", {
        action,
        pid: lastSnapMeta.pid || null,
        prevPid: lastSnapMeta.prev_pid || null,
      });
      flashMood("happy", 1500);
    } catch (err) {
      toast("⚠ " + String(err));
    }
  });
  on("window-drag", async (ev) => {
    if (!ev) return;
    if (ev.pid) lastSnapMeta = ev;
    if (ev.phase === "move") {
      if (mode !== "snap" && mode !== "expanded") {
        await setMode("snap");
      }
      const titleEl = $("#snapTitle");
      const subEl = $("#snapSub");
      if (titleEl) {
        titleEl.textContent = ev.app_name
          ? `🪟 Chia cửa sổ: ${ev.app_name}`
          : "🪟 Thả cửa sổ vào ô để chia màn hình";
      }
      if (subEl) {
        subEl.textContent = ev.prev_name
          ? `Chia đôi 2 app sẽ ghép với ${ev.prev_name}`
          : "Di chuột vào bố cục muốn ghép rồi thả chuột";
      }
      highlightSnapZone(pickSnapZoneAt(ev.x, ev.y));
    } else if (ev.phase === "leave") {
      highlightSnapZone(null);
      if (mode === "snap") setMode("collapsed");
    } else if (ev.phase === "drop") {
      const action = pickSnapZoneAt(ev.x, ev.y);
      highlightSnapZone(null);
      if (mode === "snap") await setMode("collapsed");
      if (action) {
        try {
          await api("window_snap", {
            action,
            pid: ev.pid || lastSnapMeta.pid || null,
            prevPid: ev.prev_pid || lastSnapMeta.prev_pid || null,
          });
          flashMood("happy", 1600);
        } catch (err) {
          toast("⚠ " + String(err));
        }
      }
    }
  });

  // ───────── Các tab (bảng màu chuẩn iOS App Icon) ─────────
  const TABS = [
    ["all", "Widget", I.grid, "#64d2ff"],
    ["home", "Asi AI", I.mic, "#0a84ff"],
    ["tasks", "Việc", I.tasks, "#30d158"],
    ["notes", "Ghi chú", I.note, "#ff9f0a"],
    ["clips", "Clipboard", I.clip, "#5ac8fa"],
    ["journal", "Nhật ký", I.book, "#ff375f"],
    ["words", "Từ mới", I.words, "#ff9500"],
    ["threads", "Threads", I.at, "#5e5ce6"],
    ["files", "File", I.file, "#007aff"],
    ["clean", "Dọn máy", I.broom, "#ff9f0a"],
    ["health", "Sức khoẻ", I.heart, "#ff2d55"],
    ["settings", "Cài đặt ↗", I.gear, "#8e8e93"],
  ];
  function visibleTabs() {
    if (IS_SETTINGS_WIN) {
      return [
        ["sec-disp", "Giao diện & Tai thỏ", I.sparkle, "#0a84ff"],
        ["sec-ai", "Apple Intelligence & Giọng", I.mic, "#bf5af2"],
        ["sec-ime", "Bộ gõ Tiếng Việt", I.note, "#30d158"],
        ["sec-perm", "Quyền hệ thống", I.check, "#ff9f0a"],
        ["sec-sync", "Sao lưu & Đồng bộ", I.sync, "#5ac8fa"],
      ];
    }
    const vis = settingsCache.visible_tabs && settingsCache.visible_tabs.length ? settingsCache.visible_tabs : ["home", "tasks", "notes", "clips", "journal", "words", "threads"];
    const list = [TABS[0]]; // "all" (Bảng Widget) luôn đứng đầu
    for (const item of TABS.slice(1)) {
      const [id] = item;
      if (id === "home" || vis.includes(id)) list.push(item);
    }
    if (!list.some(([id]) => id === tab) && tab !== "all" && tab !== "settings") {
      const cur = TABS.find(([id]) => id === tab);
      if (cur) list.push(cur);
    }
    if (!list.some(([id]) => id === "settings")) list.push(TABS.find(([id]) => id === "settings"));
    return list;
  }
  function hasTabDot(id) {
    return !!(
      (id === "tasks" && status && status.lateness.overdue) ||
      (id === "threads" && status && status.threads_unread) ||
      (id === "settings" && status && status.update_version)
    );
  }
  function renderTabs() {
    const tabsEl = $("#tabs");
    if (!tabsEl) return;
    const list = visibleTabs();
    const idsKey = list.map(([id]) => id).join(",");
    if (tabsEl.dataset.idsKey === idsKey && tabsEl.children.length === list.length) {
      // Cập nhật tại chỗ class .active và chấm báo .dot — tuyệt đối không xoá/vẽ lại DOM thanh tab
      for (const btn of tabsEl.querySelectorAll(".tab")) {
        const id = btn.dataset.tab;
        btn.classList.toggle("active", tab === id);
        const needDot = hasTabDot(id);
        const dotEl = btn.querySelector(".dot");
        if (needDot && !dotEl) {
          const d = document.createElement("span");
          d.className = "dot";
          btn.appendChild(d);
        } else if (!needDot && dotEl) {
          dotEl.remove();
        }
      }
      return;
    }
    tabsEl.dataset.idsKey = idsKey;
    tabsEl.innerHTML = list.map(([id, name, ic, c]) =>
      `<button class="tab ${tab === id ? "active" : ""}" data-tab="${id}" style="--tc:${c}" title="${name}"><span class="tab-ic">${ic}</span><span class="lbl">${name}</span>${hasTabDot(id) ? '<span class="dot"></span>' : ""}</button>`
    ).join("");
  }
  function scrollToSettingsSection(secId) {
    const el = document.getElementById(secId);
    if (el) el.scrollIntoView({ behavior: "smooth", block: "start" });
  }
  $("#tabs").addEventListener("click", (e) => {
    const t = e.target.closest("[data-tab]");
    if (!t) return;
    const id = t.dataset.tab;
    if (IS_SETTINGS_WIN && id.startsWith("sec-")) {
      $$("#tabs .tab").forEach((b) => b.classList.toggle("active", b === t));
      scrollToSettingsSection(id);
      return;
    }
    switchTab(id);
  });
  function switchTab(t) {
    if (t === "settings" && !IS_SETTINGS_WIN) {
      openSettingsWindow();
      return;
    }
    const before = sizeOf("expanded").join("x");
    tab = t;
    if (!IS_SETTINGS_WIN && mode === "expanded" && sizeOf("expanded").join("x") !== before) setMode("expanded", { force: true });
    else renderView();
  }
  // Khung Threads nhúng: chỉ hiện ở tab Threads khi bảng đang mở
  let embedShown = false;
  function hideEmbed() {
    if (embedShown) { embedShown = false; api("threads_embed", { show: false, x: 0, y: 0, w: 0, h: 0 }); }
  }
  function placeEmbed(url) {
    const slot = $("#thSlot");
    if (!slot || mode !== "expanded" || tab !== "threads") return;
    const r = slot.getBoundingClientRect();
    embedShown = true;
    const V = window.VNKey;
    return api("threads_embed", { show: true, x: r.left, y: r.top, w: r.width, h: r.height, ime: V && V.mode ? V.mode : null, url: url || null }).catch((err) => toast(String(err)));
  }
  function renderView() {
    if (IS_SETTINGS_WIN) {
      tab = "settings";
      renderTabs();
      viewSettings();
      return;
    }
    if (tab !== "threads") hideEmbed();
    renderTabs();
    const views = { home: viewChat, all: viewHome, tasks: viewTasks, notes: viewNotes, files: viewFiles, clean: viewClean, health: viewHealth, settings: viewSettings, ...EXTRA_VIEWS };
    (views[tab] || viewHome)();
  }

  // ═════════ ASI: CHAT & APPLE INTELLIGENCE (ON-DEVICE) ═════════
  const chat = []; // {who: "me"|"asi", text, reply?}
  let appleAiState = { available: true, engine: "Apple Intelligence (FoundationModels)", detail: "Chạy trực tiếp trên chip Apple Silicon" };
  api("apple_ai_status").then((st) => {
    if (st) {
      appleAiState = st;
      const b = $("#appleAiBadge");
      if (b) {
        b.className = `ai-pill ${st.available ? "ready" : "fallback"}`;
        b.innerHTML = `<span class="ai-spark"></span><b>${st.available ? "Apple Intelligence · Sẵn sàng" : "NLP Nội bộ"}</b><span>${esc(st.detail)}</span>`;
      }
    }
  }).catch(() => {});

  const SUGGEST = [
    ["", "Tóm tắt mục vừa copy"],
    ["", "Gợi ý ưu tiên việc hôm nay"],
    ["", "Dịch sang tiếng Anh"],
    ["", "Viết nháp email"],
    ["", "Tình trạng máy tính"],
    ["", "Dọn rác giải phóng dung lượng"],
    ["", "Kiểm tra việc trễ"],
    ["", "Tập trung 25 phút"],
  ];
  function fmtChatMd(raw) {
    const safe = esc(raw || "");
    return safe
      .replace(/\*\*(.+?)\*\*/g, "<b>$1</b>")
      .replace(/^[•*-]\s+(.+)$/gm, '<div class="md-li">• $1</div>')
      .replace(/^(\d+)\.\s+(.+)$/gm, '<div class="md-li"><b>$1.</b> $2</div>')
      .replace(/\n/g, "<br>");
  }
  function cardHtml(c) {
    if (c.kind === "metrics") return `<div class="a-metrics">${c.items.map((m) => `<div class="a-metric lv${m.level}">
        <span class="muted small">${esc(m.label)}</span><b>${esc(m.value)}</b>${m.ratio != null ? `<i style="--r:${Math.min(1, m.ratio)}"></i>` : ""}</div>`).join("")}</div>`;
    if (c.kind === "tasks") return `<div class="a-list">${c.items.map((t) => `<div class="a-task" data-tid="${t.id}"><span class="grow">${esc(t.title)}</span>
        ${t.due ? `<span class="chip ${t.due < Date.now() ? "red" : ""}">${fmtDue(t.due)}</span>` : ""}
        <button class="btn sm ghost" data-ta="done">✓</button><button class="btn sm ghost" data-ta="s60">+1h</button><button class="btn sm ghost" data-ta="stm">Mai</button></div>`).join("")}</div>`;
    if (c.kind === "events") return `<div class="a-list">${c.items.map(([t, st, ad]) => `<div class="a-task"><span class="ev-dot"></span><span class="grow">${esc(t)}</span><span class="chip blue">${ad ? "Cả ngày" : hm(new Date(st))}</span></div>`).join("")}</div>`;
    if (c.kind === "text") return `<div class="a-text">${fmtChatMd(c.text)}</div>`;
    return "";
  }
  function replyHtml(r) {
    return `${r.cards.map(cardHtml).join("")}
      ${r.actions.length ? `<div class="a-acts">${r.actions.map((a, i) => `<button class="btn sm ${i === 0 ? "primary" : ""}" data-ac="${esc(a.cmd)}" data-arg="${esc(a.arg)}">${esc(a.label)}</button>`).join("")}</div>` : ""}`;
  }
  async function askAsi(text, compact) {
    text = (text || "").trim();
    if (!text) return;
    chat.push({ who: "me", text });
    chat.push({ who: "asi", text: "…", thinking: true });
    if (compact) renderReply(); else if (tab === "home" && mode === "expanded") drawChat();
    moodOverride = "focus"; applyMood();
    let r;
    try { r = await api("asi_ask", { text }); } catch (err) { r = { text: String(err), speak: "", cards: [], actions: [], mood: "worried" }; }
    chat.pop();
    chat.push({ who: "asi", text: r.text, reply: r });
    refreshTasks(); // việc vừa thêm bằng giọng nói hiện ngay trong danh sách
    moodOverride = null;
    flashMood(r.mood || "happy", 2500);
    if (r.speak) api("asi_speak", { text: r.speak });
    if (compact) renderReply(); else if (tab === "home" && mode === "expanded") drawChat();
  }
  function renderReply() {
    const last = chat[chat.length - 1];
    const me = [...chat].reverse().find((m) => m.who === "me");
    $("#replyL").innerHTML = `<div class="r-me">“${esc(me ? me.text : "")}”</div>
      <div class="r-asi">${last.thinking ? '<span class="dots"><i></i><i></i><i></i></span>' : fmtChatMd(last.text)}</div>
      ${last.reply ? replyHtml(last.reply) : ""}
      <div class="r-foot"><button class="btn sm ghost" data-ac="more_voice">${I.mic}Nói tiếp</button><button class="btn sm ghost" data-ac="open_chat">Mở chat</button><button class="btn sm ghost" data-ac="close">Đóng</button></div>`;
    setMode("reply", { force: mode === "reply" });
    clearTimeout(replyTimer);
    replyTimer = setTimeout(() => { if (mode === "reply" && !hovering) setMode("collapsed"); }, 30000);
  }
  let replyTimer = null;
  // Bấm mic ở tai thỏ → Asi nghe → hiểu → trả lời ngay trên tai thỏ
  async function voiceCommand() {
    if (voice.active) { await stopDictation(); return; }
    $("#listenL").innerHTML = `<div class="l-wrap"><button class="mic big rec" id="listenMic">${I.mic}<span class="bars"><i></i><i></i><i></i><i></i></span></button>
      <div><b>Asi đang nghe…</b><div class="muted small">Ví dụ: “máy mình thế nào”, “dọn rác”, “có việc nào trễ không”, “nhắc tôi 3h chiều gọi khách”</div></div></div>`;
    await setMode("listen");
    api("asi_stop_speaking");
    const btn = $("#listenMic");
    btn.addEventListener("click", () => stopDictation());
    btn.classList.remove("rec");
    await dictate(btn, (text) => askAsi(text, true));
    if (!voice.active && mode === "listen") setMode("collapsed"); // không mở được micro
  }
  async function runAction(cmd, arg, compact) {
    if (cmd === "ask") return askAsi(arg, compact);
    if (cmd === "open_tab") {
      if (arg === "settings") return openSettingsWindow();
      tab = arg;
      return setMode("expanded", { force: true });
    }
    if (cmd === "open_chat") { tab = "home"; return setMode("expanded", { force: true }); }
    if (cmd === "more_voice") return voiceCommand();
    if (cmd === "close") { api("asi_stop_speaking"); return setMode("collapsed"); }
    if (cmd === "rest") { setMode("collapsed"); setTimeout(() => showAlert({ kind: "break", minutes: status ? status.session_minutes : 0 }), 450); return; }
    try {
      const r = await api("asi_action", { cmd, arg });
      chat.push({ who: "asi", text: r.text, reply: r });
      refreshTasks();
      flashMood(r.mood || "happy", 3000);
      if (r.mood === "love") confetti(30);
      if (r.speak) api("asi_speak", { text: r.speak });
    } catch (err) { chat.push({ who: "asi", text: String(err) }); }
    if (compact) renderReply(); else drawChat();
  }
  async function taskAct(row, a, compact) {
    const id = Number(row.dataset.tid);
    if (a === "done") { await api("toggle_task", { id }); confetti(12); flashMood("happy"); row.style.opacity = 0.4; row.style.textDecoration = "line-through"; }
    if (a === "s60") await api("snooze_task", { id, minutes: 60 });
    if (a === "stm") await api("snooze_task", { id, minutes: -1 });
    if (a !== "done") { row.remove(); toast("Đã hoãn"); }
    refreshTasks();
  }
  $("#replyL").addEventListener("click", (e) => {
    const t = e.target.closest("[data-ta]");
    if (t) return taskAct(t.closest(".a-task"), t.dataset.ta, true);
    const b = e.target.closest("[data-ac]");
    if (b) runAction(b.dataset.ac, b.dataset.arg || "", true);
  });

  // ───────── Tự cập nhật ứng dụng ─────────
  let updateState = null; // UpdateProgress: { phase, percent, downloaded_bytes, total_bytes, message, error, release }
  let updateVer = "0.11.0";
  api("update_info").then((v) => { if (v) { updateVer = v.current_version || updateVer; updateState = v.status || v; } }).catch(() => {});
  function updateBannerHtml() {
    const u = updateState;
    const rel = u && u.release && u.release.has_update ? u.release : null;
    const ver = (rel && rel.latest_version) || (status && status.update_version);
    if (!ver && (!u || !["downloading", "installing", "restarting"].includes(u.phase))) return "";
    if (u && (u.phase === "downloading" || u.phase === "installing")) {
      const dt = u.total_bytes ? `${bytes(u.downloaded_bytes || 0)} / ${bytes(u.total_bytes)}` : "";
      return `<div class="card upd-banner" id="updBanner"><div class="row"><span class="spin"></span><div class="grow"><b>${esc(u.message || "Đang tải bản cập nhật…")}</b>${dt ? `<div class="muted small">${esc(dt)}</div>` : ""}</div></div>
        <div class="meter" style="margin-top:6px"><i style="width:${u.percent || 5}%"></i></div></div>`;
    }
    if (u && u.phase === "restarting") {
      return `<div class="card upd-banner" id="updBanner"><div class="row"><span class="chip green">Hoàn tất</span><span class="grow"><b>${esc(u.message || "Đã nâng cấp xong! Đang khởi động lại…")}</b></span></div></div>`;
    }
    return `<div class="card upd-banner" id="updBanner"><div class="row"><div class="grow"><b>Bản cập nhật mới Mini-Asi v${esc(ver)}</b>
      <div class="muted small">Tự động nâng cấp trực tiếp, giữ nguyên toàn bộ dữ liệu</div></div>
      <button class="btn sm primary" data-upd="install">Cập nhật ngay</button></div></div>`;
  }
  function updateCardHtml(st) {
    const u = updateState || { phase: "idle", message: "" };
    const ph = u.phase || "idle";
    const rel = u.release && u.release.has_update ? u.release : null;
    let body = "";
    if (ph === "checking") {
      body = `<div class="row" style="margin-top:8px"><span class="spin"></span><span class="muted small grow">Đang kiểm tra phiên bản mới trên GitHub…</span></div>`;
    } else if (ph === "downloading" || ph === "installing") {
      const dt = u.total_bytes ? `${bytes(u.downloaded_bytes || 0)} / ${bytes(u.total_bytes)}` : "";
      body = `<div class="tts-prog" style="margin-top:8px"><span class="spin"></span><div class="grow"><b class="small">${esc(u.message || "Đang cập nhật…")}</b>${dt ? `<div class="muted small">${esc(dt)}</div>` : ""}
        <div class="meter" style="margin-top:6px"><i style="width:${u.percent || 5}%"></i></div></div></div>`;
    } else if (ph === "restarting") {
      body = `<div class="row" style="margin-top:8px"><span class="chip green">Hoàn tất</span><span class="small grow">${esc(u.message || "Đã nâng cấp xong! Đang khởi động lại…")}</span></div>`;
    } else if (rel) {
      const sz = rel.asset && rel.asset.size ? `Dung lượng tải: ${bytes(rel.asset.size)} · ` : "";
      body = `<div class="upd-box" style="margin-top:8px"><div class="row"><div class="grow"><b>Đã có bản mới v${esc(rel.latest_version)}</b>
        <div class="muted small">${sz}Tự tải về và cài đặt trực tiếp, không cần tải lại file dmg</div>
        ${rel.notes ? `<div class="muted small" style="margin-top:4px">${esc(rel.notes)}</div>` : ""}</div>
        <button class="btn primary sm" data-upd="install">Cập nhật ngay</button></div></div>`;
    } else {
      body = `<div class="row" style="margin-top:8px"><span class="muted small grow">${ph === "error" ? `<span style="color:var(--red)">Lỗi: ${esc(u.error || "")}</span>` : esc(u.message || "Bạn đang dùng phiên bản mới nhất")}</span>
        <button class="btn sm" data-upd="check">Kiểm tra cập nhật</button></div>`;
    }
    return `<div class="card" id="updateCard" style="margin-bottom:10px">
      <div class="row"><div class="grow"><b>Cập nhật ứng dụng</b> <span class="chip">v${esc(updateVer)}</span></div>
        <label class="row small"><input type="checkbox" data-k2="auto_update_check" ${st.auto_update_check !== false ? "checked" : ""}/> Tự báo khi có bản mới</label></div>
      ${body}</div>`;
  }
  function redrawUpdateUI() {
    const b = $("#updBanner");
    if (b) b.outerHTML = updateBannerHtml();
    const c = $("#updateCard");
    if (c) c.outerHTML = updateCardHtml(settingsCache);
  }
  on("update-available", (rel) => {
    api("update_info").then((v) => { if (v) { updateVer = v.current_version || updateVer; updateState = v.status || v; } redrawUpdateUI(); renderTabs(); }).catch(() => {});
    if (rel && rel.latest_version) toast(`✨ Đã có Mini-Asi v${rel.latest_version} mới — bấm “Cập nhật ngay” nhé!`);
  });
  on("update-progress", (u) => {
    updateState = u.status || u;
    redrawUpdateUI();
    renderTabs();
    if (updateState.phase === "restarting") { confetti(30); flashMood("love", 3500); toast("Đã cập nhật xong! Mini-Asi đang khởi động lại…"); }
    if (updateState.phase === "error" && tab === "settings") toast("Chưa cập nhật được: " + (updateState.error || ""));
  });
  $("#view").addEventListener("click", async (e) => {
    const b = e.target.closest("[data-upd]");
    if (!b) return;
    e.stopPropagation();
    const act = b.dataset.upd;
    if (act === "check") {
      b.disabled = true;
      b.innerHTML = '<span class="spin"></span>Đang kiểm tra…';
      try {
        const v = await api("update_check");
        if (v) { updateVer = v.current_version || updateVer; updateState = v.status || v; }
        redrawUpdateUI();
        renderTabs();
        const rel = updateState && updateState.release && updateState.release.has_update ? updateState.release : null;
        toast(rel ? `Đã tìm thấy bản mới v${rel.latest_version} 🎉` : "Bạn đang dùng bản mới nhất ✓");
      } catch (err) { toast(String(err)); redrawUpdateUI(); }
    }
    if (act === "install") {
      b.disabled = true;
      b.innerHTML = '<span class="spin"></span>Đang bắt đầu…';
      try { await api("update_install"); } catch (err) { toast(String(err)); redrawUpdateUI(); }
    }
  });

  function greet() {
    const h = new Date().getHours();
    return h < 11 ? "Chào buổi sáng" : h < 14 ? "Chào buổi trưa" : h < 18 ? "Chào buổi chiều" : "Chào buổi tối";
  }
  function viewChat() {
    const s = status;
    const quick = s ? [
      s.lateness.overdue ? `<span class="chip red">${s.lateness.overdue} việc trễ</span>` : `<span class="chip green">Không có việc trễ</span>`,
      s.disk ? `<span class="chip ${s.disk.level === "ok" ? "" : "amber"}">${I.disk}${Math.round(s.disk.free_percent)}% trống</span>` : "",
      s.next_event ? `<span class="chip blue">${I.cal}${esc(s.next_event.title)} · ${hm(new Date(s.next_event.start))}</span>` : "",
    ].join("") : "";
    const aiAvail = appleAiState && appleAiState.available;
    $("#view").innerHTML = `<div class="chat-home">
      ${updateBannerHtml()}
      <div class="c-hero">
        <div class="big-face" id="bigFace"><span class="eye l"></span><span class="eye r"></span></div>
        <div class="grow">
          <div class="row" style="gap:8px;flex-wrap:wrap">
            <div class="c-greet">${greet()}</div>
            <div class="ai-pill ${aiAvail ? "ready" : "fallback"}" id="appleAiBadge" title="${esc(appleAiState.detail || "")}">
              <span class="ai-spark"></span><b>${aiAvail ? "Apple Intelligence" : "NLP Nội bộ"}</b>
            </div>
          </div>
          <div class="muted small">Trợ lý on-device trên chip Apple Silicon</div>
          <div class="c-quick">${quick}</div>
        </div>
      </div>
      <div id="vocabStrip"></div>
      <div class="c-log" id="chatLog"></div>
      <div class="c-chips" id="chatChips">${SUGGEST.map(([, t]) => `<button class="c-chip" data-say="${esc(t)}">${esc(t)}</button>`).join("")}</div>
      <div class="c-input">
        <button class="mic big" id="chatMic" title="Nói với Asi">${I.mic}<span class="bars"><i></i><i></i><i></i><i></i></span></button>
        <input class="input" id="chatInput" placeholder="Hỏi Asi & Apple Intelligence…" />
        <button class="btn primary" id="chatSend">${I.send}</button>
      </div></div>`;
    const inp = $("#chatInput");
    const send = () => { const v = inp.value; inp.value = ""; askAsi(v, false); };
    inp.addEventListener("keydown", (e) => { if (e.key === "Enter" && !isComposing(e)) send(); });
    $("#chatSend").addEventListener("click", send);
    $("#chatMic").addEventListener("click", (e) => dictate(e.currentTarget, (text) => askAsi(text, false)));
    drawChat();
    if (window.VocabUI && settingsCache.vocab_enabled !== false) { window.VocabUI.strip($("#vocabStrip"), freshOpen); freshOpen = false; }
    setTimeout(() => inp.focus(), 80);
  }
  function drawChat() {
    const log = $("#chatLog");
    if (!log) return;
    log.innerHTML = chat.length ? chat.map((m) => m.who === "me"
      ? `<div class="bubble me">${esc(m.text)}</div>`
      : `<div class="bubble asi">${m.thinking ? '<span class="dots"><i></i><i></i><i></i></span>' : fmtChatMd(m.text)}${m.reply ? replyHtml(m.reply) : ""}</div>`).join("")
      : `<div class="muted small" style="text-align:center;padding:14px 10px">Hỏi đáp với <b>Apple Intelligence</b> hoặc chọn gợi ý bên dưới</div>`;
    log.scrollTop = log.scrollHeight;
  }
  $("#view").addEventListener("click", (e) => {
    if (tab !== "home") return;
    const say = e.target.closest("[data-say]");
    if (say) return askAsi(say.dataset.say, false);
    const t = e.target.closest("[data-ta]");
    if (t) return taskAct(t.closest(".a-task"), t.dataset.ta, false);
    const b = e.target.closest("[data-ac]");
    if (b) runAction(b.dataset.ac, b.dataset.arg || "", false);
  });

  // ═════════ BẢNG WIDGET HÌNH VUÔNG LỚN & THƯ VIỆN WIDGET KÉO THẢ ═════════
  const DEFAULT_ACTIVE_WIDGETS = ["tasks", "notes", "clips", "media", "journal", "words", "clean", "health", "camera"];
  let showWidgetLib = false;
  let widgetDataCache = { notes: [], clips: [], perms: [], jv: null, clipImgUrl: "", vocabWord: null };

  // Kho hình minh hoạ SVG kiểu Apple iOS Widget (hiển thị khi người dùng chưa chọn ảnh riêng)
  function widgetArtSvg(id, meta = {}) {
    switch (id) {
      case "tasks":
        return `<svg viewBox="0 0 240 120" class="w-art-svg"><defs><linearGradient id="wgTasks" x1="0" y1="0" x2="1" y2="1"><stop offset="0%" stop-color="#0f3822"/><stop offset="100%" stop-color="#1e824c"/></linearGradient></defs>
          <rect width="240" height="120" fill="url(#wgTasks)"/>
          <circle cx="195" cy="28" r="46" fill="rgba(48,209,88,0.18)"/>
          <rect x="18" y="22" width="132" height="22" rx="7" fill="rgba(255,255,255,0.14)"/>
          <circle cx="30" cy="33" r="5.5" fill="#30d158"/><rect x="42" y="30" width="88" height="6" rx="3" fill="rgba(255,255,255,0.85)"/>
          <rect x="18" y="50" width="114" height="22" rx="7" fill="rgba(255,255,255,0.1)"/>
          <circle cx="30" cy="61" r="5.5" stroke="#fff" stroke-width="2" fill="none"/><rect x="42" y="58" width="72" height="6" rx="3" fill="rgba(255,255,255,0.6)"/>
          <rect x="18" y="78" width="96" height="20" rx="7" fill="rgba(255,255,255,0.07)"/>
          <circle cx="188" cy="62" r="28" stroke="rgba(255,255,255,0.18)" stroke-width="6" fill="none"/>
          <circle cx="188" cy="62" r="28" stroke="#30d158" stroke-width="6" stroke-dasharray="130 180" stroke-linecap="round" fill="none"/></svg>`;
      case "notes":
        return `<svg viewBox="0 0 240 120" class="w-art-svg"><defs><linearGradient id="wgNotes" x1="0" y1="0" x2="1" y2="1"><stop offset="0%" stop-color="#4a2c00"/><stop offset="100%" stop-color="#c97a00"/></linearGradient></defs>
          <rect width="240" height="120" fill="url(#wgNotes)"/>
          <rect x="20" y="16" width="200" height="90" rx="12" fill="rgba(255,248,225,0.14)" stroke="rgba(255,214,10,0.3)"/>
          <line x1="34" y1="40" x2="204" y2="40" stroke="rgba(255,255,255,0.18)" stroke-width="1.5"/>
          <line x1="34" y1="62" x2="188" y2="62" stroke="rgba(255,255,255,0.18)" stroke-width="1.5"/>
          <line x1="34" y1="84" x2="160" y2="84" stroke="rgba(255,255,255,0.18)" stroke-width="1.5"/>
          <circle cx="194" cy="78" r="18" fill="#ffd60a"/><path d="M188 83l3-10 8-8 4 4-8 8-7 6z" fill="#1c1c1e"/></svg>`;
      case "clips":
        return `<svg viewBox="0 0 240 120" class="w-art-svg"><defs><linearGradient id="wgClips" x1="0" y1="0" x2="1" y2="1"><stop offset="0%" stop-color="#082b49"/><stop offset="100%" stop-color="#0a84ff"/></linearGradient></defs>
          <rect width="240" height="120" fill="url(#wgClips)"/>
          <rect x="42" y="26" width="124" height="76" rx="10" fill="rgba(255,255,255,0.12)" transform="rotate(-4 104 64)"/>
          <rect x="58" y="18" width="132" height="80" rx="11" fill="rgba(255,255,255,0.2)" stroke="rgba(255,255,255,0.35)"/>
          <rect x="74" y="36" width="84" height="7" rx="3.5" fill="#fff"/>
          <rect x="74" y="52" width="100" height="6" rx="3" fill="rgba(255,255,255,0.65)"/>
          <rect x="74" y="66" width="68" height="6" rx="3" fill="rgba(255,255,255,0.45)"/></svg>`;
      case "media":
        return `<svg viewBox="0 0 240 120" class="w-art-svg"><defs><linearGradient id="wgMedia" x1="0" y1="0" x2="1" y2="1"><stop offset="0%" stop-color="#3b0d2c"/><stop offset="50%" stop-color="#7c1d54"/><stop offset="100%" stop-color="#ff2d55"/></linearGradient></defs>
          <rect width="240" height="120" fill="url(#wgMedia)"/>
          <circle cx="64" cy="60" r="36" fill="#141418" stroke="rgba(255,255,255,0.25)" stroke-width="2"/>
          <circle cx="64" cy="60" r="24" stroke="rgba(255,255,255,0.12)" stroke-width="1.5" fill="none"/>
          <circle cx="64" cy="60" r="10" fill="#ff375f"/>
          <g fill="rgba(255,255,255,0.85)">
            <rect x="124" y="48" width="6" height="26" rx="3"/><rect x="136" y="34" width="6" height="40" rx="3"/>
            <rect x="148" y="42" width="6" height="32" rx="3"/><rect x="160" y="26" width="6" height="48" rx="3"/>
            <rect x="172" y="40" width="6" height="34" rx="3"/><rect x="184" y="52" width="6" height="22" rx="3"/>
            <rect x="196" y="36" width="6" height="38" rx="3"/>
          </g></svg>`;
      case "journal":
        return `<svg viewBox="0 0 240 120" class="w-art-svg"><defs><linearGradient id="wgJour" x1="0" y1="0" x2="0" y2="1"><stop offset="0%" stop-color="#1e1136"/><stop offset="55%" stop-color="#6b2d5c"/><stop offset="100%" stop-color="#f08a5d"/></linearGradient></defs>
          <rect width="240" height="120" fill="url(#wgJour)"/>
          <circle cx="182" cy="36" r="18" fill="#ffd60a" opacity="0.9"/>
          <circle cx="48" cy="24" r="1.5" fill="#fff"/><circle cx="92" cy="18" r="2" fill="#fff"/><circle cx="134" cy="28" r="1.5" fill="#fff"/>
          <path d="M0 96 L55 56 L108 88 L162 48 L240 92 L240 120 L0 120 Z" fill="rgba(20,14,38,0.55)"/>
          <path d="M0 104 L82 66 L146 96 L204 62 L240 86 L240 120 L0 120 Z" fill="rgba(12,10,24,0.82)"/></svg>`;
      case "words":
        return `<svg viewBox="0 0 240 120" class="w-art-svg"><defs><linearGradient id="wgWords" x1="0" y1="0" x2="1" y2="1"><stop offset="0%" stop-color="#3d1e05"/><stop offset="100%" stop-color="#ff9500"/></linearGradient></defs>
          <rect width="240" height="120" fill="url(#wgWords)"/>
          <rect x="18" y="16" width="204" height="88" rx="14" fill="rgba(0,0,0,0.28)" stroke="rgba(255,255,255,0.22)"/>
          <text x="32" y="58" fill="#fff" font-size="23" font-weight="800" font-family="-apple-system,system-ui">${esc(meta.word || "Serendipity")}</text>
          <text x="32" y="82" fill="#ffd60a" font-size="13" font-family="ui-monospace,Menlo,monospace">${esc(meta.ipa || "/ˌser.ənˈdɪp.ə.ti/")}</text></svg>`;
      case "clean":
        return `<svg viewBox="0 0 240 120" class="w-art-svg"><defs><linearGradient id="wgClean" x1="0" y1="0" x2="1" y2="1"><stop offset="0%" stop-color="#1a2a3a"/><stop offset="100%" stop-color="#2980b9"/></linearGradient></defs>
          <rect width="240" height="120" fill="url(#wgClean)"/>
          <rect x="24" y="34" width="192" height="52" rx="12" fill="rgba(0,0,0,0.32)" stroke="rgba(255,255,255,0.2)"/>
          <rect x="36" y="52" width="168" height="16" rx="8" fill="rgba(255,255,255,0.14)"/>
          <rect x="36" y="52" width="${Math.max(24, Math.min(168, Math.round((meta.usedPct || 55) * 1.68)))}" height="16" rx="8" fill="#30d158"/>
          <circle cx="192" cy="43" r="4" fill="#30d158"/></svg>`;
      case "health":
        return `<svg viewBox="0 0 240 120" class="w-art-svg"><defs><linearGradient id="wgHealth" x1="0" y1="0" x2="1" y2="1"><stop offset="0%" stop-color="#3a0ca3"/><stop offset="100%" stop-color="#f72585"/></linearGradient></defs>
          <rect width="240" height="120" fill="url(#wgHealth)"/>
          <circle cx="120" cy="60" r="38" stroke="rgba(255,255,255,0.16)" stroke-width="8" fill="none"/>
          <circle cx="120" cy="60" r="38" stroke="#4cc9f0" stroke-width="8" stroke-dasharray="170 240" stroke-linecap="round" fill="none"/>
          <circle cx="120" cy="60" r="24" stroke="rgba(255,255,255,0.16)" stroke-width="6" fill="none"/>
          <circle cx="120" cy="60" r="24" stroke="#30d158" stroke-width="6" stroke-dasharray="110 160" stroke-linecap="round" fill="none"/></svg>`;
      case "camera":
        return `<svg viewBox="0 0 240 120" class="w-art-svg"><defs><linearGradient id="wgCam" x1="0" y1="0" x2="1" y2="1"><stop offset="0%" stop-color="#141e30"/><stop offset="100%" stop-color="#243b55"/></linearGradient></defs>
          <rect width="240" height="120" fill="url(#wgCam)"/>
          <circle cx="120" cy="60" r="34" fill="rgba(255,255,255,0.08)" stroke="rgba(255,255,255,0.3)" stroke-width="2.5"/>
          <circle cx="120" cy="60" r="20" fill="#0a84ff" opacity="0.65"/>
          <circle cx="127" cy="52" r="6" fill="rgba(255,255,255,0.75)"/>
          <circle cx="176" cy="30" r="5" fill="#30d158"/></svg>`;
      case "snap":
        return `<svg viewBox="0 0 240 120" class="w-art-svg"><defs><linearGradient id="wgSnap" x1="0" y1="0" x2="1" y2="1"><stop offset="0%" stop-color="#0f2027"/><stop offset="100%" stop-color="#2c5364"/></linearGradient></defs>
          <rect width="240" height="120" fill="url(#wgSnap)"/>
          <rect x="24" y="18" width="90" height="84" rx="10" fill="rgba(10,132,255,0.45)" stroke="#5ac8fa" stroke-width="2"/>
          <rect x="126" y="18" width="90" height="84" rx="10" fill="rgba(48,209,88,0.42)" stroke="#30d158" stroke-width="2"/></svg>`;
      case "files":
        return `<svg viewBox="0 0 240 120" class="w-art-svg"><defs><linearGradient id="wgFiles" x1="0" y1="0" x2="1" y2="1"><stop offset="0%" stop-color="#003973"/><stop offset="100%" stop-color="#007aff"/></linearGradient></defs>
          <rect width="240" height="120" fill="url(#wgFiles)"/>
          <circle cx="120" cy="72" r="46" stroke="rgba(255,255,255,0.22)" stroke-width="2" fill="none"/>
          <circle cx="120" cy="72" r="30" stroke="rgba(255,255,255,0.38)" stroke-width="2" fill="none"/>
          <circle cx="120" cy="72" r="14" fill="#fff"/></svg>`;
      case "asi":
        return `<svg viewBox="0 0 240 120" class="w-art-svg"><defs><linearGradient id="wgAsi" x1="0" y1="0" x2="1" y2="1"><stop offset="0%" stop-color="#ff375f"/><stop offset="50%" stop-color="#bf5af2"/><stop offset="100%" stop-color="#0a84ff"/></linearGradient></defs>
          <rect width="240" height="120" fill="url(#wgAsi)"/>
          <rect x="78" y="28" width="84" height="64" rx="26" fill="#0b0b0f" stroke="rgba(255,255,255,0.4)" stroke-width="2"/>
          <rect x="98" y="48" width="12" height="22" rx="6" fill="#fff"/><rect x="130" y="48" width="12" height="22" rx="6" fill="#fff"/></svg>`;
      case "threads":
        return `<svg viewBox="0 0 240 120" class="w-art-svg"><defs><linearGradient id="wgTh" x1="0" y1="0" x2="1" y2="1"><stop offset="0%" stop-color="#161622"/><stop offset="100%" stop-color="#5e5ce6"/></linearGradient></defs>
          <rect width="240" height="120" fill="url(#wgTh)"/>
          <circle cx="120" cy="60" r="34" stroke="rgba(255,255,255,0.35)" stroke-width="3" fill="rgba(0,0,0,0.3)"/>
          <text x="120" y="72" text-anchor="middle" fill="#fff" font-size="36" font-weight="800">@</text></svg>`;
      default:
        return `<svg viewBox="0 0 240 120" class="w-art-svg"><defs><linearGradient id="wgDef" x1="0" y1="0" x2="1" y2="1"><stop offset="0%" stop-color="#2c2c2e"/><stop offset="100%" stop-color="#48484a"/></linearGradient></defs>
          <rect width="240" height="120" fill="url(#wgDef)"/>
          <circle cx="120" cy="60" r="28" stroke="rgba(255,255,255,0.35)" stroke-width="4" fill="none"/></svg>`;
    }
  }

  function activeWidgetIds() {
    const list = settingsCache.active_widgets;
    return Array.isArray(list) && list.length ? list : [...DEFAULT_ACTIVE_WIDGETS];
  }

  function buildWidgetCatalog(s, d) {
    const notesL = d.notes || [];
    const clipsL = d.clips || [];
    const jv = d.jv;
    const wrote = jv && (jv.entry.answers.some((a) => a.a.trim()) || jv.entry.text.trim());
    const latestClip = clipsL[0];
    const m = mediaQuick || {};
    const usedPct = s.disk ? Math.round(100 - s.disk.free_percent) : 50;
    const vw = d.vocabWord || { w: "Resilient", ipa: "/rɪˈzɪl.i.ənt/", vi: "Kiên cường, bền bỉ" };

    return [
      {
        id: "tasks",
        name: "Việc cần làm",
        ic: I.tasks,
        color: "#30d158",
        badge: s.lateness.overdue ? `${s.lateness.overdue} trễ` : `${s.open_count} việc`,
        badgeCls: s.lateness.overdue ? "red" : "green",
        title: s.next ? s.next.title : (s.open_count ? `Còn ${s.open_count} việc chưa hẹn giờ` : "Hôm nay đã xong hết việc"),
        sub: s.next ? `Hạn: ${fmtDue(s.next.due)}` : "Bấm để thêm việc bằng giọng nói hoặc chữ",
        go: "tasks",
        artMeta: {},
      },
      {
        id: "notes",
        name: "Ghi chú",
        ic: I.note,
        color: "#ff9f0a",
        badge: `${notesL.length} mục`,
        badgeCls: "amber",
        title: notesL[0] ? notesL[0].text.split("\n")[0].slice(0, 48) : "Chưa có ghi chú nào",
        sub: notesL[0] ? (notesL[0].text.split("\n").slice(1).join(" ").slice(0, 68) || `Cập nhật ${fmtAgo(notesL[0].updated_at)}`) : "Bấm để viết nhanh hoặc đọc vào mic",
        go: "notes",
        artMeta: {},
      },
      {
        id: "clips",
        name: "Clipboard",
        ic: I.clip,
        color: "#5ac8fa",
        badge: `${clipsL.length} đã chép`,
        badgeCls: "blue",
        title: latestClip ? (latestClip.text ? latestClip.text.slice(0, 52) : "Hình ảnh / Tệp tin vừa chép") : "Clipboard đang trống",
        sub: "Nhấn ⌃⇧V mở nhanh · Bấm để xem & dán lại",
        go: "clips",
        liveImg: d.clipImgUrl || "",
        artMeta: {},
      },
      {
        id: "media",
        name: "Nhạc & Âm thanh",
        ic: I.music,
        color: "#ff2d55",
        badge: m.playing ? "Đang phát" : "Tạm dừng",
        badgeCls: m.playing ? "green" : "",
        title: m.title || (m.app ? m.app : "Trình phát nhạc hệ thống"),
        sub: m.artist ? `${m.artist} · Loa ${m.output_volume}%` : "Điều khiển nhạc & âm lượng ngay trên Widget",
        go: "media-noop",
        quickHtml: `<div class="sq-actions">
          <button class="sq-btn" data-wcmd="media-prev" title="Bài trước">${I.prev}</button>
          <button class="sq-btn primary" data-wcmd="media-play" title="Phát / Tạm dừng">${m.playing ? I.pause : I.play}</button>
          <button class="sq-btn" data-wcmd="media-next" title="Bài tiếp">${I.next}</button>
          <button class="sq-btn" data-wcmd="media-mute" title="Bật / Tắt tiếng loa">${m.output_muted ? I.volOff : I.vol}</button>
        </div>`,
        artMeta: {},
      },
      {
        id: "journal",
        name: "Nhật ký",
        ic: I.book,
        color: "#ff375f",
        badge: jv && jv.streak ? `${jv.streak} ngày` : (wrote ? "Đã viết" : "Hôm nay"),
        badgeCls: wrote ? "green" : "",
        title: wrote ? (jv.entry.text || jv.entry.answers.map((x) => x.a).filter(Boolean).join(" · ")).slice(0, 54) : "Hôm nay của bạn thế nào?",
        sub: "Lưu giữ kỷ niệm kèm hình ảnh & cảm xúc mỗi ngày",
        go: "journal",
        artMeta: {},
      },
      {
        id: "words",
        name: "Từ mới EN",
        ic: I.words,
        color: "#ff9500",
        badge: "Mỗi ngày",
        badgeCls: "amber",
        title: `${vw.w} — ${vw.vi}`,
        sub: "Luyện nghe chuẩn & chấm điểm phát âm tiếng Anh",
        go: "words",
        quickHtml: `<div class="sq-actions">
          <button class="sq-btn" data-wcmd="word-speak" data-word="${esc(vw.w)}">${I.vol} Phát âm</button>
          <button class="sq-btn primary" data-wcmd="open-words">Luyện đọc →</button>
        </div>`,
        artMeta: { word: vw.w, ipa: vw.ipa },
      },
      {
        id: "camera",
        name: "Gương & Khung Ảnh",
        ic: I.cam,
        color: "#30d158",
        badge: "Camera / Ảnh",
        badgeCls: "blue",
        title: "Khung ảnh & Gương nhanh trên Tai thỏ",
        sub: "Bấm đổi ảnh để lồng ảnh yêu thích hoặc mở Camera",
        go: "camera-noop",
        liveImg: peekCam.lastSnap || "",
        quickHtml: `<div class="sq-actions">
          <button class="sq-btn primary" data-wcmd="open-cam">${I.cam} Mở gương Camera</button>
          <button class="sq-btn" data-wact="img" data-wid="camera">Đổi ảnh</button>
        </div>`,
        artMeta: {},
      },
      {
        id: "clean",
        name: "Dọn máy & Ổ đĩa",
        ic: I.broom,
        color: "#ff9f0a",
        badge: s.disk ? `${Math.round(s.disk.free_percent)}% trống` : "Ổ đĩa",
        badgeCls: s.disk && s.disk.level !== "ok" ? "amber" : "green",
        title: s.disk ? `Còn trống ${bytes(s.disk.free_bytes)} / ${bytes(s.disk.total_bytes)}` : "Quét rác & giải phóng bộ nhớ",
        sub: "Xoá cache an toàn, dọn Xcode / Docker / trình duyệt",
        go: "clean",
        artMeta: { usedPct },
      },
      {
        id: "health",
        name: "Sức khoẻ & Tập trung",
        ic: I.heart,
        color: "#ff2d55",
        badge: s.focus_until ? "Đang tập trung" : mins(s.session_minutes),
        badgeCls: s.focus_until ? "blue" : "",
        title: s.focus_until ? "Chế độ Pomodoro đang chạy" : `Đã dùng máy liên tục ${mins(s.session_minutes)}`,
        sub: "Nhắc nghỉ mắt 20-20-20 & đồng hồ Tập trung Pomodoro",
        go: "health",
        quickHtml: `<div class="sq-actions">
          <button class="sq-btn primary" data-wcmd="toggle-focus">${s.focus_until ? "Dừng Tập trung" : "Tập trung 25p"}</button>
          <button class="sq-btn" data-wcmd="rest-eyes">${I.eye} Nghỉ mắt</button>
        </div>`,
        artMeta: {},
      },
      {
        id: "snap",
        name: "Chia màn hình",
        ic: I.grid,
        color: "#0a84ff",
        badge: "Window Snap",
        badgeCls: "blue",
        title: "Chia nhanh cửa sổ ứng dụng trên Mac",
        sub: "Bấm nút bên dưới hoặc kéo thanh tiêu đề cửa sổ vào tai thỏ",
        go: "snap-noop",
        quickHtml: `<div class="sq-actions">
          <button class="sq-btn" data-wcmd="snap" data-snap="left">Trái 50%</button>
          <button class="sq-btn" data-wcmd="snap" data-snap="right">Phải 50%</button>
          <button class="sq-btn primary" data-wcmd="snap" data-snap="split2">Chia đôi 2 app</button>
          <button class="sq-btn" data-wcmd="snap" data-snap="max">Toàn màn hình</button>
        </div>`,
        artMeta: {},
      },
      {
        id: "files",
        name: "File & AirDrop",
        ic: I.file,
        color: "#007aff",
        badge: "AirDrop · ZIP",
        badgeCls: "blue",
        title: "Nén ZIP, giải nén, nén PDF & gửi AirDrop",
        sub: "Kéo thả file vào tai thỏ hoặc bấm để mở khay File",
        go: "files",
        artMeta: {},
      },
      {
        id: "asi",
        name: "Trợ lý Asi (Apple AI)",
        ic: I.mic,
        color: "#bf5af2",
        badge: " Apple AI",
        badgeCls: "blue",
        title: "Hỏi đáp thông minh với Apple Intelligence",
        sub: "Tóm tắt văn bản, dịch thuật, viết email & điều khiển máy bằng giọng nói",
        go: "home",
        artMeta: {},
      },
      {
        id: "threads",
        name: "Threads",
        ic: I.at,
        color: "#5e5ce6",
        badge: s.threads_unread ? `${s.threads_unread} mới` : "@threads",
        badgeCls: s.threads_unread ? "red" : "",
        title: s.threads_unread ? `Có ${s.threads_unread} phản hồi chưa đọc` : "Lướt bảng tin & đăng bài Threads",
        sub: "Theo dõi tương tác và trả lời bình luận nhanh",
        go: "threads",
        artMeta: {},
      },
      {
        id: "settings",
        name: "Cài đặt Mini-Asi",
        ic: I.gear,
        color: "#8e8e93",
        badge: "Cửa sổ riêng ↗",
        badgeCls: "",
        title: "Mở Cài đặt trên cửa sổ độc lập",
        sub: "Chỉnh cỡ tai thỏ, bộ gõ tiếng Việt, giọng đọc AI, quyền & sao lưu",
        go: "settings",
        artMeta: {},
      },
    ];
  }

  function renderSquareWidgetCard(w) {
    const customImg = (settingsCache.widget_images || {})[w.id] || "";
    const imgSrc = customImg || w.liveImg || "";
    const mediaHtml = imgSrc
      ? `<img src="${esc(imgSrc)}" alt="${esc(w.name)}" class="w-art-img" />`
      : widgetArtSvg(w.id, w.artMeta);
    return `<div class="sq-widget" data-widget-id="${w.id}" data-go="${w.go}" style="--tc:${w.color}">
      <div class="sq-media">
        ${mediaHtml}
        <div class="sq-media-overlay">
          <span class="sq-badge ${w.badgeCls || ""}">${esc(w.badge)}</span>
          <div class="sq-tools" title="Tuỳ chỉnh">
            <button class="sq-tool-btn" data-wact="img" data-wid="${w.id}" title="Đổi ảnh">Ảnh</button>
            ${customImg ? `<button class="sq-tool-btn" data-wact="reset-img" data-wid="${w.id}" title="Khôi phục mặc định">${I.refresh}</button>` : ""}
            <button class="sq-tool-btn danger" data-wact="remove" data-wid="${w.id}" title="Gỡ khỏi bảng">${I.trash}</button>
          </div>
        </div>
      </div>
      <div class="sq-body">
        <div class="sq-head">
          <span class="sq-ic" style="background:${w.color}">${w.ic}</span>
          <b class="sq-name">${esc(w.name)}</b>
        </div>
        <div class="sq-title">${esc(w.title)}</div>
        <div class="sq-sub">${esc(w.sub)}</div>
        ${w.quickHtml || ""}
      </div>
    </div>`;
  }

  function renderWidgetBoardDom() {
    const s = status || { lateness: { overdue: 0 }, open_count: 0, session_minutes: 0 };
    const catalog = buildWidgetCatalog(s, widgetDataCache);
    const activeIds = activeWidgetIds();
    const activeWidgets = activeIds.map((id) => catalog.find((w) => w.id === id)).filter(Boolean);
    const missingList = (widgetDataCache.perms || []).filter((p) => p.required && p.state !== "granted" && p.state !== "unknown");

    $("#view").innerHTML = `
      ${updateBannerHtml()}
      <div class="wb-toolbar">
        <div class="grow">
          <b class="wb-heading">Widget</b>
          <span class="muted small" style="margin-left:8px">Kéo thả để sắp xếp vị trí</span>
        </div>
        <button class="btn sm ${showWidgetLib ? "primary" : "ghost"}" id="toggleWidgetLibBtn">
          ${showWidgetLib ? "Xong" : "Sửa"}
        </button>
      </div>
      ${missingList.length ? `<div class="card perm-banner" style="margin-bottom:10px"><div class="row"><span class="grow"><b>Chưa cấp đủ quyền: ${missingList.map((p) => p.name).join(", ")}</b>
        <span class="muted small"> — cấp quyền trong Cài đặt</span></span>
        <button class="btn sm primary" data-go="perm">Cài đặt</button></div></div>` : ""}
      <div class="wb-layout ${showWidgetLib ? "with-lib" : ""}">
        <div class="sq-grid" id="widgetBoard">
          ${activeWidgets.length
            ? activeWidgets.map(renderSquareWidgetCard).join("")
            : `<div class="wb-empty">Chưa có Widget nào trên bảng. Bấm Sửa để thêm.</div>`}
        </div>
        ${showWidgetLib ? `<aside class="wb-lib" id="widgetLib">
          <div class="wb-lib-head">
            <b>Thư viện Widget</b>
            <span class="muted small">Kéo thả vào bảng hoặc bấm Thêm</span>
          </div>
          <div class="wb-lib-list">
            ${catalog.map((w) => {
              const onBoard = activeIds.includes(w.id);
              return `<div class="wb-lib-card ${onBoard ? "on-board" : ""}" data-lib-id="${w.id}" style="--tc:${w.color}">
                <div class="wb-lib-art">${widgetArtSvg(w.id, w.artMeta)}</div>
                <div class="wb-lib-info">
                  <div class="row">
                    <span class="sq-ic sm" style="background:${w.color}">${w.ic}</span>
                    <b class="grow small">${esc(w.name)}</b>
                    ${onBoard
                      ? `<span class="chip">Đã thêm</span>`
                      : `<button class="btn sm primary" data-wact="add" data-wid="${w.id}">Thêm</button>`}
                  </div>
                  <div class="muted small" style="margin-top:3px">${esc(w.sub)}</div>
                </div>
              </div>`;
            }).join("")}
          </div>
        </aside>` : ""}
      </div>
      <input type="file" id="widgetImgPicker" accept="image/*" style="display:none" />`;

    $("#toggleWidgetLibBtn")?.addEventListener("click", () => {
      showWidgetLib = !showWidgetLib;
      renderWidgetBoardDom();
    });
  }

  async function viewHome() {
    // Vẽ ngay từ bộ nhớ đệm để chuyển tab mượt tức thì (0ms nhấp nháy), sau đó nạp dữ liệu mới cập nhật lại
    renderWidgetBoardDom();
    const [s, notesL, clipsL, perms] = await Promise.all([
      status ? Promise.resolve(status) : api("get_status"),
      api("list_notes").catch(() => []),
      api("clip_list", { query: "", kind: "all" }).catch(() => []),
      api("permissions_status").catch(() => []),
    ]);
    status = s;
    const d = new Date();
    const today = `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
    let jv = null;
    try { jv = await api("journal_get", { date: today }); } catch (_) {}
    let clipImgUrl = "";
    if (clipsL[0] && clipsL[0].kind === "image") {
      try { clipImgUrl = await api("clip_image", { id: clipsL[0].id }); } catch (_) {}
    }
    widgetDataCache = { notes: notesL, clips: clipsL, perms, jv, clipImgUrl, vocabWord: widgetDataCache.vocabWord };
    if (tab === "all" && !IS_SETTINGS_WIN && !widgetDragging) {
      renderWidgetBoardDom();
    }
  }

  // Đổi ảnh tuỳ chọn cho bất kỳ Widget nào
  let pickingImgForWidget = null;
  document.addEventListener("change", (e) => {
    if (e.target.id !== "widgetImgPicker" || !pickingImgForWidget) return;
    const file = e.target.files && e.target.files[0];
    const wid = pickingImgForWidget;
    pickingImgForWidget = null;
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      const img = new Image();
      img.onload = async () => {
        const cv = document.createElement("canvas");
        const maxW = 480, maxH = 300;
        const scale = Math.min(maxW / img.width, maxH / img.height, 1);
        cv.width = Math.max(1, Math.round(img.width * scale));
        cv.height = Math.max(1, Math.round(img.height * scale));
        cv.getContext("2d").drawImage(img, 0, 0, cv.width, cv.height);
        const dataUrl = cv.toDataURL("image/jpeg", 0.86);
        settingsCache.widget_images = { ...(settingsCache.widget_images || {}), [wid]: dataUrl };
        await api("save_settings", { settings: settingsCache });
        renderWidgetBoardDom();
        toast("🖼 Đã cập nhật ảnh cho Widget!");
      };
      img.src = reader.result;
    };
    reader.readAsDataURL(file);
  });

  // Kéo thả mượt mà (Pointer Drag & Drop) giữa Thư viện Widget và Bảng Widget
  let widgetDragging = null;
  (function setupWidgetDragAndDrop() {
    const viewEl = $("#view");
    viewEl.addEventListener("pointerdown", (e) => {
      if (tab !== "all" || e.button !== 0) return;
      if (e.target.closest("button, input, select, a, .sq-actions, .sq-tools")) return;
      const boardCard = e.target.closest(".sq-widget[data-widget-id]");
      const libCard = e.target.closest(".wb-lib-card[data-lib-id]");
      const srcEl = boardCard || libCard;
      if (!srcEl) return;
      const wid = boardCard ? boardCard.dataset.widgetId : libCard.dataset.libId;
      const fromLib = !!libCard;
      const rect = srcEl.getBoundingClientRect();
      widgetDragging = {
        wid,
        fromLib,
        srcEl,
        sx: e.clientX,
        sy: e.clientY,
        ox: e.clientX - rect.left,
        oy: e.clientY - rect.top,
        w: Math.min(rect.width, 210),
        h: Math.min(rect.height, 210),
        moved: false,
        ghost: null,
        targetId: null,
        insertAfter: false,
        overLib: false,
      };
    });

    window.addEventListener("pointermove", (e) => {
      if (!widgetDragging) return;
      const dx = e.clientX - widgetDragging.sx;
      const dy = e.clientY - widgetDragging.sy;
      if (!widgetDragging.moved && Math.hypot(dx, dy) < 7) return;
      if (!widgetDragging.moved) {
        widgetDragging.moved = true;
        widgetDragging.srcEl.classList.add("dragging-src");
        const g = widgetDragging.srcEl.cloneNode(true);
        g.className = "sq-drag-ghost";
        g.style.width = widgetDragging.w + "px";
        g.style.height = widgetDragging.h + "px";
        document.body.appendChild(g);
        widgetDragging.ghost = g;
      }
      const g = widgetDragging.ghost;
      if (g) {
        g.style.transform = `translate3d(${e.clientX - widgetDragging.ox}px, ${e.clientY - widgetDragging.oy}px, 0) scale(1.03)`;
      }
      // Xét điểm rơi trên Bảng Widget hoặc Thư viện
      $$("#widgetBoard .sq-widget").forEach((c) => c.classList.remove("drop-before", "drop-after"));
      const libEl = $("#widgetLib");
      if (libEl) {
        const lr = libEl.getBoundingClientRect();
        widgetDragging.overLib = e.clientX >= lr.left && e.clientX <= lr.right && e.clientY >= lr.top && e.clientY <= lr.bottom;
        libEl.classList.toggle("drop-remove", !widgetDragging.fromLib && widgetDragging.overLib);
      }
      if (g) g.style.display = "none";
      const under = document.elementFromPoint(e.clientX, e.clientY);
      if (g) g.style.display = "";
      const hitCard = under && under.closest ? under.closest("#widgetBoard .sq-widget[data-widget-id]") : null;
      if (hitCard && hitCard.dataset.widgetId !== widgetDragging.wid) {
        const r = hitCard.getBoundingClientRect();
        const after = e.clientX > r.left + r.width / 2;
        hitCard.classList.add(after ? "drop-after" : "drop-before");
        widgetDragging.targetId = hitCard.dataset.widgetId;
        widgetDragging.insertAfter = after;
      } else {
        widgetDragging.targetId = null;
      }
    });

    const finishWidgetDrag = async (e) => {
      if (!widgetDragging) return;
      const st = widgetDragging;
      widgetDragging = null;
      if (st.ghost) st.ghost.remove();
      st.srcEl.classList.remove("dragging-src");
      $$("#widgetBoard .sq-widget").forEach((c) => c.classList.remove("drop-before", "drop-after"));
      $("#widgetLib")?.classList.remove("drop-remove");
      if (!st.moved) return;

      let list = [...activeWidgetIds()];
      if (!st.fromLib && st.overLib) {
        // Kéo từ Bảng Widget thả ngược vào Thư viện → cất Widget
        list = list.filter((id) => id !== st.wid);
        settingsCache.active_widgets = list;
        await api("save_settings", { settings: settingsCache });
        renderWidgetBoardDom();
        toast("Đã cất Widget vào Thư viện");
        return;
      }
      const boardEl = $("#widgetBoard");
      const br = boardEl ? boardEl.getBoundingClientRect() : null;
      const inBoard = br && e && e.clientX >= br.left - 20 && e.clientX <= br.right + 20 && e.clientY >= br.top - 20 && e.clientY <= br.bottom + 40;
      if (!inBoard && st.fromLib) return;

      list = list.filter((id) => id !== st.wid);
      if (st.targetId && list.includes(st.targetId)) {
        const idx = list.indexOf(st.targetId);
        list.splice(st.insertAfter ? idx + 1 : idx, 0, st.wid);
      } else {
        list.push(st.wid);
      }
      settingsCache.active_widgets = list;
      await api("save_settings", { settings: settingsCache });
      renderWidgetBoardDom();
      if (st.fromLib) toast("✨ Đã thêm Widget vào bảng!");
    };
    window.addEventListener("pointerup", finishWidgetDrag);
    window.addEventListener("pointercancel", finishWidgetDrag);
  })();

  $("#view").addEventListener("click", async (e) => {
    const icBtn = e.target.closest("[data-icon-size]");
    if (icBtn) {
      settingsCache.icon_size = icBtn.dataset.iconSize;
      applyIconSize();
      await api("save_settings", { settings: settingsCache });
      if (tab === "all") renderWidgetBoardDom();
      else if (tab === "settings") viewSettings();
      return;
    }
    const wact = e.target.closest("[data-wact]");
    if (wact) {
      e.stopPropagation();
      const act = wact.dataset.wact;
      const wid = wact.dataset.wid;
      if (act === "img") {
        pickingImgForWidget = wid;
        const inp = $("#widgetImgPicker");
        if (inp) { inp.value = ""; inp.click(); }
        return;
      }
      if (act === "reset-img") {
        if (settingsCache.widget_images) delete settingsCache.widget_images[wid];
        await api("save_settings", { settings: settingsCache });
        renderWidgetBoardDom();
        toast("Đã khôi phục hình minh hoạ mặc định");
        return;
      }
      if (act === "remove") {
        settingsCache.active_widgets = activeWidgetIds().filter((x) => x !== wid);
        await api("save_settings", { settings: settingsCache });
        renderWidgetBoardDom();
        toast("Đã cất Widget vào Thư viện (bấm “Thư viện Widget” để kéo ra lại)");
        return;
      }
      if (act === "add") {
        const cur = activeWidgetIds();
        if (!cur.includes(wid)) {
          settingsCache.active_widgets = [...cur, wid];
          await api("save_settings", { settings: settingsCache });
          renderWidgetBoardDom();
          toast("✨ Đã thêm Widget vào bảng!");
        }
        return;
      }
    }
    const wcmd = e.target.closest("[data-wcmd]");
    if (wcmd) {
      e.stopPropagation();
      const cmd = wcmd.dataset.wcmd;
      if (cmd === "open-settings") { openSettingsWindow(); return; }
      if (cmd === "open-cam") { await togglePeekCam(); return; }
      if (cmd === "open-words") { switchTab("words"); return; }
      if (cmd === "word-speak") { api("vocab_play", { word: wcmd.dataset.word || "Resilient" }); return; }
      if (cmd.startsWith("media-")) {
        const mAct = cmd === "media-mute" ? "mute_out" : cmd.slice(6);
        mediaQuick = await api("media_control", { action: mAct });
        renderWidgetBoardDom();
        return;
      }
      if (cmd === "toggle-focus") {
        if (status && status.focus_until) await api("focus_stop");
        else await api("focus_start", { minutes: 25 });
        status = await api("get_status");
        renderWidgetBoardDom();
        updateBar();
        return;
      }
      if (cmd === "rest-eyes") {
        setMode("collapsed");
        setTimeout(() => showAlert({ kind: "break", minutes: status ? status.session_minutes : 20 }), 350);
        return;
      }
      if (cmd === "snap") {
        try {
          const msg = await api("window_snap", { action: wcmd.dataset.snap, pid: null, prevPid: null });
          toast("🪟 " + msg);
          setMode("collapsed");
        } catch (err) {
          toast("⚠ " + String(err));
        }
        return;
      }
    }
    const g = e.target.closest("[data-go]");
    if (!g || (tab !== "all" && g.dataset.go !== "perm")) return;
    if (widgetDragging && widgetDragging.moved) return;
    const targetGo = g.dataset.go;
    if (!targetGo || targetGo.endsWith("-noop")) return;
    if (targetGo === "perm") { openSettingsWindow("permCard"); return; }
    if (targetGo === "settings") { openSettingsWindow(); return; }
    switchTab(targetGo);
  });

  // ═════════ VIỆC ═════════
  let showDone = false;
  async function refreshTasks() {
    tasks = await api("list_tasks");
    if (mode === "expanded" && tab === "tasks") drawTaskList();
    if (mode === "peek") renderPeek();
  }
  function viewTasks() {
    api("list_tasks").then((t) => { tasks = t; if (tab === "tasks" && mode === "expanded") drawTaskList(); }).catch(() => {});
    $("#view").innerHTML = `
      <div class="add-wrap">
        <div class="add-row">${I.plus}<input class="input" id="taskInput" placeholder="Thêm việc… vd: mai 9h gọi khách · tomorrow 3pm call John" autocomplete="off" />
          <div class="add-tools">${langPill()}${micBtn("taskMic")}</div></div>
        <div class="preview" id="preview"></div>
      </div>
      <div id="eventList"></div>
      <div id="taskList"></div>`;
    const inp = $("#taskInput");
    $("#taskMic").addEventListener("click", (e) => dictate(e.currentTarget, (text) => {
      inp.value = text;
      inp.dispatchEvent(new Event("input"));
      inp.focus();
      toast("Nhấn Enter để thêm việc");
    }));
    drawEvents();
    let deb;
    inp.addEventListener("input", () => {
      clearTimeout(deb);
      deb = setTimeout(async () => {
        const v = inp.value.trim();
        if (!v) { $("#preview").innerHTML = ""; return; }
        const p = await api("parse_task", { text: v });
        $("#preview").innerHTML = p.due
          ? `<span class="chip green">${I.clock}${fmtFull(p.due)}</span><span class="muted small">${esc(p.title)}</span>`
          : `<span class="chip">Không hẹn giờ</span><span class="muted small">Enter để thêm</span>`;
      }, 120);
    });
    inp.addEventListener("keydown", async (e) => {
      if (e.key !== "Enter" || isComposing(e) || !inp.value.trim()) return;
      try {
        const t = await api("add_task", { text: inp.value });
        inp.value = "";
        $("#preview").innerHTML = `<span class="chip green">${I.check}Đã thêm${t.due ? " · " + fmtDue(t.due) : ""}</span>`;
        flashMood("happy", 1200);
        await refreshTasks();
      } catch (err) { toast(String(err)); }
    });
    setTimeout(() => inp.focus(), 80);
    drawTaskList();
  }
  async function drawEvents() {
    const box = $("#eventList");
    if (!box || !settingsCache.show_calendar) return;
    const evs = await api("get_events");
    const now = Date.now();
    const end = new Date(); end.setHours(23, 59, 59, 999);
    const today = evs.filter((e) => e.end > now && e.start <= end.getTime());
    box.innerHTML = today.length ? `<div class="sec" style="color:var(--blue)">${I.cal} Lịch hôm nay · ${today.length}</div>` + today.map((e) => `
      <div class="task event"><span class="ev-dot"></span><div class="title">${esc(e.title)}<span class="muted small"> ${e.location ? "· " + esc(e.location) : ""}</span></div>
      <span class="chip ${e.start <= now ? "green" : "blue"}">${e.all_day ? "Cả ngày" : e.start <= now ? "Đang diễn ra" : hm(new Date(e.start)) + "–" + hm(new Date(e.end))}</span></div>`).join("") : "";
  }
  function taskRow(t, now) {
    const late = !t.done_at && t.due && t.due < now;
    const dueTxt = t.due ? `<span class="task-due ${late ? "late" : ""}">${fmtDue(t.due, now)}</span>` : "";
    return `<div class="task ${t.done_at ? "done" : ""} ${late ? "late" : ""}" data-id="${t.id}">
      <button class="check" data-t="toggle">${I.check}</button>
      <div class="task-info">
        <div class="title" data-t="title" title="Nhấp đúp để sửa">${esc(t.title)}</div>
        ${dueTxt}
      </div>
      <div class="acts">
        ${t.done_at ? "" : `<button class="icon-btn" data-t="s60" title="Hoãn 1 giờ">+1h</button>
        <button class="icon-btn" data-t="stm" title="Chuyển sang mai">Mai</button>`}
        <button class="icon-btn" data-t="del" title="Xoá">${I.trash}</button>
      </div></div>`;
  }
  function drawTaskList() {
    const box = $("#taskList");
    if (!box) return;
    const now = Date.now();
    const open = tasks.filter((t) => !t.done_at);
    const done = tasks.filter((t) => t.done_at).sort((a, b) => b.done_at - a.done_at);
    const by = (a, b) => (a.due ?? 9e15) - (b.due ?? 9e15);
    const endToday = new Date(); endToday.setHours(23, 59, 59, 999);
    const groups = [
      ["Trễ hạn", open.filter((t) => t.due && t.due < now).sort(by), "red"],
      ["Hôm nay", open.filter((t) => t.due && t.due >= now && t.due <= endToday).sort(by), ""],
      ["Sắp tới", open.filter((t) => t.due && t.due > endToday).sort(by), ""],
      ["Không hạn", open.filter((t) => !t.due).sort((a, b) => b.created - a.created), ""],
    ];
    let html = "";
    for (const [name, list, cls] of groups) {
      if (!list.length) continue;
      html += `<div class="sec ${cls}">${name} · ${list.length}</div>` + list.map((t) => taskRow(t, now)).join("");
    }
    if (!open.length) html += `<div class="empty"><div class="big">(˶ᵔ ᵕ ᵔ˶)</div>Không còn việc nào. Thêm việc ở ô bên trên nhé!</div>`;
    if (done.length) {
      html += `<div class="sec" style="cursor:pointer" data-t="showdone">Đã xong · ${done.length} ${showDone ? "▾" : "▸"}
        ${showDone ? '<button class="btn sm ghost" data-t="cleardone" style="margin-left:auto">Xoá hết</button>' : ""}</div>`;
      if (showDone) html += done.slice(0, 30).map((t) => taskRow(t, now)).join("");
    }
    box.innerHTML = html;
  }
  $("#view").addEventListener("click", async (e) => {
    if (tab !== "tasks") return;
    const b = e.target.closest("[data-t]");
    if (!b) return;
    const act = b.dataset.t;
    if (act === "showdone") { showDone = !showDone; drawTaskList(); return; }
    if (act === "cleardone") { e.stopPropagation(); await api("clear_done"); await refreshTasks(); return; }
    const row = b.closest(".task");
    if (!row) return;
    const id = Number(row.dataset.id);
    if (act === "toggle") {
      const t = await api("toggle_task", { id });
      if (t && t.done_at) {
        row.classList.add("done");
        confetti(16);
        flashMood("happy");
        await new Promise((r) => setTimeout(r, 450));
      }
    }
    if (act === "s10") await api("snooze_task", { id, minutes: 10 });
    if (act === "s60") await api("snooze_task", { id, minutes: 60 });
    if (act === "stm") await api("snooze_task", { id, minutes: -1 });
    if (act === "del") await api("delete_task", { id });
    if (act === "cal") {
      try { await api("add_to_calendar", { id }); toast("📅 Đã thêm vào Lịch"); drawEvents(); } catch (err) { toast(String(err)); }
      return;
    }
    if (act !== "title") await refreshTasks();
  });
  $("#view").addEventListener("dblclick", (e) => {
    const el = e.target.closest('.task [data-t="title"]');
    if (!el || tab !== "tasks") return;
    el.contentEditable = "true";
    el.focus();
    document.getSelection().selectAllChildren(el);
    const done = async (save) => {
      el.contentEditable = "false";
      if (save && el.textContent.trim()) await api("rename_task", { id: Number(el.closest(".task").dataset.id), title: el.textContent.trim() });
      await refreshTasks();
    };
    el.onkeydown = (k) => { if (isComposing(k)) return; if (k.key === "Enter") { k.preventDefault(); done(true); } if (k.key === "Escape") { k.stopPropagation(); done(false); } };
    el.onblur = () => done(true);
  });

  // ═════════ GHI CHÚ ═════════
  let notes = [];
  let noteId = null;
  let noteQuery = "";
  async function viewNotes() {
    notes = await api("list_notes");
    if (noteId == null && notes.length) noteId = notes[0].id;
    const cur = notes.find((n) => n.id === noteId);
    $("#view").innerHTML = `<div class="notes">
      <div class="note-list">
        <div class="row" style="margin-bottom:4px">
          <input class="input" id="noteSearch" placeholder="Tìm…" value="${esc(noteQuery)}" style="height:30px" />
          <button class="icon-btn" data-n="new" title="Ghi chú mới">${I.plus}</button>
        </div>
        <div id="noteItems"></div>
      </div>
      <div class="editor">
        <textarea id="noteText" placeholder="Ghi nhanh điều gì đó… (dòng đầu là tiêu đề)">${esc(cur ? cur.text : "")}</textarea>
        <div class="foot">
          ${micBtn("noteMic")}${langPill()}
          <span class="muted small grow" id="noteState">${cur ? "Đã lưu" : "Gõ hoặc nói để tạo ghi chú mới"}</span>
          <span id="noteActs" style="display:${cur ? "contents" : "none"}"><button class="icon-btn ${cur && cur.pinned ? "on" : ""}" data-n="pin" title="Ghim">${I.pin}</button>
          <button class="icon-btn" data-n="copy" title="Sao chép">${I.copy}</button>
          <button class="icon-btn" data-n="del" title="Xoá">${I.trash}</button></span>
        </div>
      </div></div>`;
    drawNoteItems();
    const ta = $("#noteText");
    let deb;
    const save = () => {
      clearTimeout(deb);
      deb = setTimeout(async () => {
        if (composing) return save(); // chờ bộ gõ ghép xong chữ
        const n = await api("save_note", { id: noteId, text: ta.value });
        noteId = n.id;
        notes = await api("list_notes");
        drawNoteItems();
        $("#noteState").textContent = "Đã lưu";
        $("#noteActs").style.display = "contents"; // KHÔNG vẽ lại ô soạn thảo → không mất chữ đang gõ
      }, 500);
    };
    ta.addEventListener("input", () => { $("#noteState").textContent = "Đang gõ…"; save(); });
    ta.addEventListener("compositionend", save);
    $("#noteSearch").addEventListener("input", (e) => { noteQuery = e.target.value; drawNoteItems(); });
    $("#noteMic").addEventListener("click", (e) => dictate(e.currentTarget, (text) => {
      const ta2 = $("#noteText");
      if (!ta2) return;
      const pos = ta2.selectionStart ?? ta2.value.length;
      const before = ta2.value.slice(0, pos);
      const sep = before && !/\s$/.test(before) ? " " : "";
      ta2.value = before + sep + text + ta2.value.slice(pos);
      ta2.dispatchEvent(new Event("input"));
      ta2.focus();
    }));
    setTimeout(() => ta.focus(), 80);
  }

  function drawNoteItems() {
    const q = noteQuery.trim().toLowerCase();
    const list = notes.filter((n) => !q || n.text.toLowerCase().includes(q));
    $("#noteItems").innerHTML = list.length
      ? list.map((n) => {
          const [first, ...rest] = n.text.split("\n");
          return `<div class="note-item ${n.id === noteId ? "active" : ""}" data-n="open" data-id="${n.id}">
            <b>${n.pinned ? "📌 " : ""}${esc(first || "Ghi chú trống")}</b>
            <span>${fmtAgo(n.updated)} · ${esc(rest.join(" ").slice(0, 30))}</span></div>`;
        }).join("")
      : `<div class="empty small">Chưa có ghi chú</div>`;
  }
  $("#view").addEventListener("click", async (e) => {
    if (tab !== "notes") return;
    const b = e.target.closest("[data-n]");
    if (!b) return;
    const a = b.dataset.n;
    if (a === "open") { noteId = Number(b.dataset.id); viewNotes(); }
    if (a === "new") { noteId = null; $("#noteText").value = ""; $("#noteText").focus(); $("#noteState").textContent = "Gõ để tạo ghi chú mới"; $("#noteActs").style.display = "none"; $$(".note-item").forEach((x) => x.classList.remove("active")); }
    if (a === "pin") { await api("pin_note", { id: noteId }); viewNotes(); }
    if (a === "copy") { try { await navigator.clipboard.writeText($("#noteText").value); toast("Đã sao chép"); } catch { toast("Không sao chép được"); } }
    if (a === "del") { await api("delete_note", { id: noteId }); noteId = null; viewNotes(); }
  });

  // ═════════ FILE ═════════
  let files = [];
  let fileResults = [];
  let busy = false;
  async function addFiles(paths) {
    const info = await api("file_info", { paths });
    const known = new Set(files.map((f) => f.path));
    files.push(...info.filter((f) => !known.has(f.path)));
  }
  function fileIcon(f) {
    return f.is_pdf ? I.pdf : f.is_archive ? I.zip : f.is_dir ? I.folder : I.file;
  }
  async function viewFiles() {
    const settings = await api("get_settings");
    const pdfs = files.filter((f) => f.is_pdf);
    const arcs = files.filter((f) => f.is_archive);
    $("#view").innerHTML = `
      ${files.length ? `
        <div class="row"><div class="sec grow" style="margin-top:2px">${files.length} mục đã chọn</div>
          <button class="btn sm ghost" data-f="pick">${I.plus}Thêm</button><button class="btn sm ghost" data-f="clear">Bỏ chọn</button></div>
        <div class="flist">${files.map((f, i) => `<div class="fitem"><div class="ic">${fileIcon(f)}</div>
          <div class="grow"><div class="nm">${esc(f.name)}</div><div class="muted small">${f.is_dir ? "Thư mục" : bytes(f.size)}</div></div>
          <button class="icon-btn" data-f="rm" data-i="${i}">${I.x}</button></div>`).join("")}</div>
        <div class="actions">
          <button class="btn blue" data-f="airdrop" title="Bắn file nhanh qua AirDrop sang iPhone, iPad, Mac">${I.airdrop}AirDrop</button>
          <button class="btn primary" data-f="zip" ${busy ? "disabled" : ""}>${I.zip}Nén ZIP</button>
          <button class="btn" data-f="copy" title="Chép vào clipboard để dán ra Finder, Mail, Zalo…">${I.clip}Chép file</button>
          ${arcs.length ? `<button class="btn" data-f="extract" ${busy ? "disabled" : ""}>${I.unzip}Giải nén${arcs.length > 1 ? ` (${arcs.length})` : ""}</button>` : ""}
          ${pdfs.length ? `<button class="btn" data-f="pdf" ${busy ? "disabled" : ""}>${I.pdf}Nén PDF${pdfs.length > 1 ? ` (${pdfs.length})` : ""}</button>
            <select class="input" id="pdfQ">
              <option value="screen" ${settings.pdf_quality === "screen" ? "selected" : ""}>Nhỏ nhất</option>
              <option value="ebook" ${settings.pdf_quality === "ebook" ? "selected" : ""}>Cân bằng</option>
              <option value="printer" ${settings.pdf_quality === "printer" ? "selected" : ""}>Đẹp (in ấn)</option></select>` : ""}
          ${pdfs.length >= 2 ? `<button class="btn" data-f="merge" ${busy ? "disabled" : ""}>${I.merge}Gộp ${pdfs.length} PDF</button>` : ""}
          ${busy ? '<span class="spin"></span>' : ""}
        </div>`
      : `<div class="drop-area">${I.file}<b style="color:#fff">Kéo file thả vào tai thỏ</b>
          <span class="small">Thả nửa trái: <b>AirDrop ngay</b> · Thả nửa phải: <b>Nén ZIP / Giải nén / PDF</b></span>
          <div class="row" style="margin-top:4px"><button class="btn blue" data-f="airdrop-pick">${I.airdrop}Chọn file gửi AirDrop…</button><button class="btn" data-f="pick">Chọn file nén / PDF…</button></div></div>`}
      ${fileResults.length ? `<div class="sec">Kết quả</div>` + fileResults.map((r) => `
        <div class="card result ${r.error ? "err" : ""}" style="margin-bottom:6px">
          <div class="row"><div class="grow">
            <b>${esc(r.error ? r.title : r.output.split(/[\\/]/).pop())}</b>
            <div class="muted small">${esc(r.error || r.message)}${r.error ? "" : ` · ${bytes(r.before)} → ${bytes(r.after)}`}</div></div>
            ${r.error ? "" : `<button class="btn sm" data-f="reveal" data-p="${esc(r.output)}">${I.folder}Mở</button>`}</div></div>`).join("") : ""}`;
  }
  async function runFile(title, fn) {
    busy = true;
    viewFiles();
    flashMood("focus", 60000);
    try {
      const r = await fn();
      fileResults.unshift(r);
      flashMood("happy");
      confetti(12);
    } catch (err) {
      fileResults.unshift({ error: String(err), title });
      flashMood("worried", 2500);
    }
    fileResults = fileResults.slice(0, 8);
    busy = false;
    if (tab === "files") viewFiles();
  }
  $("#view").addEventListener("click", async (e) => {
    if (tab !== "files") return;
    const b = e.target.closest("[data-f]");
    if (!b) return;
    const a = b.dataset.f;
    if (a === "pick") { const p = await api("pick_files"); if (p.length) { await addFiles(p); viewFiles(); } }
    if (a === "airdrop-pick") {
      const p = await api("pick_files");
      if (p.length) { try { await api("file_airdrop", { paths: p }); } catch (err) { toast(String(err)); } }
      return;
    }
    if (a === "airdrop") {
      try { await api("file_airdrop", { paths: files.map((f) => f.path) }); } catch (err) { toast(String(err)); }
      return;
    }
    if (a === "clear") { files = []; viewFiles(); }
    if (a === "rm") { files.splice(Number(b.dataset.i), 1); viewFiles(); }
    if (a === "reveal") api("reveal", { path: b.dataset.p });
    if (a === "copy") { await api("clip_copy_files", { paths: files.map((f) => f.path) }); toast("Đã chép — ⌘V để dán ở bất kỳ đâu"); }
    if (a === "zip") runFile("Nén ZIP", () => api("file_zip", { paths: files.map((f) => f.path) }));
    if (a === "extract") for (const f of files.filter((f) => f.is_archive)) await runFile("Giải nén " + f.name, () => api("file_extract", { path: f.path }));
    if (a === "pdf") {
      const q = $("#pdfQ").value;
      for (const f of files.filter((f) => f.is_pdf)) await runFile("Nén " + f.name, () => api("file_compress_pdf", { path: f.path, quality: q }));
    }
    if (a === "merge") runFile("Gộp PDF", () => api("file_merge_pdf", { paths: files.filter((f) => f.is_pdf).map((f) => f.path) }));
  });

  // ═════════ DỌN MÁY ═════════
  let scan = null; // {report, disk, pro}
  let scanning = false;
  let cleanSel = { rules: new Set(), paths: new Set() };
  let openRules = new Set();
  let cleanResult = null;
  let confirmClean = false;

  function ring(d) {
    if (!d) return "";
    const used = 1 - d.free_percent / 100;
    const C = 2 * Math.PI * 26;
    const col = d.level === "critical" ? "var(--red)" : d.level === "warn" ? "var(--amber)" : "var(--green)";
    return `<div class="ring"><svg viewBox="0 0 64 64"><circle cx="32" cy="32" r="26" stroke="#2a2a2e"/>
      <circle cx="32" cy="32" r="26" stroke="${col}" stroke-dasharray="${C}" stroke-dashoffset="${C * (1 - used)}"/></svg>
      <div class="lbl">${Math.round(d.free_percent)}%</div></div>`;
  }
  function selectedBytes() {
    if (!scan) return 0;
    let n = 0;
    for (const r of scan.report.rules) {
      if (r.locked) continue;
      for (const it of r.items) if (cleanSel.rules.has(r.id) || cleanSel.paths.has(it.path)) n += it.bytes;
    }
    return n;
  }
  async function viewClean() {
    const disk = scan ? scan.disk : status && status.disk;
    const head = `<div class="card disk-head">${ring(disk)}
      <div class="grow">${disk ? `<div class="big-num">${bytes(disk.free_bytes)} <span class="muted" style="font-size:13px;font-weight:500">trống / ${bytes(disk.total_bytes)}</span></div>
        <div class="muted small">${disk.level === "ok" ? "Ổ đĩa còn thoải mái" : disk.level === "warn" ? "Ổ đĩa sắp đầy — nên dọn bớt" : "Ổ đĩa gần hết chỗ!"}</div>` : "<b>Ổ đĩa</b>"}</div>
      <button class="btn ${scan ? "" : "primary"}" data-c="scan" ${scanning ? "disabled" : ""}>${scanning ? '<span class="spin"></span>Đang quét…' : scan ? "Quét lại" : `${I.search}Quét máy`}</button></div>`;

    if (cleanResult) {
      const r = cleanResult;
      $("#view").innerHTML = head + `<div class="empty"><div class="big">٩(◕‿◕)۶</div>
        <div class="big-num" style="color:var(--green)">Đã giải phóng ${bytes(r.deleted_bytes)}</div>
        <div class="muted">${r.trashed_bytes ? `${bytes(r.trashed_bytes)} file của bạn đã vào Thùng rác (hoàn tác được) · ` : ""}${r.items_done} mục</div>
        ${r.failures.length ? `<div class="chip amber" style="margin-top:8px">${r.failures.length} mục không xoá được (thiếu quyền hoặc đang dùng)</div>` : ""}
        <div style="margin-top:12px"><button class="btn" data-c="again">Xong</button></div></div>`;
      return;
    }
    if (!scan) {
      $("#view").innerHTML = head + `<div class="empty"><div class="big">${I.broom}</div>
        Quét để tìm cache, log, file cài đặt cũ, thư mục build…<br><span class="small">Quét chỉ đọc, không xoá gì cho tới khi bạn bấm Dọn.</span></div>`;
      return;
    }
    const rep = scan.report;
    const rules = rep.rules.filter((r) => r.bytes > 0);
    const rows = rules.map((r) => {
      const all = r.items.every((it) => cleanSel.paths.has(it.path));
      const some = r.items.some((it) => cleanSel.paths.has(it.path));
      const st = r.locked ? "" : cleanSel.rules.has(r.id) || all ? "on" : some ? "part" : "";
      const tag = r.locked ? `<span class="chip violet">${I.lock}Pro</span>` : r.level === "safe" ? `<span class="chip green">An toàn</span>` : `<span class="chip amber">Xem lại</span>`;
      const trashNote = r.action === "trash" ? `<span class="chip" title="Chuyển vào Thùng rác">${I.trash}</span>` : "";
      const open = openRules.has(r.id);
      return `<div class="rule ${r.locked ? "locked" : ""}">
        <div class="rule-head">
          <button class="box ${st}" data-c="rule" data-id="${r.id}" ${r.locked ? "disabled" : ""}>${I.check}</button>
          <span class="expand ${open ? "open" : ""}" data-c="open" data-id="${r.id}">${I.chevron}</span>
          <span class="nm" data-c="open" data-id="${r.id}">${esc(r.name)}</span>
          ${trashNote}${tag}<span class="muted small">${r.items.length}</span><span class="sz">${bytes(r.bytes)}</span>
        </div>
        ${open ? `<div class="rule-items">${r.items.slice(0, 40).map((it) => `<div class="ritem">
            <button class="box ${cleanSel.rules.has(r.id) || cleanSel.paths.has(it.path) ? "on" : ""}" data-c="item" data-rule="${r.id}" data-p="${esc(it.path)}" ${r.locked ? "disabled" : ""}>${I.check}</button>
            <span class="p" title="${esc(it.path)}">&lrm;${esc(shortPath(it.path))}</span>
            <span class="muted">${it.age_days} ngày</span><b>${bytes(it.bytes)}</b></div>`).join("")}
            ${r.items.length > 40 ? `<div class="muted small">… và ${r.items.length - 40} mục khác</div>` : ""}</div>` : ""}
      </div>`;
    }).join("");
    const sel = selectedBytes();
    const fda = rep.skipped.some((s) => s.reason.includes("Full Disk Access"));
    $("#view").innerHTML = head + `
      <div class="row" style="margin:12px 2px 4px"><div class="grow"><div class="muted small">Có thể giải phóng</div>
        <div class="big-num">${bytes(rep.available_bytes)}</div></div>
        <button class="btn sm ghost" data-c="safe">Chọn mục an toàn</button><button class="btn sm ghost" data-c="none">Bỏ chọn</button></div>
      ${rows || '<div class="empty">Ổ đĩa sạch sẽ, không có file rác</div>'}
      ${rep.locked_bytes > 0 ? `<div class="upsell">${I.sparkle}<div class="grow"><b>Bản Pro dọn thêm được ${bytes(rep.locked_bytes)}</b>
        <div class="muted small">File cài đặt cũ, thư mục build, file lớn, ảnh chụp màn hình cũ, bản sao lưu iPhone…</div></div>
        <button class="btn sm" data-c="pro">Nâng cấp</button></div>` : ""}
      ${rep.skipped.length ? `<div class="muted small" style="margin:10px 4px">Đã bỏ qua ${rep.skipped.length} mục để an toàn (đang được dùng, dự án mới sửa…).
        ${fda && geo.platform === "macos" ? `<button class="btn sm ghost" data-c="fda">Cấp quyền Full Disk Access</button>` : ""}</div>` : ""}
      <div class="sticky-foot">
        ${confirmClean
          ? `<span class="grow small">Cache sẽ bị <b>xoá hẳn</b> (app tự tạo lại). File của bạn vào <b>Thùng rác</b>.</span>
             <button class="btn ghost" data-c="cancel">Huỷ</button><button class="btn red" data-c="go">Dọn ngay ${bytes(sel)}</button>`
          : `<span class="grow muted small">Đã chọn ${bytes(sel)}</span>
             <button class="btn primary" data-c="ask" ${sel ? "" : "disabled"}>${I.broom}Dọn ${bytes(sel)}</button>`}
      </div>`;
  }
  function selectSafe() {
    cleanSel = { rules: new Set(), paths: new Set() };
    for (const r of scan.report.rules) if (!r.locked && r.level === "safe" && r.id !== "system-trash" && r.bytes > 0) cleanSel.rules.add(r.id);
  }
  $("#view").addEventListener("click", async (e) => {
    if (tab !== "clean") return;
    const b = e.target.closest("[data-c]");
    if (!b) return;
    const a = b.dataset.c;
    const rule = scan && scan.report.rules.find((r) => r.id === b.dataset.id || r.id === b.dataset.rule);
    if (a === "scan") {
      scanning = true; cleanResult = null; confirmClean = false;
      viewClean();
      moodOverride = "focus"; applyMood();
      try { scan = await api("clean_scan"); selectSafe(); } catch (err) { toast(String(err)); }
      scanning = false; moodOverride = null; applyMood();
      if (scan && scan.report.available_bytes > 2e9) flashMood("worried", 2500);
    }
    if (a === "rule" && rule) {
      const isOn = cleanSel.rules.has(rule.id) || rule.items.every((it) => cleanSel.paths.has(it.path));
      cleanSel.rules.delete(rule.id);
      rule.items.forEach((it) => cleanSel.paths.delete(it.path));
      if (!isOn) cleanSel.rules.add(rule.id);
    }
    if (a === "item" && rule) {
      if (cleanSel.rules.has(rule.id)) { cleanSel.rules.delete(rule.id); rule.items.forEach((it) => cleanSel.paths.add(it.path)); }
      const p = b.dataset.p;
      cleanSel.paths.has(p) ? cleanSel.paths.delete(p) : cleanSel.paths.add(p);
    }
    if (a === "open") { const id = b.dataset.id; openRules.has(id) ? openRules.delete(id) : openRules.add(id); }
    if (a === "safe") selectSafe();
    if (a === "none") cleanSel = { rules: new Set(), paths: new Set() };
    if (a === "ask") confirmClean = true;
    if (a === "cancel") confirmClean = false;
    if (a === "pro") { openSettingsWindow(); return; }
    if (a === "fda") { api("open_disk_settings"); return; }
    if (a === "again") { cleanResult = null; scan = null; }
    if (a === "go") {
      confirmClean = false;
      scanning = true;
      viewClean();
      try {
        cleanResult = await api("clean_run", { rules: [...cleanSel.rules], paths: [...cleanSel.paths] });
        confetti(40);
        flashMood("love", 4000);
        status = await api("get_status");
        if (scan) scan.disk = status.disk;
      } catch (err) { toast(String(err)); }
      scanning = false;
    }
    viewClean();
  });

  // ═════════ SỨC KHOẺ ═════════
  async function viewHealth() {
    const s = status || (await api("get_status"));
    const st = await api("get_settings");
    const pct = Math.min(100, (s.session_minutes / s.break_minutes) * 100);
    const focusLeft = s.focus_until ? Math.max(0, Math.round((s.focus_until * 1000 - Date.now()) / 60000)) : 0;
    $("#view").innerHTML = `
      <div class="grid2">
        <div class="card"><div class="muted small">Phiên hiện tại</div><div class="big-num">${mins(s.session_minutes)}</div>
          <div class="meter"><i style="width:${pct}%;${pct >= 100 ? "background:var(--amber)" : ""}"></i></div>
          <div class="row" style="margin-top:8px"><span class="muted small grow">${pct >= 100 ? "Đến giờ nghỉ rồi!" : `Nghỉ sau ${s.break_minutes - s.session_minutes} phút`}</span>
          <button class="btn sm" data-h="rested">Mình vừa nghỉ</button></div></div>
        <div class="card"><div class="muted small">Hôm nay đã dùng máy</div><div class="big-num">${mins(s.today_minutes)}</div>
          <div class="muted small" style="margin-top:12px">${s.sleepy ? "Đã khuya rồi, hãy nghỉ ngơi sớm" : "Đứng dậy vươn vai mỗi tiếng nhé"}</div></div>
      </div>
      <div class="card" style="margin-top:10px"><div class="row">
        <div class="grow"><b>${I.target} Hẹn giờ tập trung</b><div class="muted small">${s.focus_until ? `Còn ${focusLeft} phút — tai thỏ chuyển tím, tạm tắt nhắc nhở` : "Làm việc sâu, không bị nhắc nhở làm phiền"}</div></div>
        ${s.focus_until ? `<button class="btn red" data-h="stop">Dừng</button>`
          : `<button class="btn" data-h="f25">25 phút</button><button class="btn" data-h="f50">50 phút</button><button class="btn" data-h="f90">90 phút</button>`}
      </div></div>
      <div class="card" style="margin-top:10px">
        <div class="setting"><div class="grow"><b>Nhắc nghỉ mắt</b><span>Sau mỗi phiên dùng máy liên tục</span></div>
          <input class="input num" type="number" min="10" max="180" value="${st.break_minutes}" data-k="break_minutes" /><span class="muted small">phút</span>
          <button class="switch ${st.break_enabled ? "on" : ""}" data-k="break_enabled"></button></div>
        <div class="setting"><div class="grow"><b>Nhắc uống nước</b><span>Linh vật vẫy nhẹ, không làm phiền</span></div>
          <input class="input num" type="number" min="15" max="240" value="${st.water_minutes}" data-k="water_minutes" /><span class="muted small">phút</span>
          <button class="switch ${st.water_enabled ? "on" : ""}" data-k="water_enabled"></button></div>
        <div class="setting"><div class="grow"><b>Giờ đi ngủ</b><span>Sau giờ này linh vật sẽ ngáp ngủ</span></div>
          <input class="input num" type="number" min="18" max="23" value="${st.sleep_hour}" data-k="sleep_hour" /><span class="muted small">giờ</span></div>
      </div>`;
  }
  $("#view").addEventListener("click", async (e) => {
    if (tab !== "health") return;
    const b = e.target.closest("[data-h]");
    if (!b) return;
    const a = b.dataset.h;
    if (a === "rested") { await api("health_break_done"); flashMood("happy"); }
    if (a === "stop") await api("focus_stop");
    if (a[0] === "f") { await api("focus_start", { minutes: Number(a.slice(1)) }); flashMood("focus", 2000); }
    status = await api("get_status");
    applyColor(); updateBar(); viewHealth();
  });

  // Lưu cài đặt chung (dùng ở tab Sức khoẻ & Cài đặt)
  $("#view").addEventListener("click", async (e) => {
    const sw = e.target.closest(".switch[data-k]");
    if (!sw) return;
    const st = await api("get_settings");
    st[sw.dataset.k] = !st[sw.dataset.k];
    await api("save_settings", { settings: st });
    settingsCache = st;
    sw.classList.toggle("on", st[sw.dataset.k]);
    if (["sync_reminders", "sync_notes", "show_calendar"].includes(sw.dataset.k) && st[sw.dataset.k]) {
      toast("macOS có thể hỏi quyền — bấm Cho phép nhé");
      api("sync_now").then(() => tab === "settings" && viewSettings());
    }
    status = await api("get_status");
    applyColor(); applyMood(); updateBar();
  });
  // ───────── Cài đặt › Giọng đọc của Asi (Asi Neural Studio / Thanh Âm Nhanh) ─────────
  function voiceCard(tv) {
    const voiceBtn = (engine, ready, [name, g, desc]) => {
      const on = tv.engine === engine && tv.voice === name;
      return `<button class="voice-opt ${on ? "on" : ""}" data-tts="use" data-engine="${engine}" data-voice="${esc(name)}">
        <span class="voice-av ${g}">${g === "female" ? "Nữ" : "Nam"}</span><span class="grow"><b>${esc(name)}</b><span class="muted small">${esc(desc)}</span></span>
        <span class="icon-btn" data-tts="hear" data-engine="${engine}" data-voice="${esc(name)}" title="Nghe thử">▶</span></button>`;
    };
    const sysOn = tv.engine === "system";
    const sysOpt = `<button class="voice-opt ${sysOn ? "on" : ""}" data-tts="use" data-engine="system" data-voice=""><span class="voice-av">Máy</span><span class="grow"><b>Giọng mặc định macOS</b><span class="muted small">Linh / Samantha · offline</span></span>
        <span class="icon-btn" data-tts="hear" data-engine="system" data-voice="" title="Nghe thử">▶</span></button>`;
    const qs = tv.qualities || [["compact", "Gọn · Nhanh", 18], ["high", "Chất lượng cao", 32]];
    const qName = (id) => (qs.find((q) => q[0] === id) || [id, id])[1];
    const packHtml = (p) => {
      const ss = p.status, ph = ss.phase, ready = ph === "ready", vn = p.id === "vieneu";
      let foot = "";
      if (ph === "none" || ph === "error") {
        const qpick = qsel || tv.default_quality || "compact";
        const qOpts = vn ? `<div class="q-grid grow">${qs.map(([id, name, mb]) => `<button class="q-opt ${qpick === id ? "on" : ""}" data-tts="q" data-q="${id}"><b>${name}</b><span class="muted small">≈ ${mb} MB</span></button>`).join("")}</div>` : `<span class="muted small grow">2 giọng · ≈ ${p.model_mb} MB</span>`;
        foot = `${ph === "error" ? `<div class="small" style="color:var(--red);margin-top:6px">Chưa tải được: ${esc(ss.error || "")}</div>` : ""}
          <div class="row" style="margin-top:6px">${qOpts}<button class="btn primary sm" data-tts="install" data-engine="${p.id}">Cài đặt ${esc(p.name)}</button></div>`;
      } else if (ph === "installing") {
        foot = `<div class="tts-prog" data-prog="${p.id}"><span class="spin"></span><div class="grow"><b class="small">${esc(ss.step || "Đang cài đặt…")}</b><div class="muted small tts-detail">${esc(ss.detail || "")}</div></div></div>`;
      } else {
        const using = tv.engine === p.id;
        foot = `<div class="row" style="margin-top:6px"><span class="muted small grow">Đã kích hoạt${vn ? ` · ${esc(qName(ss.quality || "compact"))}` : ""}${using ? " · đang dùng" : ""}</span>
          <button class="btn sm ghost" data-tts="uninstall" data-engine="${p.id}">Gỡ</button></div>`;
      }
      return `<div class="tts-pack"><div class="row"><b class="grow">${esc(p.name)}</b><span class="muted small">${esc(p.blurb)}</span></div>
        <div class="voice-grid two">${p.voices.map((v) => voiceBtn(p.id, ready, v)).join("")}</div>${foot}</div>`;
    };
    return `<div class="card" id="ttsCard" style="margin-bottom:10px"><div class="row"><b class="grow">Giọng đọc AI của Asi</b>
        <label class="row small"><input type="checkbox" data-k2="voice_reply" ${settingsCache.voice_reply !== false ? "checked" : ""}/> Đọc to câu trả lời</label></div>
      <div class="muted small" style="margin:2px 0 6px">Chọn giọng và bấm ▶ để nghe thử.</div>
      ${tv.packs.map(packHtml).join("")}
      <div class="voice-grid one" style="margin-top:6px">${sysOpt}</div></div>`;
  }
  let qsel = null;
  async function redrawVoice() {
    const tv = await api("tts_info").catch(() => null);
    const c = $("#ttsCard");
    if (tv && c) c.outerHTML = voiceCard(tv);
  }
  on("tts-progress", (ev) => {
    if (tab !== "settings") return;
    const s = ev.status || ev, name = ev.engine === "nghi" ? "Asi Thanh Âm Nhanh" : "Asi Neural Studio";
    if (s.phase === "ready") { toast(`Đã kích hoạt ${name} 🎉 Asi sẽ đọc bằng giọng mới`); redrawVoice(); return; }
    if (s.phase === "error") { toast(`Chưa cài được ${name}`); redrawVoice(); return; }
    const c = $(`#ttsCard [data-prog="${ev.engine || "vieneu"}"]`);
    if (!c) return redrawVoice();
    c.querySelector("b").textContent = s.step || "";
    c.querySelector(".tts-detail").textContent = s.detail || "";
  });
  $("#view").addEventListener("click", async (e) => {
    const b = e.target.closest("[data-tts]");
    if (!b || tab !== "settings") return;
    e.stopPropagation();
    const a = b.dataset.tts, voice = b.dataset.voice || "", engine = b.dataset.engine || "vieneu";
    if (a === "q") { qsel = b.dataset.q; redrawVoice(); return; }
    if (a === "install") {
      try { await api("tts_install", { engine, quality: b.dataset.q || qsel || null }); } catch (err) { toast(String(err)); }
      qsel = null; setTimeout(redrawVoice, 200); return;
    }
    if (a === "uninstall") {
      if (b.dataset.sure !== "1") { b.dataset.sure = "1"; b.textContent = "Bấm lần nữa để gỡ"; return; }
      await api("tts_uninstall", { engine }); toast("Đã gỡ giọng"); redrawVoice(); return;
    }
    if (a === "use") {
      await api("tts_use", { engine, voice, preview: false });
      settingsCache.tts_engine = engine; settingsCache.tts_voice = voice;
      toast(voice ? `Đã chọn giọng ${voice}` : "Đã chọn giọng mặc định của máy");
      redrawVoice();
      return;
    }
    if (a === "hear") {
      toast(voice ? `▶ Đang phát thử giọng ${voice}…` : "▶ Đang phát thử giọng hệ thống…");
      await api("tts_use", { engine, voice, preview: true });
      settingsCache.tts_engine = engine; settingsCache.tts_voice = voice;
      redrawVoice();
    }
  });
  $("#view").addEventListener("click", async (e) => {
    const ve = e.target.closest("[data-vengine]");
    if (ve && tab === "settings") {
      const val = ve.dataset.vengine;
      const st = await api("get_settings");
      st.viet_engine = val;
      await api("save_settings", { settings: st });
      settingsCache = st;
      if (window.VNKey) window.VNKey.userOn = (val === "telex" || val === "vni") ? val : null;
      const activeMode = await syncImeInteractive(true);
      api("threads_embed_ime", { ime: activeMode });
      ve.parentElement.querySelectorAll("button").forEach((x) => x.classList.toggle("on", x === ve));
      const badge = $("#imeActiveBadge");
      if (badge) {
        badge.className = `chip ${activeMode ? "blue" : ""}`;
        badge.textContent = activeMode ? `Đang bật: ${activeMode === "vni" ? "VNI" : "Telex"}` : "Đang tắt (EN)";
      }
      const inp = $("#imeTestInput");
      if (inp) inp.focus();
      toast(activeMode ? `⌨️ Đã bật bộ gõ ${activeMode === "vni" ? "VNI" : "Telex"}` : "⌨️ Đã tắt bộ gõ của Asi");
      return;
    }
    const vc = e.target.closest("[data-vcount]");
    if (vc && tab === "settings") {
      const st = await api("get_settings");
      st.vocab_count = Number(vc.dataset.vcount);
      await api("save_settings", { settings: st });
      settingsCache = st;
      vc.parentElement.querySelectorAll("button").forEach((x) => x.classList.toggle("on", x === vc));
      return;
    }
    const w = e.target.closest("[data-wing]");
    if (w && tab === "settings") {
      const preset = w.dataset.wing;
      const st = await api("get_settings");
      st.wing_size = preset;
      st.notch_w = WING_PRESETS[preset] ?? 56;
      await api("save_settings", { settings: st });
      settingsCache = st;
      applyNotchGeometry(settingsCache, false);
      viewSettings();
      toast(preset === "hidden" ? "Đã ẩn hoàn toàn sau tai thỏ — rê chuột lên mép trên để mở" : `Đã đặt cỡ tai thỏ: ${st.notch_w} px`);
    }
  });
  $("#view").addEventListener("input", (e) => {
    if (e.target.id === "notchWRange") {
      const nw = Number(e.target.value);
      settingsCache.notch_w = nw;
      settingsCache.wing_size = nw <= 2 ? "hidden" : "custom";
      const lbl = $("#notchWVal");
      if (lbl) lbl.textContent = nw <= 2 ? "0 px (Ẩn hoàn toàn sau tai thỏ)" : `+${nw} px mỗi bên`;
      applyNotchGeometry(settingsCache, false);
    }
    if (e.target.id === "notchHRange") {
      const nh = Number(e.target.value);
      settingsCache.notch_h = nh;
      const lbl = $("#notchHVal");
      if (lbl) lbl.textContent = `${nh} px`;
      applyNotchGeometry(settingsCache, false);
    }
  });
  $("#view").addEventListener("change", async (e) => {
    if (e.target.id === "notchWRange" || e.target.id === "notchHRange") {
      applyNotchGeometry(settingsCache, true);
      await api("save_settings", { settings: settingsCache });
      return;
    }
  });
  $("#view").addEventListener("click", async (e) => {
    const dsp = e.target.closest("[data-disp]");
    if (dsp && tab === "settings") {
      await api("set_display", { mode: dsp.dataset.disp });
      setTimeout(() => location.reload(), 150); // nạp lại giao diện theo kiểu mới
    }
  });
  $("#view").addEventListener("click", async (e) => {
    const sw = e.target.closest(".switch[data-bk]");
    if (!sw) return;
    const on = !sw.classList.contains("on");
    await api("backup_set", { [sw.dataset.bk === "auto_backup" ? "autoBackup" : "deviceSync"]: on });
    sw.classList.toggle("on", on);
    if (sw.dataset.bk === "device_sync" && on) api("device_sync_now").then(() => tab === "settings" && viewSettings()).catch((err) => toast(String(err)));
  });
  $("#view").addEventListener("change", async (e) => {
    if (e.target.dataset && e.target.dataset.feat) {
      const st = await api("get_settings");
      const set = new Set(st.visible_tabs && st.visible_tabs.length ? st.visible_tabs : ["home", "tasks", "notes", "clips", "journal", "words", "threads"]);
      e.target.checked ? set.add(e.target.dataset.feat) : set.delete(e.target.dataset.feat);
      set.add("home");
      st.visible_tabs = [...set];
      await api("save_settings", { settings: st });
      settingsCache = st;
      e.target.closest(".feat").classList.toggle("on", e.target.checked);
      renderTabs();
      return;
    }
    if (e.target.dataset && e.target.dataset.idle) {
      const st = await api("get_settings");
      const cur = new Set(Array.isArray(st.idle_items) ? st.idle_items : IDLE_DEFAULT);
      if (e.target.checked) cur.add(e.target.dataset.idle); else cur.delete(e.target.dataset.idle);
      const order = ["focus", "overdue", "event", "next", "time", "tasks", "notes", "clips", "disk", "mic"];
      st.idle_items = order.filter((k) => cur.has(k));
      await api("save_settings", { settings: st });
      settingsCache = st;
      e.target.closest(".feat").classList.toggle("on", e.target.checked);
      updateBar();
      return;
    }
    if (e.target.dataset && e.target.dataset.k2) {
      const st = await api("get_settings");
      st[e.target.dataset.k2] = e.target.checked;
      await api("save_settings", { settings: st });
      settingsCache = st;
      return;
    }
    if (e.target.id === "bkDir") {
      if (e.target.value === "__pick") { await api("backup_pick_dir"); } else { await api("backup_set", { dir: e.target.value }); }
      viewSettings();
      return;
    }
    const el = e.target.closest("[data-k]");
    if (!el || el.classList.contains("switch")) return;
    const st = await api("get_settings");
    st[el.dataset.k] = el.type === "number" ? Number(el.value) : el.value;
    await api("save_settings", { settings: st });
    settingsCache = st;
    toast("Đã lưu");
  });

  // ═════════ CÀI ĐẶT ═════════
  async function viewSettings() {
    const st = await api("get_settings");
    const lic = await api("license_info");
    const mac = geo.platform === "macos";
    const ax = await api("accessibility_status");
    const si = await api("sync_info");
    settingsCache = st;
    const perms = await api("permissions_status");
    const bk = await api("backup_info");
    if (!appleAiState) api("apple_ai_status").then((r) => { appleAiState = r; }).catch(() => {});
    const permIcon = { granted: "✅", denied: "⛔", not_asked: "⚪", unknown: "⚪", manual: "🔸" };
    const permCard = `<div class="card" id="permCard" data-sec="sec-perm" style="margin-bottom:10px">
        <div class="row"><div class="grow"><b>Quyền ứng dụng</b><div class="muted small">Cấp các quyền cần thiết để Mini-Asi hoạt động đầy đủ tính năng</div></div>
          <button class="btn primary sm" data-s="permall">Cấp tất cả</button></div>
        <div class="perm-list">${perms.map((p) => `<div class="perm-row">
          <div class="grow"><b>${p.name}</b> ${p.required ? "" : '<span class="chip">Tuỳ chọn</span>'} <span class="muted small">${p.why}</span></div>
          ${p.state === "granted" ? '<span class="chip green">Đã cấp</span>' : `<button class="btn sm ghost" data-s="perm" data-p="${p.id}">${p.state === "denied" && ["accessibility", "disk"].includes(p.id) ? "Mở cài đặt" : "Cấp quyền"}</button>`}</div>`).join("")}</div>
      </div>`;
    const ago = (ms) => fmtAgo(ms);
    const backupCard = `<div class="sec" id="sec-sync">Sao lưu & Đồng bộ đám mây</div>
      <div class="card">
        <div class="setting"><div class="grow"><b>Thư mục đám mây</b><span>${bk.current ? esc(bk.current) : "Chưa chọn thư mục sao lưu"}</span></div>
          <select class="input" id="bkDir">${bk.dirs.map((d) => `<option value="${esc(d.path)}" ${bk.current === d.path ? "selected" : ""}>${esc(d.label)}</option>`).join("")}
            ${bk.chosen && !bk.dirs.some((d) => d.path === bk.chosen) ? `<option value="${esc(bk.chosen)}" selected>${esc(bk.chosen.split(/[\\/]/).pop())}</option>` : ""}
            <option value="__pick">Chọn thư mục khác…</option></select></div>
        <div class="setting"><div class="grow"><b>Sao lưu</b><span>Lần cuối: ${ago(bk.last_backup)}</span></div>
          <button class="btn sm" data-s="bknow" ${bk.current ? "" : "disabled"}>Sao lưu ngay</button>
          <button class="switch ${bk.auto_backup ? "on" : ""}" data-bk="auto_backup" title="Tự sao lưu mỗi ngày"></button></div>
        <div class="setting"><div class="grow"><b>Đồng bộ giữa các máy</b><span>${esc(bk.device_name)} · lần cuối ${ago(bk.last_device_sync)}</span></div>
          <button class="btn sm" data-s="devsync" ${bk.current ? "" : "disabled"}>Đồng bộ ngay</button>
          <button class="switch ${bk.device_sync ? "on" : ""}" data-bk="device_sync" title="Tự đồng bộ mỗi 15 phút"></button></div>
        ${bk.snapshots.length ? `<div class="setting" style="display:block"><b>Khôi phục</b>
          ${bk.snapshots.slice(0, 6).map((f) => `<div class="row small" style="margin-top:6px"><span class="grow">${esc(f.device || "")} · ${new Date(f.modified).toLocaleString("vi-VN")} · ${bytes(f.size)}</span>
            <button class="btn sm ghost" data-s="restore" data-p="${esc(f.path)}">Khôi phục</button></div>`).join("")}</div>` : ""}
      </div>`;
    const vis = st.visible_tabs && st.visible_tabs.length ? st.visible_tabs : ["home", "tasks", "notes", "clips", "journal", "words", "threads"];
    const featCard = `<details class="card feat-card" style="margin-bottom:10px"><summary><b>Quản lý thanh tính năng</b> <span class="muted small">— chọn tính năng hiện trên thanh chính</span></summary>
      <div class="feat-grid">${TABS.filter(([id]) => !["home", "settings"].includes(id)).map(([id, name, ic]) => `<label class="feat ${vis.includes(id) ? "on" : ""}">
        <input type="checkbox" data-feat="${id}" ${vis.includes(id) ? "checked" : ""}/>${ic}<span>${name}</span></label>`).join("")}</div>
      <div class="row" style="margin-top:8px"><label class="row small grow"><input type="checkbox" data-k2="threads_embed_beta" ${st.threads_embed_beta ? "checked" : ""}/> Thử nghiệm: Threads nhúng trong cửa sổ Asi</label></div>
    </details>`;
    const ime = await api("ime_info").catch(() => null);
    const tv = await api("tts_info").catch(() => null);
    await api("update_info").then((v) => { if (v) { updateVer = v.current_version || updateVer; updateState = v.status || v; } }).catch(() => {});
    const ttsCard = tv ? voiceCard(tv) : "";
    const updCard = updateCardHtml(st);
    const aiAvail = appleAiState ? appleAiState.available : true;
    const aiCard = `<div class="card" id="sec-ai" style="margin-bottom:10px">
      <div class="row">
        <div class="grow">
          <b>Apple Intelligence (On-Device)</b>
          <div class="muted small">${esc((appleAiState && appleAiState.detail) || "Chạy trực tiếp trên Apple Silicon — hỏi đáp, tóm tắt, viết nháp, dịch thuật")}</div>
        </div>
        <span class="chip ${aiAvail && st.apple_ai_enabled !== false ? "green" : ""}">${aiAvail ? (st.apple_ai_enabled !== false ? "Đang bật" : "Đang tắt") : "Chưa hỗ trợ"}</span>
        <label class="row small" style="margin-left:6px"><input type="checkbox" data-k2="apple_ai_enabled" ${st.apple_ai_enabled !== false ? "checked" : ""}/> Bật</label>
      </div>
    </div>`;
    const hasCustomPos = (st.bubble_x > 0 || st.bubble_y > 0);
    const isSmallIcon = st.icon_size === "small";
    const dispCard = `<div class="card" id="sec-disp" style="margin-bottom:10px"><b>Kiểu hiển thị & Vị trí</b>
      <div class="disp-grid">${[["notch", "Tai thỏ", "Ôm sát tai thỏ"], ["pill", "Viên thuốc", "Viên thuốc nổi"], ["bubble", "Bong bóng", "Nút tròn nổi"]]
        .map(([id, name, why]) => `<button class="disp-opt ${DISP === id ? "on" : ""}" data-disp="${id}"><span class="disp-pv pv-${id}"></span><b>${name}</b><span class="muted small">${why}</span></button>`).join("")}</div>
      <div class="row" style="margin-top:10px;padding-top:8px;border-top:1px solid var(--line)">
        <div class="grow small"><b>Cỡ biểu tượng:</b> <span class="muted">Kích thước thanh điều hướng</span></div>
        <div class="seg">
          <button class="${!isSmallIcon ? "on" : ""}" data-icon-size="large">Chuẩn</button>
          <button class="${isSmallIcon ? "on" : ""}" data-icon-size="small">Nhỏ</button>
        </div>
      </div>
      <div class="row" style="margin-top:8px;padding-top:8px;border-top:1px solid var(--line)">
        <div class="grow small"><b>Vị trí ghim:</b> <span class="muted">${hasCustomPos ? `(${Math.round(st.bubble_x)}, ${Math.round(st.bubble_y)})` : "Mặc định"}</span></div>
        <button class="btn sm ${pinned ? "primary" : ""}" data-s="pin-toggle">${pinned ? "Đang ghim mở" : "Ghim mở"}</button>
        <button class="btn sm ghost" data-s="reset-pos" ${hasCustomPos ? "" : "disabled"}>Đặt lại vị trí</button>
      </div></div>`;
    const IDLE_OPTS = [["focus", "", "Đồng hồ tập trung"], ["overdue", "", "Số việc trễ"], ["event", "", "Lịch sắp tới"], ["next", "", "Việc sắp đến hạn"], ["time", "", "Giờ hiện tại"],
      ["tasks", "", "Số việc còn lại"], ["notes", "", "Số ghi chú"], ["clips", "", "Số mục clipboard"], ["disk", "", "Dung lượng trống"], ["mic", "", "Nút micro"]];
    const idl = idleItems();
    const curWing = currentWingPx();
    const curH = st.notch_h > 0 ? Math.round(st.notch_h) : Math.max(Math.round(geo.notch_height), 32);
    const idleCard = `<div class="card" style="margin-bottom:10px"><b>Kích thước Tai thỏ & Chế độ chờ</b>
      <div class="row" style="margin-top:8px"><span class="small grow">Độ rộng cánh tai thỏ</span>
        <div class="seg">${[["hidden", "Ẩn kín"], ["xs", "Rất gọn"], ["s", "Gọn"], ["m", "Vừa"], ["l", "Rộng"]].map(([v, n]) => `<button class="${(st.wing_size || "s") === v ? "on" : ""}" data-wing="${v}">${n}</button>`).join("")}</div></div>
      <div class="row" style="margin-top:8px">
        <span class="small" style="min-width:130px">Kéo chỉnh độ rộng:</span>
        <input type="range" id="notchWRange" min="0" max="180" step="2" value="${curWing}" class="grow" />
        <b class="small" id="notchWVal" style="min-width:80px;text-align:right">${curWing <= 2 ? "Ẩn kín" : `${curWing} px`}</b>
      </div>
      <div class="row" style="margin-top:6px">
        <span class="small" style="min-width:130px">Độ cao tai thỏ:</span>
        <input type="range" id="notchHRange" min="22" max="54" step="1" value="${curH}" class="grow" />
        <b class="small" id="notchHVal" style="min-width:80px;text-align:right">${curH} px</b>
      </div>
      <div class="idle-grid" style="margin-top:10px">${IDLE_OPTS.map(([id, , name]) => `<label class="feat ${idl.includes(id) ? "on" : ""}"><input type="checkbox" data-idle="${id}" ${idl.includes(id) ? "checked" : ""}/><span>${name}</span></label>`).join("")}</div></div>`;
    const vocabCard = `<div class="card" style="margin-bottom:10px"><div class="row"><div class="grow"><b>Học từ mới tiếng Anh</b>
        <div class="muted small">Hiện từ vựng mới kèm phiên âm trên trang chính</div></div>
        <div class="seg">${[2, 3].map((n) => `<button class="${(st.vocab_count || 3) === n ? "on" : ""}" data-vcount="${n}">${n} từ</button>`).join("")}</div>
        <label class="row small" style="margin-left:8px"><input type="checkbox" data-k2="vocab_enabled" ${st.vocab_enabled !== false ? "checked" : ""}/> Bật</label></div></div>`;
    const V = window.VNKey;
    const curMode = V && V.mode ? V.mode : null;
    const imeCard = `<div class="card" id="sec-ime" style="margin-bottom:10px">
      <div class="row">
        <div class="grow">
          <b>Bộ gõ tiếng Việt (Telex / VNI)</b>
          <div class="muted small">Tự động chuyển bàn phím hệ thống khi tương tác</div>
        </div>
        <span class="chip ${curMode ? "blue" : ""}" id="imeActiveBadge">${curMode ? (curMode === "vni" ? "VNI" : "Telex") : "Tắt (EN)"}</span>
      </div>
      <div class="row" style="margin-top:8px">
        <span class="small muted grow">Chế độ gõ</span>
        <div class="seg">
          ${[["auto", "Tự động"], ["telex", "Telex"], ["vni", "VNI"], ["off", "Tắt"]].map(([v, lbl]) => `<button class="${(st.viet_engine || "auto") === v ? "on" : ""}" data-vengine="${v}">${lbl}</button>`).join("")}
        </div>
      </div>
      <div style="margin-top:8px">
        <input class="input" id="imeTestInput" placeholder="Gõ thử tiếng Việt: xin chào Việt Nam, được, thương…" style="height:32px;font-size:12px" />
      </div>
    </div>`;
    $("#view").innerHTML = updCard + permCard + dispCard + idleCard + aiCard + ttsCard + vocabCard + imeCard + featCard + backupCard + `<div style="height:10px"></div>
      <div class="card ${lic.pro ? "" : "upsell"}" style="margin:0 0 10px">
        ${lic.pro
          ? `<div class="row"><div class="grow"><b>Mini-Asi Pro</b><div class="muted small">${esc(lic.email)}${lic.expires ? ` · hết hạn ${new Date(lic.expires * 1000).toLocaleDateString("vi-VN")}` : " · vĩnh viễn"}</div></div>
             <button class="btn sm ghost" data-s="unlic">Gỡ khoá</button></div>`
          : `<div class="grow"><b>Nâng cấp Mini-Asi Pro</b>
             <div class="muted small" style="margin:2px 0 8px">Dọn sâu: file cài đặt, thư mục build, file lớn, sao lưu iPhone cũ.</div>
             <div class="row"><input class="input" id="licKey" placeholder="Dán khoá bản quyền MASI1.…" style="height:32px" />
             <button class="btn primary sm" data-s="lic">Kích hoạt</button></div>
             <div class="small" id="licMsg" style="color:var(--red);margin-top:4px"></div></div>`}
      </div>
      <div class="card">
        <div class="setting"><div class="grow"><b>Khởi động cùng máy</b><span>Mini-Asi luôn có mặt khi bạn mở máy</span></div>
          <button class="switch ${st.launch_at_login ? "on" : ""}" data-k="launch_at_login"></button></div>
        <div class="setting"><div class="grow"><b>Chế độ tập trung</b><span>Tắt màu đỏ báo trễ việc và nhắc nghỉ</span></div>
          <button class="switch ${st.focus_mode ? "on" : ""}" data-k="focus_mode"></button></div>
        <div class="setting"><div class="grow"><b>Chất lượng nén PDF mặc định</b><span>Cân bằng phù hợp gửi email</span></div>
          <select class="input" data-k="pdf_quality">
            <option value="screen" ${st.pdf_quality === "screen" ? "selected" : ""}>Nhỏ nhất</option>
            <option value="ebook" ${st.pdf_quality === "ebook" ? "selected" : ""}>Cân bằng</option>
            <option value="printer" ${st.pdf_quality === "printer" ? "selected" : ""}>Đẹp (in ấn)</option></select></div>
        <div class="setting"><div class="grow"><b>Phím tắt mở nhanh</b><span>Mở/đóng tai thỏ từ bất kỳ đâu</span></div>
          <span class="chip">${mac ? "⌘ ⇧ Space" : "Ctrl ⇧ Space"}</span></div>
        <div class="setting"><div class="grow"><b>Màn hình</b><span>${geo.has_notch ? `Có tai thỏ (${Math.round(geo.notch_width)}×${Math.round(geo.notch_height)})` : "Không có tai thỏ — dùng tai thỏ ảo"}</span></div></div>
      </div>
      <div class="sec">Clipboard</div>
      <div class="card">
        <div class="setting"><div class="grow"><b>Lưu lịch sử clipboard</b><span>Chữ, ảnh, file — bỏ qua mật khẩu từ trình quản lý mật khẩu</span></div>
          <button class="switch ${st.clip_enabled ? "on" : ""}" data-k="clip_enabled"></button></div>
        <div class="setting"><div class="grow"><b>Số mục tối đa</b><span>Mục đã ghim luôn được giữ</span></div>
          <input class="input num" type="number" min="20" max="2000" value="${st.clip_max}" data-k="clip_max" /></div>
        <div class="setting"><div class="grow"><b>Chọn là dán luôn</b><span>${ax ? "Đã có quyền Trợ năng" : "Cần bật quyền Trợ năng cho Mini-Asi"}</span></div>
          ${ax ? "" : `<button class="btn sm ghost" data-s="ax">Mở cài đặt</button>`}
          <button class="switch ${st.clip_autopaste ? "on" : ""}" data-k="clip_autopaste"></button></div>
        <div class="setting"><div class="grow"><b>Phím tắt mở clipboard</b><span>Mở thẳng danh sách đã chép</span></div>
          <span class="chip">${esc(st.clip_shortcut.replace("Control", "⌃").replace("Shift", "⇧").replace("Alt", "⌥").replace(/\+/g, " "))}</span></div>
      </div>
      <div class="sec">Giọng nói & Nhật ký</div>
      <div class="card">
        <div class="setting"><div class="grow"><b>Ngôn ngữ giọng nói</b><span>Dùng khi ghi chú, thêm việc, viết nhật ký bằng giọng nói</span></div>
          <select class="input" data-k="voice_lang"><option value="vi-VN" ${st.voice_lang === "vi-VN" ? "selected" : ""}>Tiếng Việt</option>
          <option value="en-US" ${st.voice_lang === "en-US" ? "selected" : ""}>English (US)</option><option value="en-GB" ${st.voice_lang === "en-GB" ? "selected" : ""}>English (UK)</option></select></div>
        <div class="setting"><div class="grow"><b>Asi hỏi chuyện nhật ký mỗi tối</b><span>Hỏi về ngày hôm nay và ghi lại giúp bạn</span></div>
          <input class="input num" type="number" min="17" max="23" value="${st.journal_hour}" data-k="journal_hour" /><span class="muted small">giờ</span>
          <button class="switch ${st.journal_enabled ? "on" : ""}" data-k="journal_enabled"></button></div>
      </div>
      ${mac ? `<div class="sec">Đồng bộ với ứng dụng Apple</div>
      <div class="card">
        <div class="setting"><div class="grow"><b>Việc ⇄ Lời nhắc (Reminders)</b><span>Hai chiều, trong danh sách “${esc(st.reminders_list)}”</span></div>
          <input class="input" style="width:110px;height:28px" value="${esc(st.reminders_list)}" data-k="reminders_list" />
          <button class="switch ${st.sync_reminders ? "on" : ""}" data-k="sync_reminders"></button></div>
        <div class="setting"><div class="grow"><b>Ghi chú ⇄ Ghi chú (Notes)</b><span>Hai chiều, trong thư mục “${esc(st.notes_folder)}”</span></div>
          <input class="input" style="width:110px;height:28px" value="${esc(st.notes_folder)}" data-k="notes_folder" />
          <button class="switch ${st.sync_notes ? "on" : ""}" data-k="sync_notes"></button></div>
        <div class="setting"><div class="grow"><b>Lịch (Calendar)</b><span>Hiện lịch họp hôm nay, nhắc trước giờ họp, thêm việc vào Lịch</span></div>
          <input class="input num" type="number" min="1" max="60" value="${st.event_alert_minutes}" data-k="event_alert_minutes" /><span class="muted small">phút</span>
          <button class="switch ${st.show_calendar ? "on" : ""}" data-k="show_calendar"></button></div>
        <div class="setting"><div class="grow"><b>Trạng thái</b><span id="syncState">${si.last_run ? "Đồng bộ lần cuối " + fmtAgo(si.last_run) + " · " + si.linked_tasks + " việc · " + si.linked_notes + " ghi chú" : "Chưa đồng bộ"}${si.last_error ? `<br><span style="color:var(--red)">${esc(si.last_error)}</span>` : ""}</span></div>
          <button class="btn sm" data-s="sync">${I.sync}Đồng bộ ngay</button></div>
      </div>` : ""}
      <div class="row" style="margin-top:12px"><span class="muted small grow">Mini-Asi 0.11.0 · Dữ liệu chỉ lưu trên máy bạn</span>
        <button class="btn ghost" data-s="quit">Thoát Mini-Asi</button></div>`;
  }
  $("#view").addEventListener("click", async (e) => {
    if (tab !== "settings") return;
    const b = e.target.closest("[data-s]");
    if (!b) return;
    const a = b.dataset.s;
    if (a === "reset-pos") { await resetPosition(); return; }
    if (a === "pin-toggle") {
      pinned = !pinned;
      updateBar();
      toast(pinned ? "Đã ghim mở popup — không tự đóng khi bấm ra ngoài" : "Đã bỏ ghim mở popup");
      viewSettings();
      return;
    }
    if (a === "lic") {
      const r = await api("activate_license", { key: $("#licKey").value });
      if (r.pro) { confetti(40); flashMood("love", 4000); scan = null; viewSettings(); }
      else $("#licMsg").textContent = r.error || "Khoá không hợp lệ";
    }
    if (a === "unlic") { await api("deactivate_license"); viewSettings(); }
    if (a === "ax") api("open_privacy", { pane: "accessibility" });
    if (a === "permall" || a === "perm") {
      b.disabled = true;
      b.innerHTML = '<span class="spin"></span>Đang xin quyền…';
      await api("request_permissions", { which: a === "permall" ? "all" : b.dataset.p });
      toast("Xong — kiểm tra lại danh sách quyền nhé");
      viewSettings();
      return;
    }
    if (a === "bknow") {
      b.disabled = true; b.innerHTML = '<span class="spin"></span>';
      try { const [p, n] = await api("backup_now"); toast(`Đã sao lưu ${bytes(n)} lên ${p.includes("Google") ? "Google Drive" : "thư mục đám mây"} ✓`); confetti(14); } catch (err) { toast(String(err)); }
      viewSettings(); return;
    }
    if (a === "devsync") {
      b.disabled = true; b.innerHTML = '<span class="spin"></span>';
      try { const r = await api("device_sync_now"); const n = r.stats.tasks + r.stats.notes + r.stats.journal + r.stats.clips; toast(r.devices.length ? `Đã gộp ${n} mục từ ${r.devices.join(", ")}` : "Chưa thấy máy nào khác — cài Mini-Asi trên máy kia và bật đồng bộ"); } catch (err) { toast(String(err)); }
      viewSettings(); return;
    }
    if (a === "restore") {
      if (b.dataset.sure !== "1") { b.dataset.sure = "1"; b.textContent = "Chắc chưa? Bấm lần nữa"; b.classList.add("red"); return; }
      try { await api("backup_restore", { path: b.dataset.p }); toast("Đã khôi phục ✓ (bản cũ được cất riêng)"); await refreshTasks(); } catch (err) { toast(String(err)); }
      viewSettings(); return;
    }
    if (a === "sync") {
      b.disabled = true;
      b.innerHTML = '<span class="spin"></span>Đang đồng bộ…';
      const r = await api("sync_now");
      toast(r.errors.length ? "⚠ " + r.errors[0] : `Xong: ↑${r.tasks_pushed + r.notes_pushed} ↓${r.tasks_pulled + r.notes_pulled} · ${r.events} sự kiện`);
      viewSettings();
    }
    if (a === "quit") api("quit");
  });

  // ───────── Vòng cập nhật & đồng bộ giữa cửa sổ Tai thỏ và cửa sổ Cài đặt riêng ─────────
  on("settings-changed", (newSt) => {
    if (!newSt) return;
    settingsCache = newSt;
    applyIconSize(settingsCache);
    if (!IS_SETTINGS_WIN) {
      applyNotchGeometry(settingsCache, mode === "collapsed");
      applyColor();
      applyMood();
      updateBar();
      if (mode === "expanded") renderTabs();
    }
  });
  on("settings-section", (sec) => {
    if (sec) setTimeout(() => scrollToSettingsSection(sec), 80);
  });

  function onStatus(s) {
    const prevLevel = status ? status.lateness.level : 0;
    status = s;
    if (IS_SETTINGS_WIN) return;
    applyColor();
    applyMood();
    updateBar();
    if (mode === "peek") renderPeek();
    if (mode === "expanded") renderTabs();
    if (s.lateness.level > prevLevel && prevLevel === 0) flashMood("alarm", 1800);
    if (s.lateness.level === 0 && prevLevel > 0) { flashMood("happy", 2500); confetti(14); }
  }
  on("status", onStatus);
  setInterval(() => { if (!IS_SETTINGS_WIN && status && mode !== "expanded" && (status.focus_until || idleItems().includes("time"))) updateBar(); }, 1000);
  setInterval(() => { if (!IS_SETTINGS_WIN && mode === "expanded" && tab === "tasks" && !typing()) drawTaskList(); }, 30000);

  await refreshTasks();
  onStatus(await api("get_status"));
  if (IS_SETTINGS_WIN) {
    tab = "settings";
    await setMode("expanded", { force: true });
    if (INIT_SETTINGS_SEC) setTimeout(() => scrollToSettingsSection(INIT_SETTINGS_SEC), 120);
  } else {
    await setMode("collapsed", { force: true });
    flashMood("happy", 1800); // chào khi khởi động
    // Chỉ dùng khi xem thử bằng trình duyệt (dev-mock): ?open=peek | ?open=<tab> để mở sẵn một màn hình
    const devOpen = document.body.classList.contains("mock") ? URL_PARAMS.get("open") : null;
    if (devOpen === "peek") await setMode("peek", { force: true });
    else if (devOpen) { tab = devOpen; await setMode("expanded", { force: true }); }
  }

  // Cho phép extra.js (Clipboard, Nhật ký) dùng chung tiện ích
  window.ASI = {
    api, on, $, $$, esc, I, toast, askAsi, voiceCommand, isComposing, typing, updateBar, placeEmbed, hideEmbed, switchTab, threadsInline, confetti, flashMood, fmtDue, fmtAgo, hm, bytes, mins, dictate, micBtn, langPill, geo,
    views: EXTRA_VIEWS,
    get tab() { return tab; }, set tab(t) { tab = t; },
    get mode() { return mode; }, setMode, renderView,
    get settings() { return settingsCache; },
    takeFreshOpen() { const f = freshOpen; freshOpen = false; return f; },
    refreshTasks,
  };
  document.dispatchEvent(new Event("asi-ready"));

  // Cho phép bản thử nghiệm điều khiển từ ngoài (chụp màn hình, test)
  window.__asi = { setMode, get mode() { return mode; }, set tab(t) { tab = t; }, renderView, showAlert, flashMood, addFiles };
})();
