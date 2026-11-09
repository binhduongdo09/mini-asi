// Tab Họp: ghi âm cuộc họp (micro + âm thanh máy) → chuyển chữ (Apple, trên máy) → ghi chú trong lúc họp (nếu máy đủ sức)
// → biên bản (Apple Intelligence / Gemini) → sửa được, chuyển ý quan trọng thành Việc / Lịch / Ghi chú → nghe lại ghi âm.
// Đăng ký view qua window.ASI (xem app.js). Phần nặng nằm ở src-tauri/src/meeting.rs và commands/meeting.rs.
(function () {
  "use strict";
  function boot() {
    const A = window.ASI;
    const { api, on, $, esc, I, toast, confetti, flashMood, pad } = A;
    const view = $("#view");
    let sel = null, detail = null, list = null, progress = "", checked = new Set(), editingTitle = false, starting = false;
    let editSum = false, editLine = -1, liveInfo = null; // liveInfo: { live, reason } của cuộc họp đang ghi
    const live = []; // vài dòng mới nhất khi đang ghi
    const liveNotes = []; // ghi chú trong lúc họp của cuộc họp đang ghi
    let sysPref = null, rate = 1;

    const fmtDate = (ms) => { const d = new Date(ms); return `${pad(d.getDate())}/${pad(d.getMonth() + 1)} · ${pad(d.getHours())}:${pad(d.getMinutes())}`; };
    const dur = (s) => { s = Math.round(s || 0); return s >= 3600 ? `${Math.floor(s / 3600)} giờ ${Math.floor(s / 60) % 60} phút` : s >= 60 ? `${Math.floor(s / 60)} phút` : `${s} giây`; };
    const whoChip = (w) => (w === "me" ? `<span class="mt-who me">Bạn</span>` : w === "others" ? `<span class="mt-who">Người khác</span>` : "");
    const brainName = () => (A.settings.meeting_ai === "gemini" ? "Gemini" : "Apple Intelligence");
    const sttName = (s) => (s === "on-device" ? "Chuyển chữ trên máy" : "Chuyển chữ qua máy chủ Apple (cần mạng)");
    const here = () => A.tab === "meet" && A.mode === "expanded";
    // Nút chuyển một ý thành Việc / Lịch / Ghi chú
    const convActs = (text, extra = "") => `<span class="acts mt-conv" data-text="${esc(text)}">${extra}<button class="ib txt" data-cv="task" title="Tạo việc">Việc</button><button class="ib txt" data-cv="event" title="Thêm vào lịch (Apple / Google)">Lịch</button><button class="ib txt" data-cv="note" title="Lưu thành ghi chú">Ghi chú</button></span>`;

    // ───────── trang chính ─────────
    function liveNotesHtml(notes, title = "Ghi chú trong lúc họp") {
      if (!notes.length) return "";
      return `<div class="sec">${title}<span>${notes.length}</span></div>${notes.map((n) => `<div class="mt-livenote"><span class="mt-at">${A.clockOf(n.from)}</span><div class="grow">
          ${n.summary.overview.map((x) => `<div class="mt-li">${esc(x)}${convActs(x)}</div>`).join("")}
          ${n.summary.decisions.map((x) => `<div class="mt-li dec">Quyết định: ${esc(x)}${convActs(x)}</div>`).join("")}
          ${n.summary.actions.map((a) => `<div class="mt-li act">Việc: ${esc(a.task)}${a.owner ? ` · ${esc(a.owner)}` : ""}${a.due ? ` · hạn ${esc(a.due)}` : ""}${convActs([a.task, a.due].filter(Boolean).join(" "))}</div>`).join("")}
        </div></div>`).join("")}`;
    }
    function recCard(m) {
      const lines = live.slice(-4);
      const li = liveInfo && liveInfo.id === m.id ? liveInfo : null;
      return `<div class="mt-rec">
          <div class="mt-rec-top"><i class="rec-dot"></i><b class="grow">${esc(m.title || "Cuộc họp")}</b><span class="mt-time" id="mtTime">${A.clockOf(m.secs)}</span></div>
          <div class="mt-meters"><span>Micro</span><i><b id="mtMic"></b></i>${m.system_audio ? `<span>Máy</span><i><b id="mtSys"></b></i>` : ""}</div>
          <div class="mt-live">${lines.length ? lines.map((l) => `<div>${whoChip(l.who)}${esc(l.text)}</div>`).join("") : `<div class="dim">Chữ sẽ hiện sau mỗi ~30–45 giây…</div>`}</div>
          <div class="mt-rec-foot"><span class="dim grow">${sttName(m.stt)} · ${m.system_audio ? "micro + âm thanh máy" : "chỉ micro"}${m.pending ? ` · đang chuyển ${m.pending} đoạn` : ""}
              <br>${li ? (li.live ? "Ghi chú tự động mỗi ~5 phút (Apple Intelligence trên máy)" : `Ghi chú sau khi họp${li.reason ? ` — ${esc(li.reason)}` : ""}`) : ""}</span>
            <button class="btn danger" data-mt="stop">${I.stop}Dừng ghi</button></div>
        </div>`;
    }
    function startCard() {
      const sys = sysPref ?? A.settings.meeting_system_audio !== false;
      return `<div class="mt-start">
          <button class="mt-big" data-mt="start" ${starting ? "disabled" : ""}>${starting ? `<span class="spin"></span>` : `<i class="rec-dot"></i>`}<span>Ghi cuộc họp</span></button>
          <div class="grow">
            <label class="mt-check"><input type="checkbox" id="mtSys" ${sys ? "checked" : ""}><span><b>Thu cả âm thanh máy</b> — giọng người khác trong Zoom, Meet, Teams (kể cả khi đeo tai nghe)</span></label>
            <div class="dim mt-hint">Họp trực tiếp chỉ cần micro. Máy đủ sức thì Asi ghi chú ngay trong lúc họp, không thì tóm tắt khi dừng ghi (${brainName()}) — <button class="link inline" data-mt="settings">Cài đặt</button>. Nhớ báo mọi người là đang ghi âm.</div>
          </div>
        </div>`;
    }
    async function drawHome() {
      if (!list) { view.innerHTML = `<div class="mt">${A.meet ? recCard(A.meet) : startCard()}<div class="empty sm"><span class="spin"></span></div></div>`; list = await api("meeting_list").catch(() => []); if (!here() || sel) return; }
      const m = A.meet;
      const rows = list.filter((x) => !m || x.id !== m.id);
      view.innerHTML = `<div class="mt">${m ? recCard(m) : startCard()}
        ${m ? liveNotesHtml(liveNotes) : ""}
        <div class="sec">Các cuộc họp<span>${rows.length || ""}</span></div>
        ${rows.length ? rows.map((x) => `<button class="row mt-row" data-open="${esc(x.id)}">
            <span class="mt-ic">${I.wave}</span>
            <span class="grow"><b>${esc(x.title)}</b><span class="dim">${fmtDate(x.started)} · ${dur(x.duration)}${x.summarized ? ` · ${x.actions} việc` : x.lines ? " · chưa tóm tắt" : " · chưa có chữ"}${x.has_audio ? " · có ghi âm" : ""}</span></span>
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
    function playerHtml(d) {
      if (!d.has_audio || d.recording) return "";
      return `<div class="mt-player">
          <button class="mt-play" data-mt="play" title="Phát / tạm dừng (Space)">${I.play}</button>
          <button class="ib" data-mt="back15" title="Lùi 15 giây">−15</button>
          <span class="mt-cur" id="mtCur">00:00</span>
          <input type="range" id="mtSeek" min="0" max="${Math.max(1, d.duration)}" step="0.5" value="0">
          <span class="mt-dur" id="mtDur">${A.clockOf(d.duration)}</span>
          <button class="ib" data-mt="fwd15" title="Tới 15 giây">+15</button>
          <div class="seg">${[1, 1.5, 2].map((r) => `<button data-rate="${r}" class="${rate === r ? "on" : ""}">${r}×</button>`).join("")}</div>
        </div>`;
    }
    function sumView(s) {
      const sec = (t, xs, cls) => (xs.length ? `<div class="sec">${t}</div><div class="mt-items">${xs.map((x) => `<div class="mt-item ${cls}"><span class="grow">${esc(x)}</span>${convActs(x)}</div>`).join("")}</div>` : "");
      return `${s.note ? `<div class="banner warn">${esc(s.note)}</div>` : ""}
        ${sec("Tóm tắt", s.overview, "")}${sec("Quyết định", s.decisions, "dec")}
        ${s.actions.length ? `<div class="sec">Việc cần làm<span>${s.actions.length}</span><button class="link" data-mt="all">${checked.size === s.actions.length ? "Bỏ chọn" : "Chọn hết"}</button></div>
          <div class="mt-items">${s.actions.map((a, i) => `<div class="mt-item act"><label class="mt-actl"><input type="checkbox" data-ai="${i}" ${checked.has(i) ? "checked" : ""}><span class="grow">${esc(a.task)}${a.owner || a.due ? `<span class="dim">${[a.owner, a.due && `hạn ${a.due}`].filter(Boolean).map(esc).join(" · ")}</span>` : ""}</span></label>
            ${convActs([a.task, a.owner && `(${a.owner})`, a.due].filter(Boolean).join(" "))}</div>`).join("")}</div>
          <div class="mt-act-bar"><button class="btn sm primary" data-mt="tasks" ${checked.size ? "" : "disabled"}>${I.plus}<span>Thêm ${checked.size || ""} việc vào tab Việc</span></button></div>` : ""}
        <div class="engine">${esc(s.engine)}</div>`;
    }
    function sumEdit(s) {
      const ta = (id, label, v, hint) => `<label class="mt-ed"><b>${label}</b><span class="dim">${hint}</span><textarea class="field" id="${id}" rows="${Math.min(8, Math.max(2, v.split("\n").length + 1))}">${esc(v)}</textarea></label>`;
      return `<div class="mt-edit">
        ${ta("edOv", "Tóm tắt", s.overview.join("\n"), "mỗi dòng một ý")}
        ${ta("edDec", "Quyết định", s.decisions.join("\n"), "mỗi dòng một quyết định")}
        ${ta("edAct", "Việc cần làm", s.actions.map((a) => [a.task, a.owner, a.due].join(" | ").replace(/( \| )+$/, "")).join("\n"), "mỗi dòng: việc | người phụ trách | hạn")}
        <div class="mt-act-bar"><button class="btn sm ghost" data-mt="ed-cancel">Huỷ</button><button class="btn sm primary" data-mt="ed-save">${I.check}Lưu biên bản</button></div>
      </div>`;
    }
    function sumHtml(d) {
      const s = d.summary;
      if (editSum) return sumEdit(s || { overview: [], decisions: [], actions: [] });
      if (!s) {
        const ln = liveNotesHtml(d.live || []);
        return (d.lines.length && !d.recording
          ? `<div class="mt-empty-sum"><span class="dim">Chưa có biên bản.</span><button class="btn sm ghost" data-mt="ed-start">Tự viết</button><button class="btn sm primary" data-mt="sum">${I.sparkle}Tóm tắt bằng ${brainName()}</button></div>`
          : "") + ln;
      }
      return sumView(s);
    }
    const lineHtml = (l, i) => editLine === i
      ? `<div class="mt-line editing" data-li="${i}"><span class="mt-at">${A.clockOf(l.at)}</span><input class="field grow" id="mtLineEd" value="${esc(l.text)}"><button class="ib" data-mt="line-save" title="Lưu (Enter)">${I.check}</button></div>`
      : `<div class="mt-line" data-li="${i}" data-at="${l.at}"><span class="mt-at">${A.clockOf(l.at)}</span>${whoChip(l.who)}<span class="grow">${esc(l.text)}</span>${convActs(l.text, `<button class="ib" data-mt="line-edit" title="Sửa dòng này">${I.note}</button>`)}</div>`;
    function drawDetail() {
      const d = detail;
      if (!d) { view.innerHTML = `<div class="empty"><span class="spin"></span></div>`; return; }
      const s = d.summary;
      const audio = $("#mtAudio");
      const keepT = audio && audio.dataset.id === d.id ? audio.currentTime : 0, wasPlaying = audio && !audio.paused;
      view.innerHTML = `<div class="mt">
        <div class="mt-dhead">
          <button class="ib" data-mt="back" title="Về danh sách">${I.chevron}</button>
          ${editingTitle ? `<input class="field grow" id="mtTitle" value="${esc(d.title)}">` : `<b class="grow mt-title" data-mt="rename" title="Bấm để đổi tên">${esc(d.title)}</b>`}
          ${d.recording ? `<span class="badge rec"><i class="rec-dot"></i>Đang ghi</span>` : ""}
        </div>
        <div class="dim mt-meta">${fmtDate(d.started)} · ${dur(d.duration)} · ${d.system_audio ? "micro + âm thanh máy" : "micro"} · ${d.stt === "on-device" ? "chuyển chữ trên máy" : "chuyển chữ qua Apple"}${d.pending ? ` · còn ${d.pending} đoạn đang chuyển` : ""}${d.failed ? ` · <span class="err">${d.failed} đoạn lỗi${d.last_error ? `: ${esc(d.last_error)}` : ""}</span>` : ""}</div>
        ${playerHtml(d)}
        <div class="mt-tools">
          ${s && !editSum ? `<button class="btn sm" data-mt="sum" ${progress || d.recording ? "disabled" : ""}>${I.sparkle}Tóm tắt lại</button>
            <div class="seg"><button data-eng="apple" class="${A.settings.meeting_ai !== "gemini" ? "on" : ""}">Apple</button><button data-eng="gemini" class="${A.settings.meeting_ai === "gemini" ? "on" : ""}">Gemini</button></div>` : ""}
          <span class="grow"></span>
          ${s && !editSum ? `<button class="btn sm ghost" data-mt="ed-start" title="Sửa biên bản">${I.sliders}Sửa</button>` : ""}
          <button class="btn sm ghost" data-mt="note" title="Mở ghi chú biên bản (Ghi chú › Biên bản họp)" ${d.lines.length ? "" : "disabled"}>${I.note}Ghi chú</button>
          <button class="btn sm ghost" data-mt="copy" title="Chép biên bản" ${d.lines.length ? "" : "disabled"}>${I.copy}Chép</button>
          ${d.has_audio && !d.recording ? `<button class="btn sm ghost" data-mt="audio" title="Mở file ghi âm trong Finder">${I.folder}Finder</button>` : ""}
          ${d.recording ? "" : `<button class="btn sm ghost mt-del" data-mt="del" title="Xoá cuộc họp">${I.trash}Xoá</button>`}
        </div>
        ${progress ? `<div class="mt-progress"><span class="spin"></span>${esc(progress)}</div>` : ""}
        ${sumHtml(d)}
        <div class="sec">Toàn văn<span>${d.lines.length || ""}</span>${d.lines.length ? `<span class="hint-txt">bấm một dòng để nghe đoạn đó · rê chuột để sửa / tạo việc</span>` : ""}</div>
        <div class="mt-lines" id="mtLines">${d.lines.length ? d.lines.map(lineHtml).join("") : `<div class="empty sm">${d.recording || d.pending ? "Chữ đang được chuyển…" : "Không có lời nói nào được nhận dạng."}</div>`}</div>
      </div>`;
      bindAudio(d, keepT, wasPlaying);
      if (editingTitle) { const t = $("#mtTitle"); t.focus(); t.select(); }
      if (editLine >= 0) { const t = $("#mtLineEd"); if (t) { t.focus(); t.setSelectionRange(t.value.length, t.value.length); } }
    }

    // ───────── nghe lại ─────────
    // <audio> nằm ngoài #view để không bị tạo lại mỗi lần vẽ (đang phát vẫn phát tiếp)
    function audioEl() {
      let a = $("#mtAudio");
      if (!a) { a = document.createElement("audio"); a.id = "mtAudio"; a.preload = "metadata"; a.hidden = true; document.body.appendChild(a); hookAudio(a); }
      return a;
    }
    function srcOf(path) {
      const T = window.__TAURI__;
      return T && T.core && T.core.convertFileSrc ? T.core.convertFileSrc(path) : path;
    }
    function bindAudio(d, keepT, wasPlaying) {
      if (!d.has_audio || d.recording || !d.audio_path) return;
      const a = audioEl();
      if (a.dataset.id !== d.id || a.dataset.path !== d.audio_path) {
        a.pause(); a.dataset.id = d.id; a.dataset.path = d.audio_path; a.src = srcOf(d.audio_path); a.playbackRate = rate;
      } else if (keepT) { a.currentTime = keepT; if (wasPlaying) a.play().catch(() => {}); }
      paintPlayer();
    }
    let lastHi = -1, userScroll = 0;
    function paintPlayer() {
      const a = $("#mtAudio");
      if (!a || !detail || a.dataset.id !== detail.id) return;
      const t = a.currentTime || 0;
      const cur = $("#mtCur"), seek = $("#mtSeek"), du = $("#mtDur"), btn = view.querySelector('[data-mt="play"]');
      if (cur) cur.textContent = A.clockOf(t);
      if (seek && document.activeElement !== seek) { if (isFinite(a.duration)) seek.max = a.duration; seek.value = t; }
      if (du && isFinite(a.duration)) du.textContent = A.clockOf(a.duration);
      if (btn) { btn.innerHTML = a.paused ? I.play : I.pause; btn.classList.toggle("on", !a.paused); }
      // tô sáng dòng đang nói
      let hi = -1;
      detail.lines.forEach((l, i) => { if (l.at <= t + 0.3) hi = i; });
      if (hi !== lastHi || !view.querySelector(".mt-line.now")) {
        view.querySelectorAll(".mt-line.now").forEach((x) => x.classList.remove("now"));
        const row = hi >= 0 ? view.querySelector(`.mt-line[data-li="${hi}"]`) : null;
        if (row) { row.classList.add("now"); if (!a.paused && Date.now() - userScroll > 4000) row.scrollIntoView({ block: "nearest", behavior: "smooth" }); }
        lastHi = hi;
      }
    }
    function hookAudio(a) {
      ["timeupdate", "play", "pause", "loadedmetadata", "ended"].forEach((ev) => a.addEventListener(ev, () => { if (here() && sel) paintPlayer(); }));
      a.addEventListener("error", () => { if (here() && sel) toast("Không mở được file ghi âm"); });
    }
    view.addEventListener("wheel", () => { userScroll = Date.now(); }, { passive: true });
    function playAt(t) {
      const a = $("#mtAudio");
      if (!a || !detail || a.dataset.id !== detail.id) return;
      a.currentTime = Math.max(0, t);
      a.play().catch(() => toast("Không phát được ghi âm"));
    }

    async function openDetail(id) {
      if (sel !== id) { const a = $("#mtAudio"); if (a) a.pause(); }
      sel = id; detail = null; checked = new Set(); editingTitle = false; editSum = false; editLine = -1;
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
        toast("Đã lưu biên bản vào Ghi chú › Biên bản họp");
      } catch (err) { toast(String(err)); flashMood("worried", 2500); }
      progress = ""; if (here() && sel) drawDetail();
    }
    async function saveTitle() {
      const t = $("#mtTitle");
      if (!t || !detail) return;
      editingTitle = false;
      const v = t.value.trim();
      if (v && v !== detail.title) { try { await api("meeting_rename", { id: detail.id, title: v }); detail.title = v; list = null; } catch (err) { toast(String(err)); } }
      drawDetail();
    }
    async function saveSummary() {
      const lines = (id) => ($("#" + id).value || "").split("\n").map((x) => x.replace(/^\s*[-•*]\s*/, "").trim()).filter(Boolean);
      const summary = {
        overview: lines("edOv"), decisions: lines("edDec"),
        actions: lines("edAct").map((x) => { const [task, owner = "", due = ""] = x.split("|").map((y) => y.trim()); return { task, owner, due }; }).filter((a) => a.task),
        engine: "", at: 0, note: "",
      };
      try { detail.summary = await api("meeting_update_summary", { id: detail.id, summary }); checked = new Set(detail.summary.actions.map((_, i) => i)); editSum = false; list = null; toast("Đã lưu biên bản"); }
      catch (err) { toast(String(err)); }
      drawDetail();
    }
    async function saveLine() {
      const t = $("#mtLineEd");
      if (!t || editLine < 0 || !detail) return;
      const i = editLine, v = t.value.trim();
      editLine = -1;
      try {
        await api("meeting_edit_line", { id: detail.id, index: i, text: v });
        if (v) detail.lines[i].text = v; else detail.lines.splice(i, 1);
      } catch (err) { toast(String(err)); }
      drawDetail();
    }
    async function convert(kind, text) {
      text = (text || "").trim();
      if (!text) return;
      if (kind === "note") {
        const id = detail ? detail.id : A.meet && A.meet.id;
        try { await api("meeting_item_note", { id, text }); toast("Đã lưu vào Ghi chú › Biên bản họp"); confetti(6); } catch (err) { toast(String(err)); }
        return;
      }
      A.captureText(text, { kind, back: "meet" }); // bảng tạo việc / lịch: sửa tiêu đề, ngày giờ, chọn lịch rồi mới tạo
    }

    function draw() { if (!here()) return; sel ? drawDetail() : drawHome(); }

    // ───────── sự kiện ─────────
    view.addEventListener("click", async (e) => {
      if (A.tab !== "meet") return;
      const cv = e.target.closest("[data-cv]");
      if (cv) { e.stopPropagation(); return convert(cv.dataset.cv, cv.closest("[data-text]").dataset.text); }
      const o = e.target.closest("[data-open]");
      if (o) return openDetail(o.dataset.open);
      const eng = e.target.closest("[data-eng]");
      if (eng) { await A.saveSettings({ meeting_ai: eng.dataset.eng }); return summarize(eng.dataset.eng); }
      const rt = e.target.closest("[data-rate]");
      if (rt) { rate = Number(rt.dataset.rate); const a = $("#mtAudio"); if (a) a.playbackRate = rate; view.querySelectorAll("[data-rate]").forEach((x) => x.classList.toggle("on", Number(x.dataset.rate) === rate)); return; }
      const b = e.target.closest("[data-mt]");
      if (!b) {
        // bấm vào một dòng toàn văn → nghe đoạn đó
        const ln = e.target.closest(".mt-line[data-at]");
        if (ln && detail && detail.has_audio && !detail.recording) playAt(Number(ln.dataset.at));
        return;
      }
      if (b.disabled) return;
      const a = b.dataset.mt;
      if (a === "settings") return A.openSettings("assistant");
      if (a === "start") {
        const sys = $("#mtSys") ? $("#mtSys").checked : true;
        starting = true; drawHome();
        try { const m = await api("meeting_start", { system: sys }); live.length = 0; liveNotes.length = 0; if (m.warn) toast(m.warn); flashMood("focus", 2500); confetti(6); }
        catch (err) { toast(String(err)); flashMood("worried", 2500); }
        starting = false; list = null; return draw();
      }
      if (a === "stop") {
        b.disabled = true;
        try { const id = await api("meeting_stop"); list = null; toast("Đã dừng — đang chuyển nốt và tóm tắt"); progress = A.settings.meeting_auto_summary !== false ? "Đang chuyển nốt đoạn cuối…" : ""; return openDetail(id); }
        catch (err) { toast(String(err)); b.disabled = false; }
        return;
      }
      if (a === "back") { const au = $("#mtAudio"); if (au) au.pause(); sel = null; detail = null; progress = ""; list = null; return drawHome(); }
      if (a === "play") { const au = $("#mtAudio"); if (au) au.paused ? au.play().catch(() => toast("Không phát được ghi âm")) : au.pause(); return; }
      if (a === "back15" || a === "fwd15") { const au = $("#mtAudio"); if (au) au.currentTime = Math.max(0, au.currentTime + (a === "fwd15" ? 15 : -15)); return; }
      if (a === "rename") { editingTitle = true; return drawDetail(); }
      if (a === "sum") return summarize(null);
      if (a === "ed-start") { editSum = true; return drawDetail(); }
      if (a === "ed-cancel") { editSum = false; return drawDetail(); }
      if (a === "ed-save") return saveSummary();
      if (a === "line-edit") { editLine = Number(b.closest("[data-li]").dataset.li); return drawDetail(); }
      if (a === "line-save") return saveLine();
      if (a === "all") { const n = detail.summary.actions.length; checked = checked.size === n ? new Set() : new Set([...Array(n).keys()]); return drawDetail(); }
      if (a === "tasks") {
        try { const n = await api("meeting_add_tasks", { id: detail.id, items: [...checked] }); toast(`Đã thêm ${n} việc`); confetti(12); A.refreshTasks && A.refreshTasks(); }
        catch (err) { toast(String(err)); }
        return;
      }
      if (a === "note") {
        try { const nid = await api("meeting_to_note", { id: detail.id }); if (A.openNote) A.openNote(nid); else toast("Đã lưu vào Ghi chú › Biên bản họp"); } catch (err) { toast(String(err)); }
        return;
      }
      if (a === "copy") { try { await api("meeting_copy", { id: detail.id, transcript: !detail.summary }); toast("Đã chép biên bản"); } catch (err) { toast(String(err)); } return; }
      if (a === "audio") { api("meeting_reveal", { id: detail.id }).catch((err) => toast(String(err))); return; }
      if (a === "del") {
        if (b.dataset.sure !== "1") { b.dataset.sure = "1"; b.classList.add("danger-on"); b.title = "Bấm lần nữa để xoá hẳn"; toast("Bấm lần nữa để xoá cuộc họp này (cả ghi âm)"); return; }
        try { const au = $("#mtAudio"); if (au) { au.pause(); au.removeAttribute("src"); au.dataset.id = ""; } await api("meeting_delete", { id: detail.id }); sel = null; detail = null; list = null; drawHome(); } catch (err) { toast(String(err)); }
      }
    });
    view.addEventListener("input", (e) => {
      if (A.tab !== "meet" || e.target.id !== "mtSeek") return;
      const a = $("#mtAudio"); if (a) a.currentTime = Number(e.target.value);
    });
    view.addEventListener("change", (e) => {
      if (A.tab !== "meet") return;
      if (e.target.id === "mtSys") { sysPref = e.target.checked; A.saveSettings({ meeting_system_audio: sysPref }); return; }
      const i = e.target.dataset.ai;
      if (i != null) { e.target.checked ? checked.add(Number(i)) : checked.delete(Number(i)); const btn = view.querySelector('[data-mt="tasks"]'); if (btn) { btn.disabled = !checked.size; btn.querySelector("span").textContent = `Thêm ${checked.size || ""} việc vào tab Việc`; } }
    });
    view.addEventListener("keydown", (e) => {
      if (A.tab !== "meet" || A.composing) return;
      if (e.target.id === "mtTitle") {
        if (e.key === "Enter") { e.preventDefault(); saveTitle(); }
        if (e.key === "Escape") { e.stopPropagation(); editingTitle = false; drawDetail(); }
      }
      if (e.target.id === "mtLineEd") {
        if (e.key === "Enter") { e.preventDefault(); saveLine(); }
        if (e.key === "Escape") { e.stopPropagation(); editLine = -1; drawDetail(); }
      }
      if (e.target.closest(".mt-edit") && e.key === "Escape") { e.stopPropagation(); editSum = false; drawDetail(); }
      if (e.key === "s" && (e.metaKey || e.ctrlKey) && editSum) { e.preventDefault(); saveSummary(); }
    });
    // Space = phát / dừng khi không gõ chữ
    window.addEventListener("keydown", (e) => {
      if (!here() || !sel || e.key !== " " || A.typing()) return;
      const au = $("#mtAudio");
      if (au && detail && au.dataset.id === detail.id) { e.preventDefault(); au.paused ? au.play().catch(() => {}) : au.pause(); }
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
        const base = detail.lines.length;
        detail.lines.push(...(u.lines || []));
        detail.pending = u.pending;
        if (u.error) { detail.failed = (detail.failed || 0) + 1; detail.last_error = u.error; }
        if (here() && sel === u.id && !editingTitle && editLine < 0 && !editSum) {
          const box = $("#mtLines");
          if (box && base && (u.lines || []).length) box.insertAdjacentHTML("beforeend", u.lines.map((l, k) => lineHtml(l, base + k)).join(""));
          else drawDetail();
        }
      }
    });
    on("meeting-live-mode", (x) => { if (!x) return; liveInfo = x; if (x.reason && A.meet && A.meet.id === x.id) toast(x.reason); if (here() && !sel) drawHome(); });
    on("meeting-live", (x) => {
      if (!x || !x.note) return;
      if (A.meet && A.meet.id === x.id) { liveNotes.push(x.note); flashMood("happy", 1500); if (here() && !sel) drawHome(); }
      if (detail && detail.id === x.id) { (detail.live = detail.live || []).push(x.note); if (here() && sel === x.id && !detail.summary && !editSum) drawDetail(); }
    });
    on("meeting-progress", (p) => {
      if (!p || !detail || p.id !== detail.id) return;
      progress = p.text || "";
      if (p.error) toast(p.error);
      if (here() && sel === p.id && !editingTitle && !editSum) drawDetail();
    });
    on("meeting-audio", async (id) => {
      // file ghi âm vừa được nén sang m4a → nạp lại đường dẫn
      if (detail && detail.id === id) { try { const d = await api("meeting_get", { id }); detail.has_audio = d.has_audio; detail.audio_path = d.audio_path; if (here() && sel === id && !editSum && editLine < 0) drawDetail(); } catch (_) {} }
    });
    on("meeting-done", async (id) => {
      list = null;
      if (detail && detail.id === id) { progress = ""; await openDetail(id); flashMood("happy"); }
      else if (here() && !sel) drawHome();
    });

    A.views.meet = () => { draw(); };
    /** Mở chi tiết một cuộc họp (vd. từ ghi chú biên bản) */
    A.openMeeting = async (id) => { A.tab = "meet"; await A.setMode("expanded", { force: true }); openDetail(id); };
  }
  if (window.ASI) boot();
  else document.addEventListener("asi-ready", boot, { once: true });
})();
