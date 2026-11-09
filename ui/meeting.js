// Tab Họp: ghi âm cuộc họp (micro + âm thanh máy) → chuyển chữ (Apple, trên máy) → biên bản (Apple Intelligence / Gemini).
// Đăng ký view qua window.ASI (xem app.js). Phần nặng nằm ở src-tauri/src/meeting.rs và commands/meeting.rs.
(function () {
  "use strict";
  function boot() {
    const A = window.ASI;
    const { api, on, $, esc, I, toast, confetti, flashMood, pad } = A;
    const view = $("#view");
    let sel = null, detail = null, list = null, progress = "", checked = new Set(), editing = false, starting = false;
    const live = []; // vài dòng mới nhất khi đang ghi
    let sysPref = null;

    const fmtDate = (ms) => { const d = new Date(ms); return `${pad(d.getDate())}/${pad(d.getMonth() + 1)} · ${pad(d.getHours())}:${pad(d.getMinutes())}`; };
    const dur = (s) => { s = Math.round(s || 0); return s >= 3600 ? `${Math.floor(s / 3600)} giờ ${Math.floor(s / 60) % 60} phút` : s >= 60 ? `${Math.floor(s / 60)} phút` : `${s} giây`; };
    const whoChip = (w) => (w === "me" ? `<span class="mt-who me">Bạn</span>` : w === "others" ? `<span class="mt-who">Người khác</span>` : "");
    const brainName = () => (A.settings.meeting_ai === "gemini" ? "Gemini" : "Apple Intelligence");
    const sttName = (s) => (s === "on-device" ? "Chuyển chữ trên máy" : "Chuyển chữ qua máy chủ Apple (cần mạng)");
    const here = () => A.tab === "meet" && A.mode === "expanded";

    // ───────── trang chính ─────────
    function recCard(m) {
      const lines = live.slice(-4);
      return `<div class="mt-rec">
          <div class="mt-rec-top"><i class="rec-dot"></i><b class="grow">${esc(m.title || "Cuộc họp")}</b><span class="mt-time" id="mtTime">${A.clockOf(m.secs)}</span></div>
          <div class="mt-meters"><span>Micro</span><i><b id="mtMic"></b></i>${m.system_audio ? `<span>Máy</span><i><b id="mtSys"></b></i>` : ""}</div>
          <div class="mt-live">${lines.length ? lines.map((l) => `<div>${whoChip(l.who)}${esc(l.text)}</div>`).join("") : `<div class="dim">Chữ sẽ hiện sau mỗi ~30–45 giây…</div>`}</div>
          <div class="mt-rec-foot"><span class="dim grow">${sttName(m.stt)} · ${m.system_audio ? "micro + âm thanh máy" : "chỉ micro"}${m.pending ? ` · đang chuyển ${m.pending} đoạn` : ""}</span>
            <button class="btn danger" data-mt="stop">${I.stop}Dừng ghi</button></div>
        </div>`;
    }
    function startCard() {
      const sys = sysPref ?? A.settings.meeting_system_audio !== false;
      return `<div class="mt-start">
          <button class="mt-big" data-mt="start" ${starting ? "disabled" : ""}>${starting ? `<span class="spin"></span>` : `<i class="rec-dot"></i>`}<span>Ghi cuộc họp</span></button>
          <div class="grow">
            <label class="mt-check"><input type="checkbox" id="mtSys" ${sys ? "checked" : ""}><span><b>Thu cả âm thanh máy</b> — giọng người khác trong Zoom, Meet, Teams (kể cả khi đeo tai nghe)</span></label>
            <div class="dim mt-hint">Họp trực tiếp chỉ cần micro. Chuyển chữ bằng Apple, tóm tắt bằng ${brainName()} — <button class="link inline" data-mt="settings">đổi trong Cài đặt</button>. Nhớ báo mọi người là đang ghi âm.</div>
          </div>
        </div>`;
    }
    async function drawHome() {
      if (!list) { view.innerHTML = `<div class="mt">${A.meet ? recCard(A.meet) : startCard()}<div class="empty sm"><span class="spin"></span></div></div>`; list = await api("meeting_list").catch(() => []); if (!here() || sel) return; }
      const m = A.meet;
      const rows = list.filter((x) => !m || x.id !== m.id);
      view.innerHTML = `<div class="mt">${m ? recCard(m) : startCard()}
        <div class="sec">Các cuộc họp<span>${rows.length || ""}</span></div>
        ${rows.length ? rows.map((x) => `<button class="row mt-row" data-open="${esc(x.id)}">
            <span class="mt-ic">${I.wave}</span>
            <span class="grow"><b>${esc(x.title)}</b><span class="dim">${fmtDate(x.started)} · ${dur(x.duration)}${x.summarized ? ` · ${x.actions} việc` : x.lines ? " · chưa tóm tắt" : " · chưa có chữ"}</span></span>
            ${I.chevron}</button>`).join("") : `<div class="empty sm">Chưa có cuộc họp nào. Bấm “Ghi cuộc họp” khi bắt đầu họp.</div>`}
      </div>`;
      paintLevel();
    }
    function paintLevel() {
      const m = A.meet;
      if (!m) return;
      const t = $("#mtTime"); if (t) t.textContent = A.clockOf(m.secs);
      const a = $("#mtMic"); if (a) a.style.width = `${Math.round((m.mic || 0) * 100)}%`;
      const b = $("#mtSys"); if (b) b.style.width = `${Math.round((m.sys || 0) * 100)}%`;
    }

    // ───────── chi tiết một cuộc họp ─────────
    function sumHtml(s) {
      if (!s) return detail.lines.length
        ? `<div class="mt-empty-sum"><span class="dim">Chưa có biên bản.</span><button class="btn sm primary" data-mt="sum">${I.sparkle}Tóm tắt bằng ${brainName()}</button></div>`
        : "";
      const list = (t, xs) => (xs.length ? `<div class="sec">${t}</div><ul class="mt-ul">${xs.map((x) => `<li>${esc(x)}</li>`).join("")}</ul>` : "");
      return `${s.note ? `<div class="banner warn">${esc(s.note)}</div>` : ""}
        ${list("Tóm tắt", s.overview)}${list("Quyết định", s.decisions)}
        ${s.actions.length ? `<div class="sec">Việc cần làm<span>${s.actions.length}</span><button class="link" data-mt="all">${checked.size === s.actions.length ? "Bỏ chọn" : "Chọn hết"}</button></div>
          ${s.actions.map((a, i) => `<label class="row mt-act"><input type="checkbox" data-ai="${i}" ${checked.has(i) ? "checked" : ""}><span class="grow">${esc(a.task)}${a.owner || a.due ? `<span class="dim">${[a.owner, a.due && `hạn ${a.due}`].filter(Boolean).map(esc).join(" · ")}</span>` : ""}</span></label>`).join("")}
          <div class="mt-act-bar"><button class="btn sm primary" data-mt="tasks" ${checked.size ? "" : "disabled"}>${I.plus}Thêm ${checked.size || ""} việc vào tab Việc</button></div>` : ""}
        <div class="engine">${esc(s.engine)}</div>`;
    }
    function drawDetail() {
      const d = detail;
      if (!d) { view.innerHTML = `<div class="empty"><span class="spin"></span></div>`; return; }
      const s = d.summary;
      view.innerHTML = `<div class="mt">
        <div class="mt-dhead">
          <button class="ib" data-mt="back" title="Về danh sách">${I.chevron}</button>
          ${editing ? `<input class="field grow" id="mtTitle" value="${esc(d.title)}">` : `<b class="grow mt-title" data-mt="rename" title="Bấm để đổi tên">${esc(d.title)}</b>`}
          ${d.recording ? `<span class="badge rec"><i class="rec-dot"></i>Đang ghi</span>` : ""}
        </div>
        <div class="dim mt-meta">${fmtDate(d.started)} · ${dur(d.duration)} · ${d.system_audio ? "micro + âm thanh máy" : "micro"} · ${d.stt === "on-device" ? "chuyển chữ trên máy" : "chuyển chữ qua Apple"}${d.pending ? ` · còn ${d.pending} đoạn đang chuyển` : ""}${d.failed ? ` · <span class="err">${d.failed} đoạn lỗi${d.last_error ? `: ${esc(d.last_error)}` : ""}</span>` : ""}</div>
        <div class="mt-tools">
          ${s ? `<button class="btn sm" data-mt="sum" ${progress || d.recording ? "disabled" : ""}>${I.sparkle}Tóm tắt lại</button>
            <div class="seg"><button data-eng="apple" class="${A.settings.meeting_ai !== "gemini" ? "on" : ""}">Apple</button><button data-eng="gemini" class="${A.settings.meeting_ai === "gemini" ? "on" : ""}">Gemini</button></div>` : ""}
          <span class="grow"></span>
          <button class="ib" data-mt="note" title="Lưu biên bản vào Ghi chú" ${d.lines.length ? "" : "disabled"}>${I.note}</button>
          <button class="ib" data-mt="copy" title="Chép biên bản" ${d.lines.length ? "" : "disabled"}>${I.copy}</button>
          ${d.has_audio ? `<button class="ib" data-mt="audio" title="Mở file ghi âm trong Finder">${I.folder}</button>` : ""}
          ${d.recording ? "" : `<button class="ib" data-mt="del" title="Xoá cuộc họp">${I.trash}</button>`}
        </div>
        ${progress ? `<div class="mt-progress"><span class="spin"></span>${esc(progress)}</div>` : ""}
        ${sumHtml(s)}
        <div class="sec">Toàn văn<span>${d.lines.length || ""}</span></div>
        <div class="mt-lines" id="mtLines">${d.lines.length ? d.lines.map(lineHtml).join("") : `<div class="empty sm">${d.recording || d.pending ? "Chữ đang được chuyển…" : "Không có lời nói nào được nhận dạng."}</div>`}</div>
      </div>`;
      if (editing) { const t = $("#mtTitle"); t.focus(); t.select(); }
    }
    const lineHtml = (l) => `<div class="mt-line"><span class="mt-at">${A.clockOf(l.at)}</span>${whoChip(l.who)}<span>${esc(l.text)}</span></div>`;

    async function openDetail(id) {
      sel = id; detail = null; checked = new Set(); editing = false;
      drawDetail();
      try { detail = await api("meeting_get", { id }); } catch (err) { toast(String(err)); sel = null; return draw(); }
      if (detail.summary) checked = new Set(detail.summary.actions.map((_, i) => i));
      if (here() && sel === id) drawDetail();
    }
    async function summarize(engine) {
      if (!detail) return;
      progress = "Đang tóm tắt…"; drawDetail();
      try {
        detail.summary = await api("meeting_summarize", { id: detail.id, engine: engine || null });
        checked = new Set(detail.summary.actions.map((_, i) => i));
        flashMood("happy");
        list = null;
      } catch (err) { toast(String(err)); flashMood("worried", 2500); }
      progress = ""; if (here() && sel) drawDetail();
    }
    async function saveTitle() {
      const t = $("#mtTitle");
      if (!t || !detail) return;
      editing = false;
      const v = t.value.trim();
      if (v && v !== detail.title) { try { await api("meeting_rename", { id: detail.id, title: v }); detail.title = v; list = null; } catch (err) { toast(String(err)); } }
      drawDetail();
    }

    function draw() { if (!here()) return; sel ? drawDetail() : drawHome(); }

    // ───────── sự kiện ─────────
    view.addEventListener("click", async (e) => {
      if (A.tab !== "meet") return;
      const o = e.target.closest("[data-open]");
      if (o) return openDetail(o.dataset.open);
      const eng = e.target.closest("[data-eng]");
      if (eng) { await A.saveSettings({ meeting_ai: eng.dataset.eng }); return summarize(eng.dataset.eng); }
      const b = e.target.closest("[data-mt]");
      if (!b || b.disabled) return;
      const a = b.dataset.mt;
      if (a === "settings") return A.openSettings("assistant");
      if (a === "start") {
        const sys = $("#mtSys") ? $("#mtSys").checked : true;
        starting = true; drawHome();
        try { const m = await api("meeting_start", { system: sys }); live.length = 0; if (m.warn) toast(m.warn); flashMood("focus", 2500); confetti(6); }
        catch (err) { toast(String(err)); flashMood("worried", 2500); }
        starting = false; list = null; return draw();
      }
      if (a === "stop") {
        b.disabled = true;
        try { const id = await api("meeting_stop"); list = null; toast("Đã dừng — đang chuyển nốt và tóm tắt"); progress = A.settings.meeting_auto_summary !== false ? "Đang chuyển nốt đoạn cuối…" : ""; return openDetail(id); }
        catch (err) { toast(String(err)); b.disabled = false; }
        return;
      }
      if (a === "back") { sel = null; detail = null; progress = ""; list = null; return drawHome(); }
      if (a === "rename") { editing = true; return drawDetail(); }
      if (a === "sum") return summarize(null);
      if (a === "all") { const n = detail.summary.actions.length; checked = checked.size === n ? new Set() : new Set([...Array(n).keys()]); return drawDetail(); }
      if (a === "tasks") {
        try { const n = await api("meeting_add_tasks", { id: detail.id, items: [...checked] }); toast(`Đã thêm ${n} việc`); confetti(12); A.refreshTasks && A.refreshTasks(); }
        catch (err) { toast(String(err)); }
        return;
      }
      if (a === "note") { try { await api("meeting_to_note", { id: detail.id }); toast("Đã lưu biên bản vào Ghi chú"); } catch (err) { toast(String(err)); } return; }
      if (a === "copy") { try { await api("meeting_copy", { id: detail.id, transcript: !detail.summary }); toast("Đã chép biên bản"); } catch (err) { toast(String(err)); } return; }
      if (a === "audio") { api("meeting_reveal", { id: detail.id }).catch((err) => toast(String(err))); return; }
      if (a === "del") {
        if (b.dataset.sure !== "1") { b.dataset.sure = "1"; b.classList.add("danger-on"); b.title = "Bấm lần nữa để xoá hẳn"; toast("Bấm lần nữa để xoá cuộc họp này"); return; }
        try { await api("meeting_delete", { id: detail.id }); sel = null; detail = null; list = null; drawHome(); } catch (err) { toast(String(err)); }
      }
    });
    view.addEventListener("change", (e) => {
      if (A.tab !== "meet") return;
      if (e.target.id === "mtSys") { sysPref = e.target.checked; A.saveSettings({ meeting_system_audio: sysPref }); return; }
      const i = e.target.dataset.ai;
      if (i != null) { e.target.checked ? checked.add(Number(i)) : checked.delete(Number(i)); const btn = view.querySelector('[data-mt="tasks"]'); if (btn) { btn.disabled = !checked.size; btn.lastChild.textContent = `Thêm ${checked.size || ""} việc vào tab Việc`; } }
    });
    view.addEventListener("keydown", (e) => {
      if (A.tab !== "meet" || e.target.id !== "mtTitle" || A.composing) return;
      if (e.key === "Enter") { e.preventDefault(); saveTitle(); }
      if (e.key === "Escape") { e.stopPropagation(); editing = false; drawDetail(); }
    });
    view.addEventListener("focusout", (e) => { if (A.tab === "meet" && e.target.id === "mtTitle") saveTitle(); });

    document.addEventListener("asi-meet", () => { list = null; if (here() && !sel) drawHome(); });
    document.addEventListener("asi-meet-level", () => { if (here() && !sel) paintLevel(); });
    on("meeting-update", (u) => {
      if (!u) return;
      if (A.meet && u.id === A.meet.id) {
        live.push(...(u.lines || [])); live.splice(0, Math.max(0, live.length - 8));
        A.meet.pending = u.pending;
        if (here() && !sel) drawHome();
      }
      if (detail && detail.id === u.id) {
        detail.lines.push(...(u.lines || []));
        detail.pending = u.pending;
        if (u.error) { detail.failed = (detail.failed || 0) + 1; detail.last_error = u.error; }
        if (here() && sel === u.id && !editing) {
          const box = $("#mtLines");
          if (box && (u.lines || []).length && detail.lines.length > (u.lines || []).length) box.insertAdjacentHTML("beforeend", u.lines.map(lineHtml).join(""));
          else drawDetail();
        }
      }
    });
    on("meeting-progress", (p) => {
      if (!p || !detail || p.id !== detail.id) return;
      progress = p.text || "";
      if (p.error) toast(p.error);
      if (here() && sel === p.id && !editing) drawDetail();
    });
    on("meeting-done", async (id) => {
      list = null;
      if (detail && detail.id === id) { progress = ""; await openDetail(id); flashMood("happy"); }
      else if (here() && !sel) drawHome();
    });

    A.views.meet = () => { draw(); };
  }
  if (window.ASI) boot();
  else document.addEventListener("asi-ready", boot, { once: true });
})();
