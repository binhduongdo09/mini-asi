/* ───────────────────────── Mini-Asi — vỏ hòn đảo (cửa sổ tai thỏ) ─────────────────────────
 * Trạng thái hòn đảo:
 *   collapsed (thu gọn) → peek (rê chuột: trạng thái + Micro/Camera/AirDrop/Tập trung) → expanded (bảng chính)
 *   alert (nhắc việc, nghỉ mắt, lịch, nhật ký) · listen / reply (lệnh giọng nói) · drop (kéo file vào) · sheet (công cụ file)
 * File này lo: hình học cửa sổ, linh vật, thanh trên, peek, kéo/giãn, giọng nói, báo động, kéo thả file, thanh tab.
 * Nội dung từng tab nằm ở: views.js (Asi, Việc, Ghi chú, Dọn máy) · extra.js (Clipboard, Nhật ký, Threads) · vocab.js (Từ mới).
 * Cài đặt là cửa sổ riêng: settings.html + settings.js.
 * File phụ dùng chung tiện ích qua `window.ASI` (khai báo cuối file) và đăng ký view bằng `A.views.<tab> = fn`.
 */
(async function () {
  "use strict";
  const C = await window.AsiCommon;
  const { api, on, $, $$, esc, isComposing, typing, pad, hm, fmtDue, fmtAgo, bytes, mins, I } = C;
  const island = $("#island");
  const root = document.documentElement;
  const URL_PARAMS = new URLSearchParams(location.search);

  // ───────── Hình học ─────────
  const geo = await api("get_geometry");
  let settings = await api("get_settings");
  if (window.I18N && I18N.setLang(settings.ui_lang)) return; // đổi ngôn ngữ → cửa sổ tải lại
  const DISP = ["notch", "pill", "bubble"].includes(settings.display_mode) ? settings.display_mode : "notch";
  document.body.classList.add("disp-" + DISP);
  const ON_NOTCH = DISP === "notch" && geo.has_notch;
  if (!ON_NOTCH) island.classList.add("virtual");
  if (settings.bubble_x > 0 || settings.bubble_y > 0) document.body.classList.add("floating-pos");

  const WING_PRESETS = { hidden: 0, xs: 28, s: 56, m: 88, l: 128 };
  function wingPx(st = settings) {
    const ws = st.wing_size || "s";
    if (ws === "hidden") return 0;
    if (ws === "custom") return Math.max(0, Math.min(180, Math.round(Number(st.notch_w) || 0)));
    return WING_PRESETS[ws] ?? 56;
  }

  const MARGIN_X = 18, MARGIN_B = 20; // vùng đệm trong suốt quanh hòn đảo (cho bóng & góc cong)
  const MAX_W = Math.max(600, Math.round((geo.screen_width || 1440) - 60));
  const MAX_H = Math.max(420, Math.round((geo.screen_height || 900) - 40));
  let BAR = 32, BASE_W = 300;
  const SIZES = {};
  let mode = "collapsed";
  let tab = "home";
  let status = null;
  let tasks = [];
  let media = null; // trạng thái nhạc gần nhất (media_status)
  const cam = { active: false, stream: null };
  let popupLow = false; // popup của macOS / app đang hiện (perm-prompt)
  let hovering = false, modeTimer = null, frameTimer = null, suppressHoverUntil = 0, lastFrameKey = "";
  let freshOpen = true;

  const clampW = (w) => Math.round(Math.min(Math.max(w, Math.max(BASE_W + 60, 560)), MAX_W));
  const clampH = (h) => Math.round(Math.min(Math.max(h, 400), MAX_H));
  const EMBED_OK = geo.platform === "macos" || URL_PARAMS.has("embed");
  const threadsInline = () => EMBED_OK && DISP !== "bubble" && tab === "threads" && (settings.threads_embed_beta || URL_PARAMS.has("embed")) && !!window.__thInline;

  // Mô-đun phụ (bật/tắt trong Cài đặt › Tính năng). Asi, Việc, Ghi chú, Clipboard luôn có.
  const DEFAULT_MODULES = ["threads", "clean", "focus"];
  const modules = () => (Array.isArray(settings.modules) ? settings.modules : DEFAULT_MODULES);
  const modOn = (id) => modules().includes(id);

  function peekHeight() {
    const hasMedia = !!(media && (media.title || media.playing));
    const n = Math.min(PEEK_LATE_MAX, lateTasks().length);
    return BAR + 128 + (hasMedia ? 64 : 0) + (cam.active ? 178 : 0) + (n ? 8 + n * 32 + (n - 1) * 2 : 0);
  }
  // Việc đang trễ hiện ngay khi rê chuột (Cài đặt › Tai thỏ › Khi rê chuột); trễ lâu nhất lên đầu
  const PEEK_LATE_MAX = 3;
  function lateTasks() {
    if (settings.peek_overdue === false || !status || status.focus_until) return [];
    const now = Date.now();
    return tasks.filter((t) => !t.done_at && t.due && t.due < now).sort((a, b) => a.due - b.due);
  }
  function sizeOf(m) {
    if (m === "peek") return [SIZES.peek[0], peekHeight()];
    if (m !== "expanded") return SIZES[m] || SIZES.collapsed;
    if (threadsInline()) return [clampW(settings.threads_w || 480), clampH(settings.threads_h || Math.min(MAX_H, 860))];
    return [clampW(settings.panel_w || SIZES.expanded[0]), clampH(settings.panel_h || SIZES.expanded[1])];
  }
  function sendFrame(w, h, force = false) {
    const rw = Math.round(w + MARGIN_X * 2), rh = Math.round(h + MARGIN_B);
    const k = `${rw}x${rh}`;
    if (!force && k === lastFrameKey) return Promise.resolve();
    lastFrameKey = k;
    return api("set_frame", { width: rw, height: rh });
  }
  function applyGeometry(st = settings, resizeNow = false) {
    const wing = wingPx(st);
    const defBar = Math.round(ON_NOTCH ? geo.notch_height : DISP === "notch" ? 30 : 34);
    const customH = Math.round(Number(st.notch_h) || 0);
    BAR = wing === 0 && ON_NOTCH ? Math.max(18, Math.round(geo.notch_height - 2)) : customH >= 20 ? Math.min(54, customH) : defBar;
    BASE_W = DISP === "bubble" ? 58 : ON_NOTCH ? Math.round(geo.notch_width + wing * 2) : Math.max(110, Math.round(140 + wing * 1.4));
    island.classList.toggle("notch-hidden", DISP !== "bubble" && wing <= 2);
    root.style.setProperty("--bar-h", BAR + "px");
    root.style.setProperty("--gap-w", (ON_NOTCH ? Math.round(geo.notch_width) : 0) + "px");
    SIZES.collapsed = DISP === "bubble" ? [58, 58] : [BASE_W, BAR];
    SIZES.peek = [Math.max(BASE_W + 100, 460), 0];
    SIZES.alert = [Math.max(BASE_W + 80, 460), BAR + 122];
    SIZES.listen = [Math.max(BASE_W + 60, 440), BAR + 78];
    SIZES.reply = [Math.max(BASE_W + 120, 500), BAR + 270];
    SIZES.drop = [Math.max(BASE_W + 120, 480), BAR + 112];
    SIZES.sheet = [Math.max(BASE_W + 140, 520), BAR + 188];
    SIZES.capture = [Math.max(BASE_W + 140, 560), BAR + 336];
    SIZES.expanded = [Math.max(BASE_W + 140, 700), 520];
    const [w, h] = sizeOf(mode);
    root.style.setProperty("--w", w + "px");
    root.style.setProperty("--h", h + "px");
    if (resizeNow) sendFrame(w, h, true);
  }
  applyGeometry(settings, true);

  // ───────── Bộ gõ: mở bảng → bật Telex/VNI của Asi (backend tạm chuyển bàn phím hệ thống sang ABC) ─────────
  async function syncIme(onNow, keepFocus) {
    const V = window.VNKey;
    if (!onNow) {
      if (V) { V.mode = null; V.suspended = false; }
      await api("set_interactive", { on: false, keepFocus: keepFocus ?? null });
      updateBar();
      return null;
    }
    const m = await api("set_interactive", { on: true, keepFocus: keepFocus ?? null });
    if (V) { V.mode = m || (V.userOn ? V.userOn : null); V.suspended = false; }
    updateBar();
    return V ? V.mode : m;
  }
  async function openSettings(section = null) {
    try {
      await api("open_settings_window", { section: section || null });
      if (mode !== "collapsed") setMode("collapsed");
    } catch (err) { toast(String(err)); }
  }

  // ───────── Đổi trạng thái hòn đảo ─────────
  async function setMode(next, opts = {}) {
    if (next === mode && !opts.force) return;
    if (popupLow && (next === "expanded" || next === "peek")) { popupLow = false; api("popup_done").catch(() => {}); }
    clearTimeout(modeTimer);
    clearTimeout(frameTimer);
    if (next === "collapsed" && mode !== "collapsed") suppressHoverUntil = Date.now() + 500;
    const [w, h] = sizeOf(next);
    const [cw, ch] = sizeOf(mode === next ? "collapsed" : mode);
    const growing = w >= cw || h >= ch;
    const frame = () => sendFrame(w, h, !!opts.force);
    if (growing) await frame(); // nở cửa sổ trước rồi mới chạy hoạt cảnh → không bị cắt
    island.className = island.className.replace(/mode-\w+/, "mode-" + next);
    root.style.setProperty("--w", w + "px");
    root.style.setProperty("--h", h + "px");
    const prev = mode;
    mode = next;
    if (!growing) frameTimer = setTimeout(frame, 420);
    if (next !== "peek" && cam.active) stopCam(false);
    if (next !== "expanded" || tab !== "threads") hideEmbed();
    if (next === "expanded" && prev !== "expanded") {
      freshOpen = true;
      await syncIme(true);
      renderView();
    } else if (next === "expanded") {
      renderView();
    } else if (next === "capture") {
      if (prev !== "capture") await syncIme(true); // bảng tạo việc/lịch có ô nhập → cần bàn phím + bộ gõ
    } else if (prev === "expanded" || prev === "capture") {
      syncIme(false);
    }
    if (next === "peek") { renderPeek(); refreshMedia(); }
    updateBar();
    applyMood();
  }

  // ───────── Linh vật & màu ─────────
  let moodOverride = null, moodTimer = null, angerTimer = null;
  function flashMood(m, ms = 2200) {
    moodOverride = m;
    clearTimeout(moodTimer);
    moodTimer = setTimeout(() => { moodOverride = null; applyMood(); }, ms);
    applyMood();
  }
  function currentMood() {
    if (moodOverride) return moodOverride;
    if (mode === "drop") return "drop";
    if (!status) return "idle";
    if (status.focus_until) return "focus"; // đang tập trung: Asi đứng yên, không làm phiền
    if (status.lateness.level >= 1 && !status.focus_mode) return "angry";
    if (status.disk && status.disk.level === "critical") return "worried";
    if (status.session_minutes >= status.break_minutes) return "tired";
    if (status.sleepy) return "sleepy";
    return "idle";
  }
  function applyMood() {
    const m = currentMood();
    island.className = island.className.replace(/mood-\w+/, "mood-" + m);
    const lvl = m === "angry" && status ? status.lateness.level : 0;
    island.dataset.anger = String(lvl);
    clearInterval(angerTimer);
    if (lvl >= 3) {
      angerTimer = setInterval(() => {
        if (currentMood() !== "angry" || mode !== "collapsed") return;
        island.classList.add("stomp");
        setTimeout(() => island.classList.remove("stomp"), 700);
      }, 20000);
    }
  }
  // Nền luôn đen như Dynamic Island; trạng thái chỉ báo bằng một vạch màu mảnh ở đáy
  function applyColor() {
    const s = status;
    let accent = null;
    const calm = s && (s.focus_mode || s.focus_until);
    if (s && !calm) {
      if (s.disk && s.disk.level !== "ok") accent = "var(--amber)";
      if (s.session_minutes >= s.break_minutes) accent = "var(--blue)";
      if (s.lateness.level > 0) accent = "var(--red)";
    }
    $("#accent").classList.toggle("on", !!accent);
    if (accent) root.style.setProperty("--accent", accent);
  }

  // ───────── Thanh trên ─────────
  const IDLE_DEFAULT = ["focus", "overdue", "event", "next", "tasks", "mic"];
  const idleItems = () => (Array.isArray(settings.idle_items) ? settings.idle_items : IDLE_DEFAULT);
  // ───────── Ghi cuộc họp: trạng thái đang ghi (đồng hồ đỏ trên tai thỏ) ─────────
  let meet = null; // { id, title, secs, system_audio, stt, pending, mic, sys }
  const clockOf = (secs) => { const s = Math.max(0, Math.floor(secs || 0)); return s >= 3600 ? `${Math.floor(s / 3600)}:${pad(Math.floor(s / 60) % 60)}:${pad(s % 60)}` : `${pad(Math.floor(s / 60))}:${pad(s % 60)}`; };
  function setMeet(m) {
    meet = m ? { ...(meet || {}), ...m } : null;
    updateBar();
    document.dispatchEvent(new Event("asi-meet"));
  }
  on("meeting-state", (m) => setMeet(m));
  on("meeting-level", (lv) => {
    if (!meet || !Array.isArray(lv)) return;
    meet.secs = lv[0]; meet.mic = lv[1]; meet.sys = lv[2];
    updateBar();
    document.dispatchEvent(new Event("asi-meet-level"));
  });
  api("meeting_status").then((m) => m && setMeet(m)).catch(() => {});

  function updateBar() {
    const rw = $("#rightWing");
    if (mode === "expanded") {
      const V = window.VNKey;
      const key = `exp:${V && V.mode ? V.mode : "en"}:${status && status.update_version ? 1 : 0}`;
      if (rw.dataset.keys === key) return;
      rw.dataset.keys = key;
      rw.innerHTML = `<button class="ime-pill ${V && V.mode ? "on" : ""}" data-act="ime" title="Bộ gõ tiếng Việt của Asi · ⌥ bấm để đổi Telex/VNI">${V && V.mode ? (V.mode === "vni" ? "VNI" : "VI") : "EN"}</button>
        <button class="bar-btn ${status && status.update_version ? "dot" : ""}" data-act="settings" title="Cài đặt">${I.gear}</button>`;
      return;
    }
    const s = status;
    const items = idleItems();
    const micHtml = items.includes("mic") && DISP !== "bubble" && !meet ? `<button class="wing-mic" data-act="vmic" title="Ra lệnh bằng giọng nói">${I.mic}</button>` : "";
    if (!s) { if (rw.dataset.keys !== "nos") { rw.innerHTML = micHtml; rw.dataset.keys = "nos"; } return; }
    if (DISP === "bubble" && mode === "collapsed") {
      const n = s.lateness.overdue && !s.focus_mode ? s.lateness.overdue : 0;
      const k = "bub:" + n + (meet ? ":rec" : "");
      if (rw.dataset.keys !== k) { rw.innerHTML = meet ? `<i class="rec-dot b-rec"></i>` : n ? `<span class="b-dot">${n}</span>` : ""; rw.dataset.keys = k; }
      return;
    }
    const now = Date.now(), b = [];
    const want = (k) => items.includes(k);
    if (meet) b.push(["meet", "red rec", "", clockOf(meet.secs)]); // đang ghi họp: đồng hồ đỏ
    if (want("focus") && s.focus_until) {
      const left = Math.max(0, s.focus_until * 1000 - now);
      b.push(["focus", "violet", "", `${pad(Math.floor(left / 60000))}:${pad(Math.floor((left % 60000) / 1000))}`]);
    }
    if (want("overdue") && s.lateness.overdue > 0 && !s.focus_mode && !s.focus_until) b.push(["overdue", "red", "", String(s.lateness.overdue)]);
    if (want("event") && s.next_event && s.next_event.start > now && s.next_event.start - now < 15 * 60000) b.push(["event", "blue", I.cal, hm(new Date(s.next_event.start))]);
    if (want("next") && s.next && s.next.due - now < 3600000) b.push(["next", "", I.clock, hm(new Date(s.next.due))]);
    if (want("time")) b.push(["time", "ghost", "", hm(new Date(now))]);
    if (want("tasks") && s.open_count > 0 && !b.some((x) => x[0] === "overdue")) b.push(["tasks", "ghost", I.check, String(s.open_count)]);
    const keys = b.map((x) => x[0]).join(",") + (micHtml ? "|mic" : "");
    if (rw.dataset.keys === keys) {
      // cùng các mục → chỉ đổi chữ tại chỗ, không vẽ lại (không nhấp nháy)
      for (const [k, , , t] of b) { const el = rw.querySelector(`[data-k="${k}"] .bt`); if (el && el.textContent !== t) el.textContent = t; }
      return;
    }
    rw.innerHTML = b.map(([k, c, ic, t]) => `<span class="badge ${c}" data-k="${k}">${ic}<span class="bt">${t}</span></span>`).join("") + micHtml;
    rw.dataset.keys = keys;
    // Cánh phải hẹp → bỏ bớt mục ít ưu tiên ở cuối
    const shown = [...rw.querySelectorAll(".badge")];
    const overflow = () => rw.firstElementChild && rw.firstElementChild.getBoundingClientRect().left < rw.getBoundingClientRect().left - 0.5;
    while (shown.length > 1 && overflow()) shown.pop().remove();
  }

  // ───────── Peek: một dòng trạng thái + nhạc (nếu đang phát) + 4 nút nhanh ─────────
  function peekStatus() {
    const s = status, now = Date.now();
    if (meet) return { go: "meet", html: `<span class="pill red"><i class="rec-dot"></i>Đang ghi ${clockOf(meet.secs)}</span><span class="t" translate="no">${esc(meet.title || "Cuộc họp")}</span>` };
    if (!s) return { html: "", go: "home" };
    if (s.lateness.overdue > 0 && !s.focus_until) {
      const late = lateTasks();
      if (late.length) {
        const more = late.length - PEEK_LATE_MAX;
        return { go: "tasks", html: `<span class="pill red">${s.lateness.overdue} việc trễ</span><span class="t dim">${more > 0 ? `và ${more} việc khác` : "Xem danh sách việc"}</span>` };
      }
      const t = tasks.find((x) => !x.done_at && x.due && x.due < now);
      return { go: "tasks", html: `<span class="pill red">${s.lateness.overdue} việc trễ</span><span class="t" translate="no">${esc(t ? t.title : "")}</span>` };
    }
    if (s.focus_until) {
      const left = Math.max(1, Math.round((s.focus_until * 1000 - now) / 60000));
      return { go: "home", html: `<span class="pill violet">Đang tập trung</span><span class="t">còn ${left} phút</span>` };
    }
    if (s.next_event && (!s.next || s.next_event.start < s.next.due)) {
      const live = s.next_event.start <= now;
      return { go: "tasks", html: `<span class="pill blue">${live ? "Đang diễn ra" : fmtDue(s.next_event.start)}</span><span class="t" translate="no">${esc(s.next_event.title)}</span>` };
    }
    if (s.next) return { go: "tasks", html: `<span class="pill">${fmtDue(s.next.due)}</span><span class="t" translate="no">${esc(s.next.title)}</span>` };
    return { go: "tasks", html: `<span class="t dim">${s.open_count ? `${s.open_count} việc chưa đặt giờ` : "Không có việc nào sắp tới"}</span>` };
  }
  function peekMediaHtml() {
    const m = media;
    if (!m || !(m.title || m.playing)) return "";
    return `<div class="pk-media">
      <div class="art ${m.playing ? "on" : ""}">${m.playing ? '<i></i><i></i><i></i>' : I.music}</div>
      <div class="trk"><b>${esc(m.title || m.app || "Đang phát")}</b><span>${esc([m.artist, m.app].filter(Boolean).join(" · "))}</span></div>
      <button class="mbtn" data-pk="prev" title="Bài trước">${I.prev}</button>
      <button class="mbtn big" data-pk="play" title="${m.playing ? "Tạm dừng" : "Phát"}">${m.playing ? I.pause : I.play}</button>
      <button class="mbtn" data-pk="next" title="Bài sau">${I.next}</button>
    </div>`;
  }
  function peekTilesHtml() {
    const m = media || {};
    const focusOn = !!(status && status.focus_until);
    const tile = (id, on, ic, label, sub, cls = "") => `<button class="tile ${on ? "on" : ""} ${cls}" data-pk="${id}" title="${esc(label + " · " + sub)}"><span class="ti">${ic}</span><b>${label}</b><span>${sub}</span></button>`;
    return `<div class="pk-tiles">
      ${tile("mic", !m.mic_muted, m.mic_muted ? I.micOff : I.mic, "Micro", m.mic_muted ? "Đang tắt" : "Đang bật", m.mic_muted ? "warn" : "")}
      ${tile("cam", cam.active, cam.active ? I.camOff : I.cam, "Camera", cam.active ? "Đang mở" : "Soi gương")}
      ${tile("airdrop", false, I.airdrop, "AirDrop", "Gửi file")}
      ${modOn("focus") ? tile("focus", focusOn, I.target, "Tập trung", focusOn ? "Dừng" : "25 phút", "violet") : ""}
    </div>`;
  }
  function peekLateHtml() {
    const late = lateTasks().slice(0, PEEK_LATE_MAX);
    if (!late.length) return "";
    return `<div class="pk-late">${late.map((t) => `<div class="pk-lrow" data-go="tasks" data-tid="${t.id}">
      <span class="due">${fmtDue(t.due)}</span><span class="tt" translate="no">${esc(t.title)}</span>
      <button class="pk-lb" data-pt="done" title="Xong">${I.check}</button><button class="pk-lb" data-pt="s60" title="Hoãn 1 giờ">+1h</button></div>`).join("")}</div>`;
  }
  function renderPeek() {
    if (!status) return;
    // Số việc trễ / nhạc đổi → chiều cao đổi: dựng lại khung trước (setMode gọi lại renderPeek)
    if (mode === "peek" && Math.round(parseFloat(root.style.getPropertyValue("--h"))) !== peekHeight()) { setMode("peek", { force: true }); return; }
    const pk = $("#peek");
    const st = peekStatus();
    const parts = [`<div class="pk-status" data-go="${st.go}">${st.html}<span class="chev">${I.chevron}</span></div>`, peekLateHtml(), peekMediaHtml(), peekTilesHtml()];
    if (cam.active) parts.push(`<div class="pk-cam"><video id="camVideo" autoplay playsinline muted></video><button class="btn sm" data-pk="snap">Chụp vào Clipboard</button></div>`);
    const html = parts.join("");
    if (pk.dataset.html === html) return; // không đổi gì → không vẽ lại (chống nhấp nháy)
    pk.dataset.html = html;
    pk.innerHTML = html;
    if (cam.active && cam.stream) { const v = $("#camVideo"); if (v) v.srcObject = cam.stream; }
  }
  async function refreshMedia() {
    const before = peekHeight();
    try { media = await api("media_status"); } catch (_) { return; }
    if (mode !== "peek") return;
    if (peekHeight() !== before) setMode("peek", { force: true });
    else renderPeek();
  }
  function stopCam(resize = true) {
    if (cam.stream) { try { cam.stream.getTracks().forEach((t) => t.stop()); } catch (_) {} }
    cam.active = false;
    cam.stream = null;
    if (resize && mode === "peek") setMode("peek", { force: true });
  }
  async function toggleCam() {
    if (cam.active) { stopCam(true); return; }
    const ok = await api("ensure_camera_perm").catch(() => true);
    if (!ok) { toast("Chưa có quyền Camera — bật trong Cài đặt hệ thống › Quyền riêng tư"); return; }
    if (!navigator.mediaDevices || !navigator.mediaDevices.getUserMedia) { toast("Máy này chưa mở được Camera trong Mini-Asi"); return; }
    try {
      cam.stream = await navigator.mediaDevices.getUserMedia({ video: { width: { ideal: 640 }, height: { ideal: 360 }, facingMode: "user" }, audio: false });
      cam.active = true;
      await setMode("peek", { force: true });
    } catch (err) { toast("Không mở được Camera: " + String(err.message || err)); }
  }
  async function peekAction(act) {
    if (["prev", "play", "next"].includes(act)) {
      try { media = await api("media_control", { action: act }); } catch (err) { toast(String(err)); }
      renderPeek();
      return;
    }
    if (act === "mic") {
      try { media = await api("toggle_mic_mute"); } catch (err) { toast(String(err)); return; }
      renderPeek();
      toast(media.mic_muted ? "Đã tắt micro" : "Đã bật micro");
      return;
    }
    if (act === "cam") return toggleCam();
    if (act === "snap") {
      const v = $("#camVideo");
      if (!v || !v.videoWidth) return;
      const cv = document.createElement("canvas");
      cv.width = v.videoWidth; cv.height = v.videoHeight;
      const ctx = cv.getContext("2d");
      ctx.translate(cv.width, 0); ctx.scale(-1, 1); ctx.drawImage(v, 0, 0);
      await api("clip_copy_image", { dataUrl: cv.toDataURL("image/png") });
      flashMood("happy", 1200);
      toast("Đã chụp — ⌘V để dán");
      return;
    }
    if (act === "focus") {
      if (status && status.focus_until) await api("focus_stop");
      else { await api("focus_start", { minutes: 25 }); toast("Tập trung 25 phút — Asi tạm im lặng"); }
      status = await api("get_status");
      applyColor(); applyMood(); updateBar(); renderPeek();
      return;
    }
    if (act === "airdrop") {
      suppressHoverUntil = Date.now() + 2000;
      const paths = await api("pick_files");
      if (!paths || !paths.length) return;
      try { await api("file_airdrop", { paths }); setMode("collapsed"); } catch (err) { toast(String(err)); }
    }
  }

  // ───────── Kéo giãn bảng (tay kéo đáy & góc) · kéo cạnh tai thỏ để đổi độ rộng ─────────
  (function grips() {
    for (const cls of ["grip", "grip-corner l", "grip-corner r", "notch-edge l", "notch-edge r"]) {
      const g = document.createElement("div");
      g.className = cls;
      g.title = cls.startsWith("notch") ? "Kéo để đổi độ rộng tai thỏ · bấm đúp để ẩn/hiện" : "Kéo để đổi kích thước · bấm đúp để về mặc định";
      island.appendChild(g);
    }
    let drag = null, wingDrag = null, raf = 0;
    island.addEventListener("pointerdown", (e) => {
      const ne = e.target.closest(".notch-edge");
      if (ne && (mode === "collapsed" || mode === "peek") && DISP !== "bubble") {
        e.preventDefault(); e.stopPropagation();
        ne.setPointerCapture(e.pointerId);
        wingDrag = { x: e.screenX, side: ne.classList.contains("l") ? -1 : 1, start: wingPx(), last: wingPx() };
        island.classList.add("resizing");
        api("set_frame", { width: Math.max(600, (geo.notch_width || 185) + 380) + MARGIN_X * 2, height: sizeOf(mode)[1] + MARGIN_B });
        return;
      }
      const g = e.target.closest(".grip, .grip-corner");
      if (!g || mode !== "expanded") return;
      e.preventDefault();
      g.setPointerCapture(e.pointerId);
      const [w, h] = sizeOf("expanded");
      drag = { x: e.screenX, y: e.screenY, w, h, corner: g.classList.contains("grip-corner"), side: g.classList.contains("l") ? -1 : 1 };
      island.classList.add("resizing");
      api("set_frame", { width: MAX_W + MARGIN_X * 2, height: MAX_H + MARGIN_B }); // nở sẵn để kéo mượt
    });
    island.addEventListener("pointermove", (e) => {
      if (wingDrag) {
        const nw = Math.max(0, Math.min(180, Math.round(wingDrag.start + wingDrag.side * (e.screenX - wingDrag.x))));
        wingDrag.last = nw;
        settings.notch_w = nw;
        settings.wing_size = nw <= 2 ? "hidden" : "custom";
        cancelAnimationFrame(raf);
        raf = requestAnimationFrame(() => { applyGeometry(settings, false); updateBar(); });
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
      if (wingDrag) {
        const nw = wingDrag.last;
        wingDrag = null;
        island.classList.remove("resizing");
        applyGeometry(settings, true);
        updateBar();
        await saveSettings(["wing_size", "notch_w"]);
        return;
      }
      if (!drag) return;
      const { nw, nh } = drag;
      drag = null;
      island.classList.remove("resizing");
      if (nw && nh) {
        if (threadsInline()) { settings.threads_w = nw; settings.threads_h = nh; } else { settings.panel_w = nw; settings.panel_h = nh; }
        saveSettings(["panel_w", "panel_h", "threads_w", "threads_h"]);
      }
      const [w, h] = sizeOf("expanded");
      await api("set_frame", { width: w + MARGIN_X * 2, height: h + MARGIN_B });
      lastFrameKey = `${Math.round(w + MARGIN_X * 2)}x${Math.round(h + MARGIN_B)}`;
      if (tab === "threads" && embedShown) setTimeout(placeEmbed, 60);
    };
    island.addEventListener("pointerup", end);
    island.addEventListener("pointercancel", end);
    island.addEventListener("dblclick", (e) => {
      if (e.target.closest(".notch-edge")) {
        settings.wing_size = wingPx() <= 2 ? "s" : "hidden";
        settings.notch_w = WING_PRESETS[settings.wing_size];
        applyGeometry(settings, true);
        updateBar();
        saveSettings(["wing_size", "notch_w"]);
        return;
      }
      if (!e.target.closest(".grip, .grip-corner")) return;
      if (threadsInline()) { settings.threads_w = 0; settings.threads_h = 0; } else { settings.panel_w = 0; settings.panel_h = 0; }
      saveSettings(["panel_w", "panel_h", "threads_w", "threads_h"]);
      setMode("expanded", { force: true });
    });
  })();

  /** Lưu một số khoá cài đặt (không gửi cả bộ → không ghi đè thay đổi từ cửa sổ Cài đặt) */
  async function saveSettings(keys) {
    const patch = {};
    for (const k of keys) patch[k] = settings[k];
    try { settings = await api("patch_settings", { patch }); } catch (err) { toast(String(err)); }
  }

  // ───────── Rê chuột · kéo di chuyển · bấm ─────────
  function hoverSync(inside) {
    hovering = inside;
    clearTimeout(modeTimer);
    if (island.classList.contains("resizing") || island.classList.contains("dragging")) return;
    if (inside && mode === "collapsed" && DISP !== "bubble" && Date.now() >= suppressHoverUntil) {
      modeTimer = setTimeout(() => { if (hovering && !overMic && mode === "collapsed" && Date.now() >= suppressHoverUntil) setMode("peek"); }, 130);
    } else if (!inside && mode === "peek") {
      modeTimer = setTimeout(() => { if (!hovering && mode === "peek") setMode("collapsed"); }, cam.active ? 1600 : 380);
    }
  }
  on("hover", (inside) => { if (!inside) micHover(false); hoverSync(inside); });
  const MOCK = document.body.classList.contains("mock");
  island.addEventListener("mouseenter", () => (MOCK ? hoverSync(true) : (hovering = true)));
  island.addEventListener("mouseleave", () => { if (MOCK) { micHover(false); hoverSync(false); } });

  // Rê chuột lên nút micro trên tai thỏ (giữ ~0,45 giây) = bấm micro. Vòng sáng chạy quanh nút trong lúc chờ;
  // rời nút trước đó thì thôi. Kích hoạt xong phải rời nút rồi quay lại mới kích hoạt lần nữa.
  // Toạ độ chuột lấy từ backend (sự kiện "hover-at") vì cửa sổ không được focus thì trang không nhận mousemove.
  let overMic = false, micArmed = false, micTimer = 0;
  function micHover(over) {
    const btn = $("#rightWing .wing-mic");
    if (over && (!btn || settings.hover_mic === false || voice.active || meet || (mode !== "collapsed" && mode !== "peek"))) over = false;
    if (over === overMic) return;
    overMic = over;
    clearTimeout(micTimer);
    if (btn) btn.classList.toggle("arming", over && !micArmed);
    if (!over) {
      micArmed = false;
      if (hovering && mode === "collapsed") hoverSync(true); // rời nút nhưng vẫn trong tai thỏ → mở peek như thường
      return;
    }
    if (micArmed) return;
    clearTimeout(modeTimer); // đang chờ nói → không bung peek (nút sẽ chạy chỗ khác)
    micTimer = setTimeout(() => {
      if (!overMic || voice.active) return;
      micArmed = true;
      btn.classList.remove("arming");
      voiceCommand();
    }, 450);
  }
  function micHoverAt(x, y) {
    const btn = $("#rightWing .wing-mic");
    if (!btn) return micHover(false);
    const r = btn.getBoundingClientRect();
    micHover(x >= r.left - 3 && x <= r.right + 3 && y >= r.top - 4 && y <= r.bottom + 4);
  }
  on("hover-at", (p) => Array.isArray(p) && micHoverAt(p[0], p[1]));
  if (MOCK) island.addEventListener("mousemove", (e) => micHoverAt(e.clientX, e.clientY));

  // Kéo để ghim hòn đảo ở vị trí bất kỳ (thả gần tai thỏ → về chỗ cũ)
  let justDragged = false;
  (function dragMove() {
    let d = null, raf = 0;
    island.addEventListener("pointerdown", (e) => {
      if (e.button !== 0) return;
      if (e.target.closest("[data-act], [data-pk], .grip, .grip-corner, .notch-edge, button, input, select, textarea, a, label, video")) return;
      if (mode !== "collapsed" && mode !== "peek" && !e.target.closest("#bar")) return;
      const curW = sizeOf(mode)[0] + MARGIN_X * 2;
      const sx0 = geo.screen_x || 0, sy0 = geo.screen_y || 0, sw0 = geo.screen_width || 1440;
      const custom = settings.bubble_x > 0 || settings.bubble_y > 0;
      const wx = custom ? (DISP === "bubble" ? settings.bubble_x : settings.bubble_x - curW / 2) : (DISP === "bubble" ? sx0 + sw0 - 140 : sx0 + (sw0 - curW) / 2);
      const wy = custom ? settings.bubble_y : (DISP === "bubble" ? sy0 + 90 : DISP === "pill" ? sy0 + (geo.notch_height || 30) + 6 : sy0);
      d = { sx: e.screenX, sy: e.screenY, wx, wy, docked: DISP === "notch" && !custom, moved: false, pid: e.pointerId };
    });
    island.addEventListener("pointermove", (e) => {
      if (!d) return;
      const dx = e.screenX - d.sx, dy = e.screenY - d.sy;
      if (!d.moved && Math.hypot(dx, dy) < (d.docked ? 16 : 6)) return;
      if (!d.moved) { try { island.setPointerCapture(d.pid); } catch (_) {} clearTimeout(modeTimer); }
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
        const center = (geo.screen_x || 0) + (geo.screen_width || 1440) / 2;
        const backHome = DISP === "notch" && ny < 36 && Math.abs(nx + curW / 2 - center) < 60;
        api("move_bubble", { x: nx, y: ny, save: true });
        if (backHome) { settings.bubble_x = 0; settings.bubble_y = 0; document.body.classList.remove("floating-pos"); }
        else { settings.bubble_x = DISP === "bubble" ? nx : Math.round(nx + curW / 2); settings.bubble_y = ny; }
        if (tab === "threads" && embedShown) setTimeout(placeEmbed, 40);
        justDragged = true;
        setTimeout(() => (justDragged = false), 80);
      }
      island.classList.remove("dragging");
      d = null;
    };
    island.addEventListener("pointerup", endDrag);
    island.addEventListener("pointercancel", endDrag);
  })();

  async function toggleIme(alt) {
    const V = window.VNKey;
    const cur = V && V.mode ? V.mode : null;
    let next;
    if (alt) next = cur === "vni" ? "telex" : "vni";
    else if (cur) { V.lastPref = cur; next = "off"; }
    else next = (V && V.lastPref) || (settings.viet_engine === "vni" ? "vni" : "telex");
    settings.viet_engine = next;
    if (V) V.userOn = next === "off" ? null : next;
    await saveSettings(["viet_engine"]);
    const active = await syncIme(true);
    api("threads_embed_ime", { ime: active }).catch(() => {});
    toast(active ? `Gõ tiếng Việt · ${active === "vni" ? "VNI" : "Telex"}` : "Gõ tiếng Anh");
  }
  $("#bar").addEventListener("click", async (e) => {
    if (justDragged) return;
    const act = e.target.closest("[data-act]")?.dataset.act;
    if (act === "vmic") { e.stopPropagation(); voiceCommand(); return; }
    if (act === "ime") { e.stopPropagation(); toggleIme(e.altKey); return; }
    if (act === "settings") { e.stopPropagation(); openSettings(); return; }
    if (e.target.closest('[data-k="meet"]')) { e.stopPropagation(); tab = "meet"; setMode("expanded", { force: true }); return; }
    if (mode === "collapsed" || mode === "peek") setMode("expanded");
    else if (mode === "expanded") setMode("collapsed");
  });
  $("#peek").addEventListener("click", async (e) => {
    if (justDragged) return;
    const b = e.target.closest("[data-pk]");
    if (b) { e.stopPropagation(); await peekAction(b.dataset.pk); return; }
    const pt = e.target.closest("[data-pt]");
    if (pt) {
      e.stopPropagation();
      const id = Number(pt.closest("[data-tid]").dataset.tid);
      pt.disabled = true;
      try {
        if (pt.dataset.pt === "done") { await api("toggle_task", { id }); confetti(12); flashMood("happy"); }
        else { await api("snooze_task", { id, minutes: 60 }); toast("Đã hoãn 1 giờ"); }
      } catch (err) { toast(String(err)); }
      refreshTasks();
      return;
    }
    if (e.target.closest(".pk-media, .pk-tiles, .pk-cam")) return;
    const go = e.target.closest("[data-go]");
    if (go) tab = go.dataset.go;
    setMode("expanded");
  });
  window.addEventListener("keydown", (e) => {
    if (isComposing(e)) return;
    if (e.key === "Escape" && mode === "capture") { closeCapture(); return; }
    if (e.key === "Escape" && mode !== "collapsed") { setMode("collapsed"); return; }
    // ⌘1…⌘9: chuyển tab
    if (mode === "expanded" && (e.metaKey || e.ctrlKey) && /^[1-9]$/.test(e.key)) {
      const t = visibleTabs()[Number(e.key) - 1];
      if (t) { e.preventDefault(); switchTab(t[0]); }
    }
  });
  // Chỉ thu gọn khi người dùng thật sự rời đi — bộ gõ / bảng emoji có thể làm mất focus thoáng qua
  const maybeCollapse = (ms) => { modeTimer = setTimeout(() => { if (!document.hasFocus() && !hovering && !voice.active && mode === "expanded") setMode("collapsed"); }, ms); };
  window.addEventListener("blur", () => { if (mode === "expanded") maybeCollapse(450); });
  on("window-blur", () => { if (mode === "expanded") maybeCollapse(350); });
  window.addEventListener("focus", () => { clearTimeout(modeTimer); if (mode === "expanded") syncIme(true, false); });
  on("toggle", () => (mode === "expanded" ? setMode("collapsed") : setMode("expanded")));
  on("open-tab", (id) => {
    if (id === "settings") return openSettings();
    if (id !== "open" && TABS.some(([t]) => t === id)) tab = id;
    setMode("expanded", { force: true });
  });
  // Popup của macOS / app đang hiện → tai thỏ thu gọn & hạ tầng (xem window.rs). Người dùng mở lại tai thỏ = popup xong.
  on("perm-prompt", (low) => {
    popupLow = !!low;
    if (low && mode !== "collapsed") setMode("collapsed");
    else if (!low) { const [w, h] = sizeOf(mode); sendFrame(w, h, true); }
  });
  on("ime-warn", () => toast("Hãy thêm bàn phím ABC trong Cài đặt hệ thống › Bàn phím để Asi tự gõ tiếng Việt"));

  // Mắt chớp & liếc ngẫu nhiên
  (function life() {
    const face = $("#face");
    const blink = () => {
      if (!["sleepy", "happy", "love", "focus"].includes(currentMood())) {
        face.classList.add("blink");
        setTimeout(() => face.classList.remove("blink"), 140);
        const big = document.getElementById("bigFace");
        if (big) { big.classList.add("blink"); setTimeout(() => big.classList.remove("blink"), 150); }
      }
      setTimeout(blink, 2500 + Math.random() * 4000);
    };
    const look = () => {
      face.style.transform = currentMood() === "idle" ? `translateX(${Math.round((Math.random() - 0.5) * 4)}px)` : "";
      setTimeout(look, 1800 + Math.random() * 3500);
    };
    setTimeout(blink, 1500);
    setTimeout(look, 2500);
  })();

  function confetti(n = 24) {
    const box = $("#confetti");
    const colors = ["#ffd60a", "#30d158", "#64d2ff", "#ff6b81", "#bf5af2"];
    for (let i = 0; i < n; i++) {
      const c = document.createElement("i");
      c.style.left = 30 + Math.random() * 30 + "px";
      c.style.top = BAR / 2 + "px";
      c.style.background = colors[i % colors.length];
      c.style.setProperty("--dx", Math.random() * 160 - 40 + "px");
      c.style.setProperty("--dy", Math.random() * 90 + 10 + "px");
      c.style.setProperty("--rot", Math.random() * 540 + "deg");
      box.appendChild(c);
      setTimeout(() => c.remove(), 1200);
    }
  }
  const toast = C.toastIn(island);

  // ───────── Nhập bằng giọng nói ─────────
  const voice = { active: null };
  function micBtn(id, label = "Nói để nhập") {
    return `<button class="mic" id="${id}" title="${label} · bấm lần nữa để dừng">${I.mic}<span class="bars"><i></i><i></i><i></i><i></i></span></button>`;
  }
  /** Bấm mic: thu → (bấm lần nữa hoặc im 2 giây) → nhận dạng → onText(chuỗi). opts.lang ghi đè ngôn ngữ, opts.keep giữ bản ghi */
  async function dictate(btn, onText, opts = {}) {
    if (voice.active) { await stopDictation(); return; }
    try { await api("voice_start"); } catch (err) { toast(String(err)); return; }
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
      const text = await api("voice_stop", { lang: v.lang || settings.voice_lang, keep: v.keep });
      if (text) { v.onText(text); flashMood("happy", 1200); }
    } catch (err) {
      toast(String(err));
      flashMood("worried", 1600);
    } finally {
      v.btn.classList.remove("busy");
      if (moodOverride === "focus") { moodOverride = null; applyMood(); }
      setTimeout(() => { if (mode === "listen" && !voice.active) setMode("collapsed"); }, 2500);
    }
  }
  on("voice-level", ([level]) => {
    if (!voice.active) return;
    $$("i", voice.active.btn).forEach((b, k) => (b.style.height = 3 + Math.round(level * (8 + k * 3) * (0.6 + Math.random() * 0.6)) + "px"));
    island.style.setProperty("--lvl", level.toFixed(2));
  });
  on("voice-ended", () => stopDictation());

  // ───────── Báo động ─────────
  const alertQueue = [];
  let alertTimer = null;
  function showAlert(a) {
    if (mode === "expanded") {
      // đang mở bảng: không chen ngang, chỉ báo nhỏ
      toast(a.kind === "reminder" ? `Đến giờ: ${a.task.title}` : a.kind === "event" ? `Sắp tới: ${a.event.title}` : a.kind === "break" ? "Đến giờ nghỉ mắt" : a.kind === "stock" ? `${a.title} · ${a.text}` : "Asi muốn hỏi chuyện hôm nay");
      if (a.kind !== "reminder" && a.kind !== "stock") return;
    }
    alertQueue.push(a);
    if (mode !== "alert" && mode !== "expanded") nextAlert();
  }
  function nextAlert() {
    clearTimeout(alertTimer);
    const a = alertQueue.shift();
    if (!a) { if (mode === "alert") setMode("collapsed"); return; }
    const el = $("#alert");
    const x = `<button class="x" data-a="dismiss" title="Đóng">${I.x}</button>`;
    if (a.kind === "reminder") {
      el.innerHTML = `${x}<div class="al-title" translate="no">${esc(a.task.title)}</div><div class="al-sub">Đến giờ rồi</div>
        <div class="al-acts"><button class="btn primary" data-a="done">${I.check}Xong</button>
        <button class="btn" data-a="s10">10 phút</button><button class="btn" data-a="s60">1 giờ</button><button class="btn" data-a="stm">Mai</button></div>`;
      flashMood("alarm", 6000);
      island.classList.add("shake");
      setTimeout(() => island.classList.remove("shake"), 1600);
    } else if (a.kind === "break") {
      el.innerHTML = `${x}<div class="al-title">Nghỉ mắt 20 giây nhé</div><div class="al-sub">Bạn đã dùng máy ${a.minutes} phút liên tục — nhìn xa khoảng 6 mét.</div>
        <div class="al-acts"><button class="btn primary" data-a="rest">Bắt đầu nghỉ</button><button class="btn" data-a="later">Để sau 10 phút</button></div>`;
      flashMood("tired", 8000);
    } else if (a.kind === "event") {
      const m = Math.max(1, Math.round((a.event.start - Date.now()) / 60000));
      el.innerHTML = `${x}<div class="al-title" translate="no">${esc(a.event.title)}</div><div class="al-sub">${m} phút nữa · ${hm(new Date(a.event.start))}${a.event.location ? " · " + esc(a.event.location) : ""}</div>
        <div class="al-acts"><button class="btn primary" data-a="dismiss">Đã biết</button></div>`;
      flashMood("alarm", 3000);
    } else if (a.kind === "stock") {
      el.innerHTML = `${x}<div class="al-title">${esc(a.title)}</div><div class="al-sub">${esc(a.text)} · cảnh báo giá đã đặt</div>
        <div class="al-acts"><button class="btn primary" data-a="dismiss">Đã biết</button><button class="btn" data-a="squote" data-sym="${esc(a.symbol)}">Xem giá</button></div>`;
      flashMood("alarm", 5000);
      island.classList.add("shake");
      setTimeout(() => island.classList.remove("shake"), 1600);
    } else if (a.kind === "journal") {
      el.innerHTML = `<button class="x" data-a="jskip" title="Bỏ qua hôm nay">${I.x}</button><div class="al-title">Hôm nay của bạn thế nào?</div><div class="al-sub">3 câu hỏi nhỏ — Asi ghi nhật ký giúp bạn.</div>
        <div class="al-acts"><button class="btn primary" data-a="jwrite">Viết</button><button class="btn" data-a="jtalk">${I.mic}Nói</button><button class="btn" data-a="jlater">Để sau</button></div>`;
      flashMood("love", 3000);
    }
    el.dataset.id = a.task ? a.task.id : "";
    setMode("alert", { force: true });
    alertTimer = setTimeout(nextAlert, 45000);
  }
  $("#alert").addEventListener("click", async (e) => {
    const a = e.target.closest("[data-a]")?.dataset.a;
    if (!a) return;
    const id = Number($("#alert").dataset.id);
    if (a === "done") { await api("toggle_task", { id }); confetti(); flashMood("happy"); }
    if (a === "s10") await api("snooze_task", { id, minutes: 10 });
    if (a === "s60") await api("snooze_task", { id, minutes: 60 });
    if (a === "stm") await api("snooze_task", { id, minutes: -1 });
    if (a === "later") await api("health_snooze", { minutes: 10 });
    if (a === "rest") return restCountdown();
    if (a === "squote") { alertQueue.length = 0; tab = "home"; await setMode("expanded", { force: true }); return askAsi(`giá ${e.target.closest("[data-sym]").dataset.sym}`, false); }
    if (a === "jwrite" || a === "jtalk") {
      alertQueue.length = 0;
      tab = "journal";
      window.__asiJournalTalk = a === "jtalk";
      return setMode("expanded", { force: true });
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
      el.innerHTML = `<div class="al-rest"><div class="count">${n}</div><div><div class="al-title">Nhìn ra xa…</div><div class="al-sub">Thả lỏng vai, chớp mắt vài lần</div></div></div>`;
      if (n-- <= 0) {
        api("health_break_done");
        moodOverride = null;
        flashMood("happy");
        confetti(14);
        return nextAlert();
      }
      alertTimer = setTimeout(tick, 1000);
    };
    tick();
  }
  on("reminder", (task) => { refreshTasks(); showAlert({ kind: "reminder", task }); });
  on("break", (minutes) => showAlert({ kind: "break", minutes }));
  on("event-soon", (event) => showAlert({ kind: "event", event }));
  on("journal-ask", () => { if (modOn("journal")) showAlert({ kind: "journal" }); });
  on("stock-alert", (s) => {
    if (!s || !s.alert) return;
    showAlert({ kind: "stock", symbol: s.alert.symbol, title: s.title, text: s.text });
  });
  let syncDirty = false;
  on("synced", () => {
    if (typing()) { syncDirty = true; return; }
    refreshTasks();
    if (mode === "expanded" && (tab === "notes" || tab === "tasks")) renderView();
  });
  document.addEventListener("focusout", () => setTimeout(() => {
    if (syncDirty && !typing()) { syncDirty = false; refreshTasks(); if (mode === "expanded" && tab === "notes") renderView(); }
  }, 300));
  on("water", () => flashMood("happy", 2500));
  on("focus-done", () => { confetti(); flashMood("love", 4000); toast("Xong phiên tập trung 🎉"); });

  // ───────── Kéo thả file: nửa trái = AirDrop ngay · nửa phải = tấm công cụ file ─────────
  $$("#drop .drop-ic").forEach((el) => (el.innerHTML = I[el.dataset.i]));
  let dropZone = "airdrop", dropPoll = null, dropBefore = "collapsed";
  function setDropZone(z) {
    dropZone = z === "tools" || z === "capture" ? z : "airdrop";
    $$("#drop .drop-half").forEach((h) => h.classList.toggle("on", h.dataset.zone === dropZone));
  }
  $("#drop").addEventListener("dragover", (e) => { e.preventDefault(); setDropZone(e.clientX < window.innerWidth / 2 ? "airdrop" : "tools"); });
  on("tauri://drag-over", (p) => {
    const pos = p && p.position;
    if (!pos || typeof pos.x !== "number") return;
    const lx = pos.x > window.innerWidth ? pos.x / (window.devicePixelRatio || 1) : pos.x; // toạ độ logical hoặc physical
    setDropZone(lx < window.innerWidth / 2 ? "airdrop" : "tools");
  });
  // Kéo file → 2 nửa (AirDrop / công cụ file). Kéo chữ, link, ảnh từ web/app chat, danh bạ… (không phải file)
  // → 1 ô "Tạo việc / lịch".
  let dropContent = false;
  on("tauri://drag-enter", (p) => {
    if (mode === "drop") return;
    dropContent = !(p && p.paths && p.paths.length);
    $("#drop").classList.toggle("content", dropContent);
    dropBefore = mode;
    setDropZone(dropContent ? "capture" : "airdrop");
    setMode("drop");
    clearInterval(dropPoll);
    if (dropContent) return;
    dropPoll = setInterval(async () => {
      if (mode !== "drop") { clearInterval(dropPoll); return; }
      try { setDropZone((await api("cursor_side")) === "right" ? "tools" : "airdrop"); } catch (_) {}
    }, 90);
  });
  on("tauri://drag-leave", () => {
    clearInterval(dropPoll);
    if (mode === "drop") setMode(dropBefore === "expanded" ? "expanded" : "collapsed");
  });
  on("tauri://drag-drop", async (p) => {
    clearInterval(dropPoll);
    const paths = (p && p.paths) || [];
    if (!paths.length) {
      // chữ đang chọn, tin nhắn, link, ảnh trên web, danh bạ… → đọc bảng kéo-thả của macOS
      let info = null;
      try { info = await api("drop_read", { paths: [] }); } catch (err) { toast(String(err)); }
      if (!info || info.kind === "none") { await setMode(dropBefore === "expanded" ? "expanded" : "collapsed"); toast("Chưa đọc được nội dung này — thử kéo chữ, ảnh, link hoặc danh bạ"); return; }
      return openCapture(info);
    }
    let side = dropZone;
    try { side = (await api("cursor_side")) === "right" ? "tools" : "airdrop"; } catch (_) {}
    if (side === "airdrop") {
      await setMode("collapsed");
      try { await api("file_airdrop", { paths }); return; } catch (err) { toast(String(err)); }
    }
    await openSheet(paths);
  });

  // Tấm công cụ file
  const sheet = { files: [], busy: false, result: null };
  async function openSheet(paths, add = false) {
    const info = await api("file_info", { paths });
    if (!add) { sheet.files = []; sheet.result = null; }
    const known = new Set(sheet.files.map((f) => f.path));
    sheet.files.push(...info.filter((f) => !known.has(f.path)));
    sheet.capture = await api("capture_supports", { paths: sheet.files.map((f) => f.path) }).catch(() => false);
    sheet.pick = false;
    // ảnh / file chữ (kể cả trong thư mục) → nút đổi định dạng
    sheet.kinds = await api("file_kinds", { paths: sheet.files.map((f) => f.path) }).catch(() => ({ images: 0, texts: 0 }));
    renderSheet();
    await setMode("sheet", { force: true });
  }
  function renderSheet() {
    const f = sheet.files;
    const pdfs = f.filter((x) => x.is_pdf), arcs = f.filter((x) => x.is_archive);
    const ic = (x) => (x.is_pdf ? I.pdf : x.is_archive ? I.zip : x.is_dir ? I.folder : x.is_image ? I.image : x.is_text ? I.text : I.file);
    const total = f.reduce((s, x) => s + (x.size || 0), 0);
    const r = sheet.result;
    const resHtml = sheet.busy
      ? `<div class="sh-res"><span class="spin"></span><span class="grow">Đang xử lý…</span></div>`
      : r ? (r.error
        ? `<div class="sh-res err"><span class="grow">${esc(r.error)}</span></div>`
        : `<div class="sh-res ok">${I.check}<span class="grow"><b>${esc(r.output.split(/[\\/]/).pop())}</b> <span class="dim">${r.message && /\d+ file|ảnh|lỗi/.test(r.message) ? esc(r.message) + " · " : ""}${bytes(r.before)} → ${bytes(r.after)}</span></span><button class="btn sm" data-sh="reveal">Mở thư mục</button></div>`)
      : `<div class="sh-res idle">File mới lưu ngay cạnh file gốc · kéo cả thư mục → kết quả vào thư mục mới bên cạnh</div>`;
    const k = sheet.kinds || { images: 0, texts: 0 };
    const dis = sheet.busy ? "disabled" : "";
    const acts = sheet.pick
      ? `<div class="sh-acts sh-fmt"><span class="sh-fmt-l">${k.images} ảnh →</span>
          ${[["jpg", "JPG"], ["png", "PNG"], ["heic", "HEIC"], ["tiff", "TIFF"], ["pdf", "PDF"]].map(([f, n]) => `<button class="act" data-fmt="${f}" ${dis}><b>${n}</b><span>${f === "pdf" ? "mỗi ảnh 1 file" : f === "heic" ? "nhẹ nhất" : f === "jpg" ? "phổ biến" : f === "png" ? "không mất nét" : "in ấn"}</span></button>`).join("")}
          ${k.images > 1 ? `<button class="act" data-fmt="pdf1" ${dis}><b>1 PDF</b><span>gộp ${k.images} ảnh</span></button>` : ""}
          <button class="act ghost" data-sh="unpick">${I.x}<span>Thôi</span></button></div>`
      : `<div class="sh-acts">
        <button class="act" data-sh="zip" ${dis}>${I.zip}<span>Nén ZIP</span></button>
        ${arcs.length ? `<button class="act" data-sh="extract" ${dis}>${I.unzip}<span>Giải nén</span></button>` : ""}
        ${pdfs.length ? `<button class="act" data-sh="pdf" ${dis}>${I.pdf}<span>Nén PDF</span></button>` : ""}
        ${pdfs.length >= 2 ? `<button class="act" data-sh="merge" ${dis}>${I.merge}<span>Gộp ${pdfs.length} PDF</span></button>` : ""}
        ${k.images ? `<button class="act" data-sh="pick" ${dis}>${I.image}<span>Đổi định dạng</span></button>` : ""}
        ${k.texts ? `<button class="act" data-sh="topdf" ${dis}>${I.pdf}<span>${k.texts > 1 ? `${k.texts} file → PDF` : "Sang PDF"}</span></button>` : ""}
        ${sheet.capture ? `<button class="act" data-sh="capture">${I.cal}<span>Tạo việc · lịch</span></button>` : ""}
        <button class="act" data-sh="airdrop">${I.airdrop}<span>AirDrop</span></button>
        ${k.images && k.texts ? "" : `<button class="act" data-sh="copy">${I.copy}<span>Chép file</span></button>`}
      </div>`;
    $("#sheet").innerHTML = `<button class="x" data-sh="close" title="Đóng">${I.x}</button>
      <div class="sh-head"><b>${f.length === 1 ? esc(f[0].name) : `${f.length} mục`}</b><span class="dim">${f.length === 1 && f[0].is_dir ? "Thư mục" : bytes(total)}</span>
        <button class="link" data-sh="add">${I.plus}Thêm</button></div>
      <div class="sh-files">${f.slice(0, 6).map((x) => `<span class="fchip" title="${esc(x.path)}">${ic(x)}${esc(x.name)}</span>`).join("")}${f.length > 6 ? `<span class="fchip">+${f.length - 6}</span>` : ""}</div>
      ${acts}${resHtml}`;
  }
  async function runSheet(fn) {
    sheet.busy = true; sheet.result = null; renderSheet();
    flashMood("focus", 60000);
    try { sheet.result = await fn(); flashMood("happy"); confetti(12); }
    catch (err) { sheet.result = { error: String(err) }; flashMood("worried", 2500); }
    sheet.busy = false;
    renderSheet();
  }
  $("#sheet").addEventListener("click", async (e) => {
    const fm = e.target.closest("[data-fmt]");
    if (fm && !fm.disabled) {
      const f = fm.dataset.fmt, paths = sheet.files.map((x) => x.path);
      sheet.pick = false;
      return runSheet(() => api("file_convert_images", { paths, format: f === "pdf1" ? "pdf" : f, merge: f === "pdf1" }));
    }
    const b = e.target.closest("[data-sh]");
    if (!b || b.disabled) return;
    const a = b.dataset.sh, paths = sheet.files.map((f) => f.path);
    if (a === "close") return setMode("collapsed");
    if (a === "pick" || a === "unpick") { sheet.pick = a === "pick"; return renderSheet(); }
    if (a === "topdf") return runSheet(() => api("file_text_pdf", { paths }));
    if (a === "capture") { try { return openCapture(await api("drop_read", { paths })); } catch (err) { return toast(String(err)); } }
    if (a === "add") { suppressHoverUntil = Date.now() + 2000; const p = await api("pick_files"); if (p && p.length) openSheet(p, true); return; }
    if (a === "reveal" && sheet.result) return api("reveal", { path: sheet.result.output });
    if (a === "airdrop") { try { await api("file_airdrop", { paths }); setMode("collapsed"); } catch (err) { toast(String(err)); } return; }
    if (a === "copy") { await api("clip_copy_files", { paths }); toast("Đã chép — ⌘V để dán vào Finder, Mail, Zalo…"); return; }
    if (a === "zip") return runSheet(() => api("file_zip", { paths }));
    if (a === "merge") return runSheet(() => api("file_merge_pdf", { paths: sheet.files.filter((f) => f.is_pdf).map((f) => f.path) }));
    if (a === "extract") return runSheet(async () => { let r; for (const f of sheet.files.filter((x) => x.is_archive)) r = await api("file_extract", { path: f.path }); return r; });
    if (a === "pdf") return runSheet(async () => { let r; for (const f of sheet.files.filter((x) => x.is_pdf)) r = await api("file_compress_pdf", { path: f.path, quality: settings.pdf_quality || "ebook" }); return r; });
  });

  // ───────── Thả nội dung → tạo việc / sự kiện lịch (Apple / Google qua app Lịch của Mac) ─────────
  const cap = { info: null, d: null, cals: null, dirty: false, aiBusy: false, aiDraft: null, err: "", busy: false };
  const aiReady = () => (settings.ai_provider && settings.ai_provider !== "apple") || settings.apple_ai_enabled !== false;
  const ymd = (ms) => { const d = new Date(ms); return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`; };
  const engineName = (e) => (!e || e === "rule" ? "Đọc bằng quy tắc trên máy" : `Đọc bằng ${e}`);
  // opts.kind: "task" | "event" — ép loại (vd. bấm "Lịch" trên một ý của biên bản họp); opts.back: tab quay về sau khi xong
  function forceKind(d, k) {
    if (!k || !d) return d;
    d.kind = k;
    if (k === "event") { if (!d.start) { const t = new Date(); t.setMinutes(0, 0, 0); t.setHours(t.getHours() + 1); d.start = t.getTime(); } if (!d.end || d.end <= d.start) d.end = d.start + 3600000; }
    else d.all_day = false;
    return d;
  }
  async function closeCapture() {
    const back = cap.back;
    cap.back = null;
    if (back) { tab = back; await setMode("expanded", { force: true }); }
    else await setMode("collapsed");
  }
  async function openCapture(info, opts = {}) {
    Object.assign(cap, { info, d: null, dirty: false, aiDraft: null, err: "", busy: false, aiBusy: false, back: opts.back || null, kind: opts.kind || null });
    renderCapture();
    await setMode("capture", { force: true });
    try { cap.d = await api("capture_analyze", { ai: false }); }
    catch (err) { cap.d = { kind: "task", title: "", start: null, end: null, all_day: false, location: "", notes: info.preview || "", engine: "rule", ocr: "" }; if (info.kind !== "image") cap.err = String(err); }
    forceKind(cap.d, cap.kind);
    if (cap.kind === "event") loadCals();
    renderCapture();
    loadCals();
    if (info.kind === "image" || ((info.kind === "text" || info.kind === "url") && aiReady())) runCapAi(true);
    setTimeout(() => { const t = $("#cp-title"); if (t && !t.value) t.focus(); }, 60);
  }
  async function loadCals() {
    if (cap.cals) return;
    try { cap.cals = await api("calendar_list"); } catch (err) { cap.cals = []; cap.calErr = String(err); }
    if (mode === "capture" && cap.d && cap.d.kind === "event") { readCap(); renderCapture(); }
  }
  async function runCapAi(auto) {
    if (cap.aiBusy) return;
    cap.aiBusy = true; cap.err = ""; renderCapFoot();
    try {
      const d = await api("capture_analyze", { ai: true });
      if (mode !== "capture") return;
      if (auto && cap.dirty) { cap.aiDraft = d; }
      else { cap.d = forceKind({ ...d, calendar_id: cap.d ? cap.d.calendar_id : "" }, cap.kind); cap.dirty = false; cap.aiBusy = false; renderCapture(); return; }
    } catch (err) { if (!auto || !cap.d || !cap.d.title) cap.err = String(err); }
    cap.aiBusy = false;
    if (mode === "capture") renderCapFoot();
  }
  function srcHtml(i) {
    if (!i) return "";
    if (i.kind === "image") return `<div class="cp-src">${i.image ? `<img src="${i.image}" alt="">` : `<span class="cp-ic">${I.image}</span>`}<span class="grow dim">${cap.d && cap.d.ocr ? esc(cap.d.ocr.slice(0, 160)) : "Ảnh — Asi đọc chữ trong ảnh ngay trên máy (Vision)"}</span></div>`;
    if (i.kind === "contact") return `<div class="cp-src"><span class="cp-ic">${I.at}</span><span class="grow">${esc(i.contacts.join(", "))}</span></div>`;
    return `<div class="cp-src"><span class="cp-ic">${i.kind === "url" ? I.ext : I.text}</span><span class="grow dim cp-clamp">${esc(i.preview)}</span></div>`;
  }
  function calOptions() {
    const sel = (cap.d && cap.d.calendar_id) || settings.capture_calendar || "";
    const list = cap.cals || [];
    if (!list.length) return `<option value="">Lịch mặc định</option>`;
    const has = list.some((c) => c.id === sel);
    return list.map((c) => `<option value="${esc(c.id)}" ${(has ? c.id === sel : c.default) ? "selected" : ""}>${esc(c.source ? `${c.source} · ${c.title}` : c.title)}</option>`).join("");
  }
  function renderCapture() {
    const d = cap.d, ev = d && d.kind === "event";
    const el = $("#capture");
    if (!d) {
      el.innerHTML = `<button class="x" data-cp="close" title="Đóng">${I.x}</button><div class="cp-head">${I.cal}<b>Tạo việc · lịch</b></div>${srcHtml(cap.info)}<div class="cp-wait"><span class="spin"></span>Đang đọc nội dung…</div>`;
      return;
    }
    const allDay = ev && d.all_day;
    const noGoogle = cap.cals && cap.cals.length && !cap.cals.some((c) => /google|gmail/i.test(c.source));
    el.innerHTML = `<button class="x" data-cp="close" title="Đóng">${I.x}</button>
      <div class="cp-head">${I.cal}<b>Tạo việc · lịch</b><span class="dim cp-eng">${esc(engineName(d.engine))}</span></div>
      ${srcHtml(cap.info)}
      <div class="cp-row">
        <div class="seg"><button data-cp="k-task" class="${ev ? "" : "on"}">Việc</button><button data-cp="k-event" class="${ev ? "on" : ""}">Lịch</button></div>
        <input class="field grow" id="cp-title" placeholder="${ev ? "Tên sự kiện" : "Việc cần làm"}" value="${esc(d.title)}">
      </div>
      <div class="cp-row">
        <input class="field" type="date" id="cp-date" value="${d.start ? ymd(d.start) : ""}">
        <input class="field cp-time" type="time" id="cp-time" value="${d.start && !allDay ? hm(new Date(d.start)) : ""}" ${allDay ? "disabled" : ""}>
        ${ev ? `<span class="dim">–</span><input class="field cp-time" type="time" id="cp-end" value="${d.end && !allDay ? hm(new Date(d.end)) : ""}" ${allDay ? "disabled" : ""}>
          <label class="cp-check"><input type="checkbox" id="cp-allday" ${allDay ? "checked" : ""}>Cả ngày</label>`
        : `<span class="dim cp-hint">${d.start ? "" : "Không hạn"}</span>${d.start ? `<button class="link" data-cp="nodate">Bỏ hạn</button>` : ""}`}
      </div>
      ${ev ? `<div class="cp-row">
        <input class="field grow" id="cp-loc" placeholder="Địa điểm" value="${esc(d.location)}">
        <select class="field cp-cal" id="cp-cal" title="Lịch Google đã thêm trong Cài đặt hệ thống › Tài khoản Internet sẽ tự đồng bộ lên Google Calendar">${calOptions()}</select>
      </div>` : ""}
      <textarea class="field cp-notes" id="cp-notes" rows="2" placeholder="Ghi chú">${esc(d.notes)}</textarea>
      ${ev && noGoogle ? `<div class="cp-tip">Muốn lên Google Calendar? Thêm tài khoản Google trong Cài đặt hệ thống › Tài khoản Internet, rồi chọn lịch Google ở đây.</div>` : ev && cap.calErr ? `<div class="cp-tip err">${esc(cap.calErr)}</div>` : ""}
      <div class="cp-foot"></div>`;
    renderCapFoot();
  }
  function renderCapFoot() {
    const f = $("#capture .cp-foot");
    if (!f || !cap.d) return;
    const ev = cap.d.kind === "event";
    const left = cap.aiBusy
      ? `<span class="cp-ai"><span class="spin"></span>${cap.info && cap.info.kind === "image" && !cap.d.ocr ? "Đang đọc chữ trong ảnh…" : "AI đang đọc…"}</span>`
      : cap.aiDraft ? `<button class="btn sm ghost" data-cp="use-ai">${I.sparkle}Dùng gợi ý AI</button>`
      : cap.err ? `<span class="cp-err">${esc(cap.err)}</span>`
      : `<button class="btn sm ghost" data-cp="ai" title="Đọc lại bằng ${esc(settings.ai_provider && settings.ai_provider !== "apple" ? "AI đã chọn trong Cài đặt" : "Apple Intelligence")}">${I.sparkle}Đọc lại bằng AI</button>`;
    f.innerHTML = `${left}<span class="grow"></span><button class="btn primary" data-cp="create" ${cap.busy ? "disabled" : ""}>${cap.busy ? `<span class="spin"></span>` : ev ? I.cal : I.check}${ev ? "Thêm vào lịch" : "Tạo việc"}</button>`;
  }
  function readCap() {
    const d = cap.d;
    if (!d || !$("#cp-title")) return d;
    d.title = $("#cp-title").value;
    d.notes = $("#cp-notes").value;
    const date = $("#cp-date").value, time = $("#cp-time").value;
    if (d.kind === "event") {
      d.all_day = $("#cp-allday").checked;
      d.location = $("#cp-loc").value;
      d.calendar_id = $("#cp-cal").value;
      if (!date) { d.start = d.end = null; return d; }
      if (d.all_day) { d.start = new Date(`${date}T00:00`).getTime(); d.end = d.start + 86400000; }
      else {
        d.start = new Date(`${date}T${time || "09:00"}`).getTime();
        const e = $("#cp-end").value;
        let end = e ? new Date(`${date}T${e}`).getTime() : 0;
        if (!(end > d.start)) end = d.start + 3600000;
        d.end = end;
      }
    } else {
      d.start = date ? new Date(`${date}T${time || "09:00"}`).getTime() : null;
      d.end = null;
    }
    return d;
  }
  async function createCapture() {
    const d = readCap();
    if (!d.title.trim()) { $("#cp-title").focus(); return toast("Nhập tiêu đề trước đã"); }
    if (d.kind === "event" && !d.start) { $("#cp-date").focus(); return toast("Sự kiện cần ngày"); }
    cap.busy = true; renderCapFoot();
    try {
      const r = await api("capture_create", { d });
      if (d.kind === "event" && d.calendar_id) settings.capture_calendar = d.calendar_id;
      toast(r.message); confetti(10); flashMood("happy");
      if (!r.event) refreshTasks();
      await closeCapture();
    } catch (err) { cap.err = String(err); flashMood("worried", 2500); }
    cap.busy = false;
    if (mode === "capture") renderCapFoot();
  }
  $("#capture").addEventListener("input", () => { cap.dirty = true; });
  $("#capture").addEventListener("change", (e) => {
    cap.dirty = true;
    if (e.target.id === "cp-allday" || e.target.id === "cp-date") { readCap(); if (e.target.id === "cp-allday") renderCapture(); }
  });
  $("#capture").addEventListener("keydown", (e) => {
    if (isComposing(e)) return;
    if (e.key === "Enter" && e.target.id === "cp-title") { e.preventDefault(); createCapture(); }
    if (e.key === "Escape") { e.stopPropagation(); closeCapture(); }
  });
  $("#capture").addEventListener("click", async (e) => {
    const b = e.target.closest("[data-cp]");
    if (!b || b.disabled) return;
    const a = b.dataset.cp;
    if (a === "close") return closeCapture();
    if (a === "k-task" || a === "k-event") {
      readCap();
      const k = a === "k-task" ? "task" : "event";
      if (cap.d.kind === k) return;
      cap.d.kind = k;
      if (k === "event") { if (!cap.d.start) { const t = new Date(); t.setMinutes(0, 0, 0); t.setHours(t.getHours() + 1); cap.d.start = t.getTime(); } if (!cap.d.end || cap.d.end <= cap.d.start) cap.d.end = cap.d.start + 3600000; loadCals(); }
      else cap.d.all_day = false;
      renderCapture();
      return;
    }
    if (a === "nodate") { readCap(); cap.d.start = null; renderCapture(); return; }
    if (a === "ai") return runCapAi(false);
    if (a === "use-ai" && cap.aiDraft) { cap.d = forceKind({ ...cap.aiDraft, calendar_id: cap.d.calendar_id }, cap.kind); cap.aiDraft = null; cap.dirty = false; renderCapture(); return; }
    if (a === "create") return createCapture();
  });

  // ───────── Thanh tab ─────────
  // [id, tên, biểu tượng, mô-đun (null = luôn có)]
  const TABS = [
    ["home", "Asi", I.sparkle, null],
    ["tasks", "Việc", I.tasks, null],
    ["notes", "Ghi chú", I.note, null],
    ["meet", "Họp", I.wave, null],
    ["clips", "Clipboard", I.clip, null],
    ["journal", "Nhật ký", I.book, "journal"],
    ["words", "Từ mới", I.words, "words"],
    ["threads", "Social", I.grid, "threads"], // id giữ "threads" để không mất cài đặt mô-đun cũ
    ["clean", "Dọn máy", I.broom, "clean"],
  ];
  function visibleTabs() {
    const list = TABS.filter(([id, , , m]) => !m || modOn(m));
    if (!list.some(([id]) => id === tab)) { const cur = TABS.find(([id]) => id === tab); if (cur) list.push(cur); }
    return list;
  }
  const tabDot = (id) => !!((id === "tasks" && status && status.lateness.overdue) || (id === "threads" && status && status.threads_unread) || (id === "meet" && meet));
  function renderTabs() {
    const el = $("#tabs");
    const list = visibleTabs();
    const key = list.map(([id]) => id).join(",");
    if (el.dataset.key !== key) {
      el.dataset.key = key;
      // Thanh công cụ kiểu macOS: biểu tượng + tên luôn hiện đủ (nhiều mục thì thu hẹp đều, hẹp quá mới cuộn ngang)
      el.style.setProperty("--n", list.length);
      el.innerHTML = list.map(([id, name, ic], i) => `<button class="tab" data-tab="${id}" title="${name}${i < 9 ? ` (⌘${i + 1})` : ""}"><span class="ti">${ic}<span class="dot"></span></span><span class="lbl">${name}</span></button>`).join("");
    }
    for (const b of el.children) {
      b.classList.toggle("on", b.dataset.tab === tab);
      b.classList.toggle("has-dot", tabDot(b.dataset.tab));
    }
  }
  $("#tabs").addEventListener("click", (e) => { const t = e.target.closest("[data-tab]"); if (t) switchTab(t.dataset.tab); });
  function switchTab(t) {
    if (t === "settings") return openSettings();
    const before = sizeOf("expanded").join("x");
    tab = t;
    if (mode === "expanded" && sizeOf("expanded").join("x") !== before) setMode("expanded", { force: true });
    else renderView();
  }
  const VIEWS = {};
  function renderView() {
    if (tab !== "threads") hideEmbed();
    renderTabs();
    const v = $("#view");
    v.dataset.tab = tab;
    v.scrollTop = 0;
    (VIEWS[tab] || VIEWS.home || (() => {}))();
  }
  // Threads nhúng (thử nghiệm): khung webview đặt chồng lên ô #thSlot
  let embedShown = false;
  function hideEmbed() { if (embedShown) { embedShown = false; api("threads_embed", { show: false, x: 0, y: 0, w: 0, h: 0 }); } }
  function placeEmbed(url) {
    const slot = $("#thSlot");
    if (!slot || mode !== "expanded" || tab !== "threads") return;
    const r = slot.getBoundingClientRect();
    embedShown = true;
    const V = window.VNKey;
    return api("threads_embed", { show: true, x: r.left, y: r.top, w: r.width, h: r.height, ime: V && V.mode ? V.mode : null, url: url || null }).catch((err) => toast(String(err)));
  }

  // ───────── Asi: hỏi đáp (dùng chung cho tab Asi và lớp trả lời trên tai thỏ) ─────────
  const chat = []; // {who: "me"|"asi", text, reply?, thinking?}
  const chatChanged = () => document.dispatchEvent(new Event("asi-chat"));
  function fmtMd(raw) {
    return esc(raw || "")
      .replace(/\*\*(.+?)\*\*/g, "<b>$1</b>")
      .replace(/^[•*-]\s+(.+)$/gm, '<span class="li">$1</span>')
      .replace(/^(\d+)\.\s+(.+)$/gm, '<span class="li n"><b>$1.</b> $2</span>')
      .replace(/\n/g, "<br>");
  }
  // Giờ của sự kiện trong thẻ trả lời: hôm nay → "15:00"; ngày khác → "Mai 15:00", "T6 10:00"; cả ngày → "Cả ngày · 20/11"
  function evWhen(st, allDay) {
    const d = new Date(st), today = d.toDateString() === new Date().toDateString();
    if (allDay) return today ? "Cả ngày" : `Cả ngày · ${pad(d.getDate())}/${pad(d.getMonth() + 1)}`;
    return today || st < Date.now() ? hm(d) : fmtDue(st);
  }
  function cardHtml(c) {
    if (c.kind === "metrics") return `<div class="a-metrics">${c.items.map((m) => `<div class="a-metric lv${m.level}"><span>${esc(m.label)}</span><b>${esc(m.value)}</b>${m.ratio != null ? `<i style="--r:${Math.min(1, m.ratio)}"></i>` : ""}</div>`).join("")}</div>`;
    if (c.kind === "tasks") return `<div class="a-list">${c.items.map((t) => `<div class="a-row" data-tid="${t.id}"><span class="grow">${esc(t.title)}</span>
        ${t.due ? `<span class="due ${t.due < Date.now() ? "late" : ""}">${fmtDue(t.due)}</span>` : ""}
        <button class="ib" data-ta="done" title="Xong">${I.check}</button><button class="ib txt" data-ta="s60" title="Hoãn 1 giờ">+1h</button></div>`).join("")}</div>`;
    if (c.kind === "events") return `<div class="a-list">${c.items.map(([t, st, ad]) => `<div class="a-row"><span class="ev"></span><span class="grow">${esc(t)}</span><span class="due">${evWhen(st, ad)}</span></div>`).join("")}</div>`;
    if (c.kind === "text") return `<div class="a-text">${fmtMd(c.text)}</div>`;
    if (c.kind === "links") return `<div class="a-list">${c.items.map((l) => `<button class="a-row a-link" data-url="${esc(l.url)}" title="${esc(l.url)}"><span class="grow">${esc(l.title)}</span>${l.meta ? `<span class="due">${esc(l.meta)}</span>` : ""}${I.ext}</button>`).join("")}</div>`;
    return "";
  }
  function replyHtml(r) {
    return r.cards.map(cardHtml).join("") + (r.actions.length ? `<div class="a-acts">${r.actions.map((a, i) => `<button class="btn sm ${i === 0 ? "primary" : ""}" data-ac="${esc(a.cmd)}" data-arg="${esc(a.arg)}">${esc(a.label)}</button>`).join("")}</div>` : "")
      + (r.engine ? `<div class="engine">${esc(r.engine)}</div>` : ""); // ai trả lời: Apple Intelligence / ChatGPT · … / Lệnh cơ bản
  }
  async function askAsi(text, compact) {
    text = (text || "").trim();
    if (!text) return;
    chat.push({ who: "me", text });
    chat.push({ who: "asi", text: "", thinking: true });
    compact ? renderReply() : chatChanged();
    moodOverride = "focus"; applyMood();
    let r;
    try { r = await api("asi_ask", { text }); } catch (err) { r = { text: String(err), speak: "", cards: [], actions: [], mood: "worried" }; }
    chat.pop();
    chat.push({ who: "asi", text: r.text, reply: r });
    refreshTasks();
    moodOverride = null;
    flashMood(r.mood || "happy", 2500);
    if (r.speak) api("asi_speak", { text: r.speak });
    compact ? renderReply() : chatChanged();
  }
  let replyTimer = null;
  function renderReply() {
    const last = chat[chat.length - 1];
    const me = [...chat].reverse().find((m) => m.who === "me");
    $("#reply").innerHTML = `<button class="x" data-ac="close" title="Đóng">${I.x}</button>
      <div class="r-me" translate="no">${esc(me ? me.text : "")}</div>
      <div class="r-asi" translate="no">${last.thinking ? '<span class="dots"><i></i><i></i><i></i></span>' : fmtMd(last.text)}</div>
      ${last.reply ? replyHtml(last.reply) : ""}
      ${last.thinking ? "" : `<div class="r-foot"><button class="btn sm" data-ac="more_voice">${I.mic}Nói tiếp</button><button class="btn sm ghost" data-ac="open_chat">Mở Asi</button></div>`}`;
    setMode("reply", { force: mode === "reply" });
    clearTimeout(replyTimer);
    replyTimer = setTimeout(() => { if (mode === "reply" && !hovering) setMode("collapsed"); }, 30000);
  }
  async function voiceCommand() {
    if (voice.active) { await stopDictation(); return; }
    $("#listen").innerHTML = `<div class="ls">${micBtn("listenMic", "Đang nghe")}<div><b>Asi đang nghe…</b><span>Ví dụ: “nhắc tôi 3 giờ chiều gọi khách”</span></div></div>`;
    await setMode("listen");
    api("asi_stop_speaking");
    const btn = $("#listenMic");
    await dictate(btn, (text) => askAsi(text, true));
    if (!voice.active && mode === "listen") setMode("collapsed");
  }
  async function runAction(cmd, arg, compact) {
    if (cmd === "ask") return askAsi(arg, compact);
    if (cmd === "open_tab") { if (arg === "settings" || arg === "ai") return openSettings(arg === "ai" ? "assistant" : null); tab = arg; return setMode("expanded", { force: true }); }
    if (cmd === "open_url") return api("open_link", { url: arg });
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
    compact ? renderReply() : chatChanged();
  }
  async function taskAct(row, a) {
    const id = Number(row.dataset.tid);
    if (a === "done") { await api("toggle_task", { id }); confetti(12); flashMood("happy"); row.classList.add("done"); }
    if (a === "s60") { await api("snooze_task", { id, minutes: 60 }); row.remove(); toast("Đã hoãn 1 giờ"); }
    refreshTasks();
  }
  function bindReplyClicks(el) {
    el.addEventListener("click", (e) => {
      const t = e.target.closest("[data-ta]");
      if (t) return taskAct(t.closest(".a-row"), t.dataset.ta);
      const l = e.target.closest("[data-url]");
      if (l) return api("open_link", { url: l.dataset.url });
      const b = e.target.closest("[data-ac]");
      if (b) runAction(b.dataset.ac, b.dataset.arg || "", el.id === "reply");
    });
  }
  bindReplyClicks($("#reply"));

  // ───────── Bản cập nhật (banner ở tab Asi; chi tiết trong Cài đặt › Chung) ─────────
  let update = null;
  api("update_info").then((v) => { if (v) update = v.status || v; }).catch(() => {});
  function updateBanner() {
    const u = update;
    const rel = u && u.release && u.release.has_update ? u.release : null;
    const ver = (rel && rel.latest_version) || (status && status.update_version);
    if (u && (u.phase === "downloading" || u.phase === "installing")) {
      return `<div class="banner" id="updBanner"><span class="spin"></span><div class="grow"><b>${esc(u.message || "Đang tải bản cập nhật…")}</b><div class="meter"><i style="width:${u.percent || 5}%"></i></div></div></div>`;
    }
    if (u && u.phase === "restarting") return `<div class="banner" id="updBanner"><b class="grow">Đã cập nhật — đang khởi động lại…</b></div>`;
    if (!ver) return "";
    return `<div class="banner" id="updBanner"><div class="grow"><b>Có bản mới ${esc(ver)}</b><span class="dim"> · giữ nguyên dữ liệu</span></div><button class="btn sm primary" data-upd="install">Cập nhật</button></div>`;
  }
  const redrawBanner = () => { const b = $("#updBanner"); if (b) b.outerHTML = updateBanner() || "<span></span>"; };
  on("update-available", () => api("update_info").then((v) => { if (v) update = v.status || v; redrawBanner(); updateBar(); }).catch(() => {}));
  on("update-progress", (u) => {
    update = u.status || u;
    redrawBanner();
    if (update.phase === "restarting") { confetti(30); flashMood("love", 3500); }
    if (update.phase === "error") toast("Chưa cập nhật được: " + (update.error || ""));
  });
  $("#view").addEventListener("click", async (e) => {
    const b = e.target.closest("[data-upd]");
    if (!b) return;
    b.disabled = true;
    b.innerHTML = '<span class="spin"></span>';
    try { await api("update_install"); } catch (err) { toast(String(err)); redrawBanner(); }
  });

  // Apple Intelligence có sẵn không (để tab Asi gợi ý câu hỏi rộng hơn)
  let ai = { available: false };
  api("apple_ai_status").then((r) => { if (r) { ai = r; document.dispatchEvent(new Event("asi-status")); } }).catch(() => {});

  // ───────── Việc: dữ liệu dùng chung ─────────
  async function refreshTasks() {
    tasks = await api("list_tasks");
    document.dispatchEvent(new Event("asi-tasks"));
    if (mode === "peek") renderPeek();
  }

  // ───────── Cài đặt đổi (từ cửa sổ Cài đặt) ─────────
  on("settings-changed", (st) => {
    if (st && window.I18N) I18N.setLang(st.ui_lang);
    if (!st) return;
    if ((st.display_mode || "notch") !== DISP) { setTimeout(() => location.reload(), 120); return; } // đổi kiểu hiển thị → nạp lại
    if (st.viet_engine !== settings.viet_engine && window.VNKey) {
      window.VNKey.userOn = st.viet_engine === "telex" || st.viet_engine === "vni" ? st.viet_engine : null;
      if (mode === "expanded") setTimeout(() => syncIme(true), 0);
    }
    const geoKey = (x) => [x.wing_size, x.notch_w, x.notch_h].join("|");
    const changedGeo = geoKey(st) !== geoKey(settings);
    settings = st;
    if (changedGeo) applyGeometry(settings, mode === "collapsed");
    applyColor(); applyMood(); updateBar();
    if (mode === "expanded") renderTabs();
    if (mode === "peek") renderPeek();
  });

  function onStatus(s) {
    const prevLevel = status ? status.lateness.level : 0;
    status = s;
    applyColor(); applyMood(); updateBar();
    if (mode === "peek") renderPeek();
    if (mode === "expanded") renderTabs();
    document.dispatchEvent(new Event("asi-status"));
    if (s.lateness.level > prevLevel && prevLevel === 0) flashMood("alarm", 1800);
    if (s.lateness.level === 0 && prevLevel > 0) { flashMood("happy", 2500); confetti(14); }
  }
  on("status", onStatus);
  setInterval(() => { if (status && mode !== "expanded" && (status.focus_until || idleItems().includes("time"))) updateBar(); }, 1000);

  // ───────── Xuất cho các file view ─────────
  window.ASI = {
    ...C, toast, confetti, flashMood, dictate, micBtn, geo, views: VIEWS,
    askAsi, voiceCommand, runAction, taskAct, replyHtml, fmtMd, bindReplyClicks, updateBanner,
    updateBar, placeEmbed, hideEmbed, switchTab, threadsInline, setMode, renderView, refreshTasks, openSettings, modOn,
    get composing() { return C.composing; },
    get chat() { return chat; },
    get aiReady() { return !!(ai.available && settings.apple_ai_enabled !== false); },
    get tab() { return tab; }, set tab(t) { tab = t; },
    get mode() { return mode; },
    get status() { return status; }, set status(s) { onStatus(s); },
    get tasks() { return tasks; },
    get settings() { return settings; },
    get DISP() { return DISP; },
    get meet() { return meet; },
    clockOf, openCapture,
    /** Mở bảng tạo việc / lịch từ một đoạn chữ (vd. ý quan trọng trong biên bản họp) */
    async captureText(text, opts = {}) { try { openCapture(await api("capture_set_text", { text }), opts); } catch (err) { toast(String(err)); } },
    saveSettings: async (patch) => { Object.assign(settings, patch); await saveSettings(Object.keys(patch)); },
    takeFreshOpen() { const f = freshOpen; freshOpen = false; return f; },
    applyMood, applyColor,
  };
  document.dispatchEvent(new Event("asi-ready"));
  window.__asi = { setMode, get mode() { return mode; }, set tab(t) { tab = t; }, renderView, showAlert, flashMood, openSheet, openCapture, voiceCommand };

  await refreshTasks();
  onStatus(await api("get_status"));
  await setMode("collapsed", { force: true });
  flashMood("happy", 1800); // chào khi khởi động
  // Chỉ khi xem thử bằng trình duyệt: ?open=peek | ?open=<tab> | ?open=alert-… để mở sẵn
  const devOpen = MOCK ? URL_PARAMS.get("open") : null;
  if (devOpen === "peek") { media = await api("media_status"); await setMode("peek", { force: true }); }
  else if (devOpen === "listen") voiceCommand();
  else if (devOpen === "reply") askAsi("máy mình thế nào", true);
  else if (devOpen === "sheet") openSheet(["/Users/binh/Desktop/Hop-dong.pdf", "/Users/binh/Desktop/Bao-gia.pdf", "/Users/binh/Downloads/Anh.zip"]);
  else if (devOpen === "sheet2") openSheet(["/Users/binh/Desktop/Ảnh chuyến đi", "/Users/binh/Desktop/IMG_0420.HEIC", "/Users/binh/Desktop/Biên bản.md"]);
  else if (devOpen === "drop") { setDropZone("tools"); setMode("drop"); }
  else if (devOpen === "dropc") { $("#drop").classList.add("content"); setDropZone("capture"); setMode("drop"); }
  else if (devOpen === "capture") openCapture(await api("drop_read", { paths: [] }));
  else if (devOpen && devOpen.startsWith("alert-")) {
    const k = devOpen.slice(6);
    showAlert(k === "break" ? { kind: "break", minutes: 52 } : k === "journal" ? { kind: "journal" } : k === "stock" ? { kind: "stock", symbol: "FPT", title: "FPT vượt 100.000đ rồi", text: "Giá hiện tại 100.200đ" } : k === "event" ? { kind: "event", event: { title: "Họp khách hàng", start: Date.now() + 9 * 60000 } } : { kind: "reminder", task: tasks[0] });
  } else if (devOpen) { tab = devOpen; await setMode("expanded", { force: true }); }
})();
