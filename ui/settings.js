/* ───────── Mini-Asi — cửa sổ Cài đặt (settings.html) ─────────
 * Cửa sổ macOS thường (có thanh tiêu đề), tách khỏi tai thỏ để không che hộp thoại xin quyền.
 * Mọi thay đổi gửi bằng `patch_settings` (chỉ các khoá vừa đổi) → backend phát `settings-changed` cho cả 2 cửa sổ.
 * Các mục: Chung · Tai thỏ · Tính năng · Nhắc nhở · Asi & giọng nói · Bộ gõ · Clipboard · Đồng bộ & sao lưu · Quyền · Pro.
 */
(async function () {
  "use strict";
  const C = await window.AsiCommon;
  const { api, on, $, $$, esc, fmtAgo, bytes, I } = C;
  const toast = C.toastIn(document.body);
  const geo = await api("get_geometry");
  const MAC = geo.platform === "macos";
  let st = await api("get_settings");
  let sec = (new URLSearchParams(location.search).get("sec") || "general").replace(/^sec-/, "");

  const SECTIONS = [
    ["general", "Chung", I.gear],
    ["look", "Tai thỏ", I.eye],
    ["features", "Tính năng", I.grid],
    ["reminders", "Nhắc nhở", I.clock],
    ["assistant", "Asi & giọng nói", I.message],
    ["ime", "Bộ gõ tiếng Việt", I.keyboard],
    ["clips", "Clipboard", I.clip],
    ["sync", "Đồng bộ & sao lưu", I.sync],
    ["perms", "Quyền truy cập", I.shield],
    ["pro", "Mini-Asi Pro", I.lock],
  ];
  const DEFAULT_MODULES = ["threads", "clean", "focus"];
  const modules = () => (Array.isArray(st.modules) ? st.modules : DEFAULT_MODULES);

  async function patch(p) {
    try { st = await api("patch_settings", { patch: p }); } catch (err) { toast(String(err)); }
  }

  // ───────── Mẫu dựng giao diện ─────────
  const sw = (key, onv = st[key]) => `<button class="switch ${onv ? "on" : ""}" data-sw="${key}" role="switch" aria-checked="${!!onv}"></button>`;
  const num = (key, min, max, unit) => `<span class="numbox"><input class="field num" type="number" min="${min}" max="${max}" value="${esc(st[key])}" data-num="${key}" />${unit ? `<span>${unit}</span>` : ""}</span>`;
  const txt = (key, w = 140) => `<input class="field" style="width:${w}px" value="${esc(st[key])}" data-txt="${key}" />`;
  const seg = (key, opts, cur = st[key]) => `<div class="seg">${opts.map(([v, n]) => `<button class="${String(cur) === String(v) ? "on" : ""}" data-seg="${key}" data-v="${v}">${n}</button>`).join("")}</div>`;
  const row = (label, desc, ctrl = "") => `<div class="srow"><div class="grow"><b>${label}</b>${desc ? `<span>${desc}</span>` : ""}</div>${ctrl}</div>`;
  const group = (title, rows, foot = "") => `${title ? `<h3>${title}</h3>` : ""}<div class="group">${rows.join("")}</div>${foot ? `<p class="gfoot">${foot}</p>` : ""}`;
  const head = (title, sub) => `<header><h2>${title}</h2>${sub ? `<p>${sub}</p>` : ""}</header>`;

  // ───────── Chung ─────────
  let upd = null;
  function updateRows() {
    const u = (upd && (upd.status || upd)) || {};
    const rel = u.release && u.release.has_update ? u.release : null;
    const ver = (upd && upd.current_version) || "";
    let ctrl = `<button class="btn sm" data-act="upd-check">Kiểm tra</button>`, desc = esc(u.message || "Bạn đang dùng bản mới nhất");
    if (u.phase === "checking") { ctrl = '<span class="spin"></span>'; desc = "Đang kiểm tra…"; }
    if (u.phase === "downloading" || u.phase === "installing") { ctrl = `<span class="dim">${u.percent || 0}%</span>`; desc = esc(u.message || "Đang tải…"); }
    if (u.phase === "restarting") { ctrl = ""; desc = "Đã cập nhật — đang khởi động lại…"; }
    if (u.phase === "error") desc = `<span class="err">${esc(u.error || "Lỗi")}</span>`;
    if (rel && !["downloading", "installing", "restarting"].includes(u.phase)) {
      desc = `Có bản mới ${esc(rel.latest_version)}${rel.asset && rel.asset.size ? ` · ${bytes(rel.asset.size)}` : ""}${rel.notes ? ` — ${esc(rel.notes)}` : ""}`;
      ctrl = `<button class="btn sm primary" data-act="upd-install">Cập nhật</button>`;
    }
    return [row(`Phiên bản ${esc(ver)}`, desc, ctrl), row("Tự báo khi có bản mới", "", sw("auto_update_check", st.auto_update_check !== false))];
  }
  async function secGeneral() {
    upd = await api("update_info").catch(() => null);
    return head("Chung") +
      group("", [
        row("Mở cùng máy", "Mini-Asi có mặt ngay khi bạn đăng nhập", sw("launch_at_login")),
        row("Phím tắt mở nhanh", "Mở/đóng bảng từ bất kỳ đâu", `<kbd>${MAC ? "⌘ ⇧ Space" : "Ctrl ⇧ Space"}</kbd>`),
        row("Chất lượng nén PDF", "Dùng khi kéo PDF vào tai thỏ", `<select class="field" data-txt="pdf_quality">${[["screen", "Nhỏ nhất"], ["ebook", "Cân bằng"], ["printer", "Đẹp (in ấn)"]].map(([v, n]) => `<option value="${v}" ${st.pdf_quality === v ? "selected" : ""}>${n}</option>`).join("")}</select>`),
      ]) +
      `<div id="updGroup">${group("Cập nhật", updateRows())}</div>` +
      group("", [row("Thoát Mini-Asi", "Dữ liệu của bạn chỉ lưu trên máy này", `<button class="btn sm" data-act="quit">Thoát</button>`)]);
  }

  // ───────── Tai thỏ ─────────
  const IDLE = [["overdue", "Số việc trễ"], ["next", "Việc sắp đến hạn (trong 1 giờ)"], ["event", "Lịch sắp diễn ra (trong 15 phút)"], ["focus", "Đồng hồ tập trung"], ["tasks", "Số việc còn lại"], ["time", "Giờ hiện tại"], ["mic", "Nút micro ra lệnh"]];
  function secLook() {
    const disp = st.display_mode || "notch";
    const idle = Array.isArray(st.idle_items) ? st.idle_items : ["focus", "overdue", "event", "next", "tasks", "mic"];
    const h = st.notch_h > 0 ? Math.round(st.notch_h) : Math.round(geo.notch_height || 32);
    const custom = st.bubble_x > 0 || st.bubble_y > 0;
    return head("Tai thỏ", geo.has_notch ? `Màn hình có tai thỏ ${Math.round(geo.notch_width)}×${Math.round(geo.notch_height)}` : "Màn hình không có tai thỏ — Mini-Asi tạo tai thỏ ảo") +
      `<h3>Kiểu hiển thị</h3><div class="disp">${[["notch", "Tai thỏ", "Ôm sát mép trên"], ["pill", "Viên thuốc", "Nổi dưới thanh menu"], ["bubble", "Bong bóng", "Nút tròn, kéo đi đâu cũng được"]]
        .map(([id, n, d]) => `<button class="disp-opt ${disp === id ? "on" : ""}" data-disp="${id}"><span class="pv pv-${id}"></span><b>${n}</b><span>${d}</span></button>`).join("")}</div>` +
      group("Kích thước", [
        row("Độ rộng hai bên", "Mẹo: kéo cạnh tai thỏ để chỉnh trực tiếp", seg("wing_size", [["hidden", "Ẩn"], ["xs", "Rất gọn"], ["s", "Gọn"], ["m", "Vừa"], ["l", "Rộng"]].concat(st.wing_size === "custom" ? [["custom", "Tự chỉnh"]] : []), st.wing_size || "s")),
        row("Chiều cao", "", `<input type="range" id="notchH" min="22" max="54" step="1" value="${h}" /><span class="dim val" id="notchHVal">${h} px</span>`),
        row("Vị trí", custom ? "Đang ở vị trí bạn kéo tới" : "Mặc định — kéo hòn đảo để đổi chỗ", custom ? `<button class="btn sm" data-act="reset-pos">Về chỗ cũ</button>` : ""),
      ]) +
      group("Khi thu gọn, hiện", IDLE.map(([id, n]) => `<label class="srow check-row"><input type="checkbox" data-idle="${id}" ${idle.includes(id) ? "checked" : ""}/><span class="cbox">${I.check}</span><b>${n}</b></label>`), "Chỉ hiện những gì quan trọng — cánh tai thỏ hẹp sẽ tự bỏ bớt mục cuối.");
  }

  // ───────── Tính năng (mô-đun) ─────────
  function secFeatures() {
    const m = modules();
    const mod = (id, n, d, extra = "") => `<div class="srow"><div class="grow"><b>${n}</b><span>${d}</span></div>${extra}<button class="switch ${m.includes(id) ? "on" : ""}" data-mod="${id}"></button></div>`;
    return head("Tính năng", "Asi, Việc, Ghi chú và Clipboard luôn có. Bật thêm thứ bạn thật sự dùng — bảng sẽ gọn hơn.") +
      group("", [
        mod("clean", "Dọn máy", "Quét cache, nhật ký, file cài đặt cũ để giải phóng ổ đĩa"),
        mod("focus", "Tập trung", "Nút hẹn giờ 25 phút trên tai thỏ — Asi tạm im lặng"),
        mod("threads", "Threads", "Mở Threads trong cửa sổ nhỏ, xem thống kê & trả lời"),
        mod("journal", "Nhật ký", "Mỗi tối Asi hỏi 3 câu về ngày của bạn", m.includes("journal") ? `<span class="inline dim">lúc</span>${num("journal_hour", 17, 23, "giờ")}` : ""),
        mod("words", "Từ mới tiếng Anh", "2–3 từ mỗi lần mở bảng, có chấm phát âm", m.includes("words") ? seg("vocab_count", [[2, "2 từ"], [3, "3 từ"]], st.vocab_count || 3) : ""),
      ]) +
      (m.includes("threads") && MAC ? group("Thử nghiệm", [row("Threads nhúng trong bảng", "Thay vì mở cửa sổ riêng", sw("threads_embed_beta"))]) : "");
  }

  // ───────── Nhắc nhở ─────────
  function secReminders() {
    return head("Nhắc nhở", "Asi nhắc nhẹ nhàng trên tai thỏ — không bật thông báo ồn ào.") +
      group("", [
        row("Nghỉ mắt", "Sau mỗi phiên dùng máy liên tục", `${num("break_minutes", 10, 180, "phút")}${sw("break_enabled")}`),
        row("Uống nước", "Linh vật vẫy nhẹ, không làm phiền", `${num("water_minutes", 15, 240, "phút")}${sw("water_enabled")}`),
        row("Giờ đi ngủ", "Sau giờ này linh vật ngáp ngủ", num("sleep_hour", 18, 23, "giờ")),
      ]) +
      group("", [row("Không làm phiền", "Tạm tắt báo đỏ khi trễ việc và nhắc nghỉ", sw("focus_mode"))]);
  }

  // ───────── Asi & giọng nói ─────────
  let tts = null, ai = null, qsel = null;
  function voiceRows() {
    if (!tts) return [];
    const cur = (e, v) => tts.engine === e && tts.voice === v;
    const voice = (engine, [name, g, desc], ready) => `<div class="srow voice ${cur(engine, name) ? "on" : ""}">
        <span class="vav">${g === "female" ? "Nữ" : "Nam"}</span><div class="grow"><b>${esc(name)}</b><span>${esc(desc)}</span></div>
        <button class="ib" data-tts="hear" data-engine="${engine}" data-voice="${esc(name)}" title="Nghe thử" ${ready ? "" : "disabled"}>${I.play}</button>
        <button class="btn sm ${cur(engine, name) ? "primary" : ""}" data-tts="use" data-engine="${engine}" data-voice="${esc(name)}" ${ready ? "" : "disabled"}>${cur(engine, name) ? "Đang dùng" : "Dùng"}</button></div>`;
    const qs = tts.qualities || [];
    const out = [];
    for (const p of tts.packs) {
      const s = p.status, ready = s.phase === "ready";
      let ctrl = "";
      if (s.phase === "none" || s.phase === "error") {
        const q = qsel || tts.default_quality || "compact";
        ctrl = `${p.id === "vieneu" && qs.length ? seg("_q", qs.map(([id, n, mb]) => [id, `${n} · ${mb} MB`]), q) : `<span class="dim">≈ ${p.model_mb} MB</span>`}<button class="btn sm primary" data-tts="install" data-engine="${p.id}">Tải về</button>`;
      } else if (s.phase === "installing") ctrl = `<span class="spin"></span><span class="dim prog" data-prog="${p.id}">${esc(s.step || "Đang cài…")}</span>`;
      else ctrl = `<button class="link" data-tts="uninstall" data-engine="${p.id}">Gỡ</button>`;
      out.push(`<div class="srow pack"><div class="grow"><b>${esc(p.name)}</b><span>${esc(p.blurb)}${s.phase === "error" ? ` · <span class="err">${esc(s.error || "Lỗi")}</span>` : ""}</span></div>${ctrl}</div>`);
      if (ready) for (const v of p.voices) out.push(voice(p.id, v, true));
    }
    out.push(`<div class="srow voice ${tts.engine === "system" ? "on" : ""}"><span class="vav">Máy</span><div class="grow"><b>Giọng có sẵn của máy</b><span>Không cần tải · offline</span></div>
      <button class="ib" data-tts="hear" data-engine="system" data-voice="" title="Nghe thử">${I.play}</button>
      <button class="btn sm ${tts.engine === "system" ? "primary" : ""}" data-tts="use" data-engine="system" data-voice="">${tts.engine === "system" ? "Đang dùng" : "Dùng"}</button></div>`);
    return out;
  }
  async function secAssistant() {
    [tts, ai] = await Promise.all([api("tts_info").catch(() => null), api("apple_ai_status").catch(() => null)]);
    const aiOk = ai && ai.available;
    return head("Asi & giọng nói") +
      group("Trí tuệ", [row("Apple Intelligence", esc(ai ? ai.detail : "Không kiểm tra được"), aiOk ? sw("apple_ai_enabled", st.apple_ai_enabled !== false) : `<span class="tag">Chưa hỗ trợ</span>`)],
        aiOk ? "Câu hỏi được xử lý ngay trên máy — không gửi lên mạng." : "Cần macOS 26 trở lên trên máy Apple Silicon. Khi chưa có, Asi vẫn hiểu các lệnh cơ bản (thêm việc, nhắc, dọn máy…) hoàn toàn offline.") +
      group("Nghe & nói", [
        row("Ngôn ngữ khi nói với Asi", "Dùng cho ghi chú, thêm việc, nhật ký bằng giọng nói", `<select class="field" data-txt="voice_lang">${[["vi-VN", "Tiếng Việt"], ["en-US", "English (US)"], ["en-GB", "English (UK)"]].map(([v, n]) => `<option value="${v}" ${st.voice_lang === v ? "selected" : ""}>${n}</option>`).join("")}</select>`),
        row("Đọc to câu trả lời", "Asi nói khi trả lời lệnh giọng nói", sw("voice_reply")),
      ]) +
      `<div id="ttsGroup">${group("Giọng đọc", voiceRows())}</div>`;
  }
  const redrawTts = async () => { tts = await api("tts_info").catch(() => tts); const g = $("#ttsGroup"); if (g) g.innerHTML = group("Giọng đọc", voiceRows()); };
  on("tts-progress", (ev) => {
    const s = ev.status || ev;
    if (s.phase === "ready" || s.phase === "error") { toast(s.phase === "ready" ? "Đã cài giọng đọc mới" : "Chưa cài được giọng đọc"); return redrawTts(); }
    const el = $(`[data-prog="${ev.engine || "vieneu"}"]`);
    if (el) el.textContent = s.step || ""; else redrawTts();
  });

  // ───────── Bộ gõ ─────────
  function secIme() {
    return head("Bộ gõ tiếng Việt", "Khi mở bảng, Asi tạm chuyển bàn phím sang ABC và tự gõ dấu — đóng bảng sẽ trả lại bộ gõ của bạn.") +
      group("", [row("Kiểu gõ", "Tự động: theo bộ gõ bạn đang dùng (Telex/VNI)", seg("viet_engine", [["auto", "Tự động"], ["telex", "Telex"], ["vni", "VNI"], ["off", "Tắt"]], st.viet_engine || "auto"))],
        "Trên thanh bảng có nút VI/EN để bật/tắt nhanh · giữ ⌥ khi bấm để đổi Telex ↔ VNI.");
  }

  // ───────── Clipboard ─────────
  async function secClips() {
    const ax = await api("accessibility_status").catch(() => false);
    return head("Clipboard") +
      group("", [
        row("Lưu lịch sử", "Chữ, ảnh, file — tự bỏ qua mật khẩu từ trình quản lý mật khẩu", sw("clip_enabled")),
        row("Giữ tối đa", "Mục đã ghim luôn được giữ", num("clip_max", 20, 2000, "mục")),
        row("Bấm là dán luôn", ax ? "Đã có quyền Trợ năng" : "Cần quyền Trợ năng", `${ax ? "" : `<button class="link" data-act="ax">Cấp quyền</button>`}${sw("clip_autopaste")}`),
        row("Phím tắt mở Clipboard", "", `<kbd>${esc((st.clip_shortcut || "").replace("Control", "⌃").replace("Shift", "⇧").replace("Alt", "⌥").replace(/\+/g, " "))}</kbd>`),
      ]);
  }

  // ───────── Đồng bộ & sao lưu ─────────
  async function secSync() {
    const [bk, si] = await Promise.all([api("backup_info"), MAC ? api("sync_info").catch(() => null) : null]);
    let html = head("Đồng bộ & sao lưu");
    if (MAC) {
      html += group("Ứng dụng Apple", [
        row("Việc ⇄ Lời nhắc", `Danh sách “${esc(st.reminders_list)}”`, `${txt("reminders_list", 120)}${sw("sync_reminders")}`),
        row("Ghi chú ⇄ Ghi chú", `Thư mục “${esc(st.notes_folder)}”`, `${txt("notes_folder", 120)}${sw("sync_notes")}`),
        row("Lịch", "Hiện lịch hôm nay trong tab Việc, nhắc trước giờ họp", `${num("event_alert_minutes", 1, 60, "phút")}${sw("show_calendar")}`),
        row("Lần đồng bộ cuối", si && si.last_run ? `${fmtAgo(si.last_run)} · ${si.linked_tasks} việc · ${si.linked_notes} ghi chú${si.last_error ? ` · <span class="err">${esc(si.last_error)}</span>` : ""}` : "Chưa đồng bộ", `<button class="btn sm" data-act="sync">Đồng bộ ngay</button>`),
      ]);
    }
    html += group("Sao lưu lên đám mây", [
      row("Thư mục", bk.current ? "Bản sao lưu nằm trong thư mục “Mini-Asi” ở đây" : "Chọn Google Drive, iCloud… để bật sao lưu", `<select class="field" id="bkDir">${bk.dirs.map((d) => `<option value="${esc(d.path)}" ${bk.current === d.path ? "selected" : ""}>${esc(d.label)}</option>`).join("")}
        ${bk.chosen && !bk.dirs.some((d) => d.path === bk.chosen) ? `<option value="${esc(bk.chosen)}" selected>${esc(bk.chosen.split(/[\\/]/).pop())}</option>` : ""}<option value="__pick">Chọn thư mục khác…</option></select>`),
      row("Tự sao lưu mỗi ngày", `Lần cuối: ${fmtAgo(bk.last_backup)}`, `<button class="btn sm" data-act="bknow" ${bk.current ? "" : "disabled"}>Sao lưu ngay</button><button class="switch ${bk.auto_backup ? "on" : ""}" data-bk="auto_backup"></button>`),
      row("Đồng bộ giữa các máy", `${esc(bk.device_name)} · lần cuối ${fmtAgo(bk.last_device_sync)}`, `<button class="btn sm" data-act="devsync" ${bk.current ? "" : "disabled"}>Đồng bộ ngay</button><button class="switch ${bk.device_sync ? "on" : ""}" data-bk="device_sync"></button>`),
    ]);
    if (bk.snapshots.length) {
      html += group("Khôi phục", bk.snapshots.slice(0, 5).map((f) => row(esc(new Date(f.modified).toLocaleString("vi-VN")), `${esc(f.device || "")} · ${bytes(f.size)}`, `<button class="btn sm ghost" data-act="restore" data-p="${esc(f.path)}">Khôi phục</button>`)),
        "Khôi phục sẽ thay dữ liệu hiện tại — bản hiện tại được cất riêng để phòng khi cần.");
    }
    return html;
  }

  // ───────── Quyền ─────────
  async function secPerms() {
    const perms = await api("permissions_status");
    const missing = perms.filter((p) => p.state !== "granted").length;
    return head("Quyền truy cập", "Mini-Asi chỉ xin những quyền cần cho tính năng bạn dùng.") +
      group("", perms.map((p) => row(`${esc(p.name)}${p.required === false ? '<span class="tag">Tuỳ chọn</span>' : ""}`, esc(p.why),
        p.state === "granted" ? `<span class="tag green">Đã cấp</span>` : `<button class="btn sm" data-act="perm" data-p="${p.id}">${p.state === "denied" && ["accessibility", "disk"].includes(p.id) ? "Mở Cài đặt hệ thống" : "Cấp quyền"}</button>`))) +
      (missing > 1 ? `<div class="gact"><button class="btn primary" data-act="permall">Cấp tất cả</button></div>` : "");
  }

  // ───────── Pro ─────────
  async function secPro() {
    const lic = await api("license_info");
    if (lic.pro) {
      return head("Mini-Asi Pro", "Cảm ơn bạn đã ủng hộ!") +
        group("", [row("Đã kích hoạt", `${esc(lic.email)}${lic.expires ? ` · hết hạn ${new Date(lic.expires * 1000).toLocaleDateString("vi-VN")}` : " · vĩnh viễn"}`, `<button class="btn sm ghost" data-act="unlic">Gỡ khoá</button>`)]);
    }
    return head("Mini-Asi Pro", "Dọn sâu hơn: file cài đặt cũ, thư mục build, file lớn, bản sao lưu iPhone cũ.") +
      group("Kích hoạt", [`<div class="srow"><input class="field grow" id="licKey" placeholder="Dán khoá bản quyền MASI1.…" /><button class="btn primary sm" data-act="lic">Kích hoạt</button></div>`], '<span id="licMsg" class="err"></span>');
  }

  const RENDER = { general: secGeneral, look: secLook, features: secFeatures, reminders: secReminders, assistant: secAssistant, ime: secIme, clips: secClips, sync: secSync, perms: secPerms, pro: secPro };
  function renderNav() {
    $("#nav").innerHTML = SECTIONS.map(([id, n, ic]) => `<button class="nav-it ${id === sec ? "on" : ""}" data-sec="${id}">${ic}<span>${n}</span></button>`).join("");
  }
  let renderSeq = 0;
  async function render(keepScroll = false) {
    if (!RENDER[sec]) sec = "general";
    renderNav();
    const my = ++renderSeq;
    const html = await RENDER[sec]();
    if (my !== renderSeq) return; // đã chuyển mục khác trong lúc chờ
    const m = $("#main"), y = m.scrollTop;
    m.innerHTML = html;
    m.scrollTop = keepScroll ? y : 0;
  }

  // ───────── Sự kiện ─────────
  $("#nav").addEventListener("click", (e) => { const b = e.target.closest("[data-sec]"); if (b) { sec = b.dataset.sec; render(); } });
  const main = $("#main");
  main.addEventListener("click", async (e) => {
    const t = e.target;
    const s = t.closest("[data-sw]");
    if (s) {
      const k = s.dataset.sw, val = !s.classList.contains("on");
      s.classList.toggle("on", val);
      await patch({ [k]: val });
      if (["sync_reminders", "sync_notes", "show_calendar"].includes(k) && val) { toast("macOS có thể hỏi quyền — bấm Cho phép nhé"); api("sync_now").then(() => sec === "sync" && render(true)).catch(() => {}); }
      return;
    }
    const md = t.closest("[data-mod]");
    if (md) {
      const id = md.dataset.mod, set = new Set(modules()), val = !set.has(id);
      val ? set.add(id) : set.delete(id);
      const p = { modules: [...set] };
      if (id === "journal") p.journal_enabled = val;
      if (id === "words") p.vocab_enabled = val;
      await patch(p);
      return render(true);
    }
    const sg = t.closest("[data-seg]");
    if (sg) {
      const k = sg.dataset.seg, raw = sg.dataset.v;
      if (k === "_q") { qsel = raw; return redrawTts(); }
      const v = /^\d+$/.test(raw) ? Number(raw) : raw;
      const p = { [k]: v };
      if (k === "wing_size") p.notch_w = { hidden: 0, xs: 28, s: 56, m: 88, l: 128 }[v] ?? st.notch_w;
      await patch(p);
      return render(true);
    }
    const dp = t.closest("[data-disp]");
    if (dp) { await api("set_display", { mode: dp.dataset.disp }); st = await api("get_settings"); return render(true); }
    const bk = t.closest("[data-bk]");
    if (bk) {
      const val = !bk.classList.contains("on");
      bk.classList.toggle("on", val);
      await api("backup_set", { [bk.dataset.bk === "auto_backup" ? "autoBackup" : "deviceSync"]: val });
      if (bk.dataset.bk === "device_sync" && val) api("device_sync_now").then(() => render(true)).catch((err) => toast(String(err)));
      return;
    }
    const tt = t.closest("[data-tts]");
    if (tt && !tt.disabled) {
      const a = tt.dataset.tts, engine = tt.dataset.engine || "vieneu", voice = tt.dataset.voice || "";
      if (a === "install") { try { await api("tts_install", { engine, quality: qsel || null }); } catch (err) { toast(String(err)); } qsel = null; setTimeout(redrawTts, 200); return; }
      if (a === "uninstall") {
        if (tt.dataset.sure !== "1") { tt.dataset.sure = "1"; tt.textContent = "Bấm lần nữa để gỡ"; tt.classList.add("danger"); return; }
        await api("tts_uninstall", { engine }); toast("Đã gỡ giọng đọc"); return redrawTts();
      }
      if (a === "use" || a === "hear") {
        await api("tts_use", { engine, voice, preview: a === "hear" });
        if (a === "use") toast(voice ? `Đã chọn giọng ${voice}` : "Đã chọn giọng của máy");
        return redrawTts();
      }
    }
    const b = t.closest("[data-act]");
    if (!b || b.disabled) return;
    const a = b.dataset.act;
    const busy = () => { b.disabled = true; b.innerHTML = '<span class="spin"></span>'; };
    try {
      if (a === "quit") return api("quit");
      if (a === "upd-check") { busy(); upd = await api("update_check"); const g = $("#updGroup"); if (g) g.innerHTML = group("Cập nhật", updateRows()); return; }
      if (a === "upd-install") { busy(); await api("update_install"); return; }
      if (a === "reset-pos") { await api("move_bubble", { x: 0, y: 0, save: true }); st = await api("get_settings"); toast("Đã đưa về vị trí mặc định"); return render(true); }
      if (a === "ax") return api("open_privacy", { pane: "accessibility" });
      if (a === "perm" || a === "permall") { busy(); await api("request_permissions", { which: a === "permall" ? "all" : b.dataset.p }); return render(true); }
      if (a === "sync") {
        busy();
        const r = await api("sync_now");
        toast(r.errors.length ? r.errors[0] : `Xong: ↑${r.tasks_pushed + r.notes_pushed} ↓${r.tasks_pulled + r.notes_pulled} · ${r.events} sự kiện`);
        return render(true);
      }
      if (a === "bknow") { busy(); const [p, n] = await api("backup_now"); toast(`Đã sao lưu ${bytes(n)} lên ${p.includes("Google") ? "Google Drive" : "thư mục đám mây"}`); return render(true); }
      if (a === "devsync") {
        busy();
        const r = await api("device_sync_now");
        const n = r.stats.tasks + r.stats.notes + r.stats.journal + r.stats.clips;
        toast(r.devices.length ? `Đã gộp ${n} mục từ ${r.devices.join(", ")}` : "Chưa thấy máy nào khác — cài Mini-Asi trên máy kia và bật đồng bộ");
        return render(true);
      }
      if (a === "restore") {
        if (b.dataset.sure !== "1") { b.dataset.sure = "1"; b.textContent = "Bấm lần nữa để khôi phục"; b.classList.remove("ghost"); b.classList.add("danger"); return; }
        busy(); await api("backup_restore", { path: b.dataset.p }); toast("Đã khôi phục (bản cũ được cất riêng)"); return render(true);
      }
      if (a === "lic") {
        const r = await api("activate_license", { key: $("#licKey").value.trim() });
        if (r.pro) { toast("Đã kích hoạt Mini-Asi Pro 🎉"); return render(); }
        $("#licMsg").textContent = r.error || "Khoá không hợp lệ";
        return;
      }
      if (a === "unlic") { await api("deactivate_license"); return render(); }
    } catch (err) { toast(String(err)); render(true); }
  });
  main.addEventListener("change", async (e) => {
    const t = e.target;
    if (t.dataset.num) { await patch({ [t.dataset.num]: Number(t.value) }); toast("Đã lưu"); return; }
    if (t.dataset.txt) { await patch({ [t.dataset.txt]: t.value }); toast("Đã lưu"); return; }
    if (t.dataset.idle) {
      const cur = new Set(Array.isArray(st.idle_items) ? st.idle_items : ["focus", "overdue", "event", "next", "tasks", "mic"]);
      t.checked ? cur.add(t.dataset.idle) : cur.delete(t.dataset.idle);
      await patch({ idle_items: IDLE.map(([id]) => id).filter((id) => cur.has(id)) });
      return;
    }
    if (t.id === "notchH") { await patch({ notch_h: Number(t.value) }); return; }
    if (t.id === "bkDir") { if (t.value === "__pick") await api("backup_pick_dir"); else await api("backup_set", { dir: t.value }); return render(true); }
  });
  main.addEventListener("input", (e) => { if (e.target.id === "notchH") $("#notchHVal").textContent = e.target.value + " px"; });

  on("settings-changed", (n) => { if (n) st = n; });
  on("settings-section", (s) => { if (s) { sec = s.replace(/^sec-/, ""); render(); } });
  on("update-progress", (u) => { upd = { ...(upd || {}), status: u.status || u }; const g = $("#updGroup"); if (g) g.innerHTML = group("Cập nhật", updateRows()); });

  api("update_info").then((v) => { if (v) $("#ver").textContent = "Mini-Asi " + (v.current_version || ""); }).catch(() => {});
  render();
})();
