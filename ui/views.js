/* ───────── Mini-Asi — nội dung các tab chính: Asi · Việc · Ghi chú · Dọn máy ─────────
 * Mỗi tab là một hàm vẽ vào #view, đăng ký qua `A.views.<id>`. Tiện ích chung lấy từ window.ASI (app.js).
 * Quy tắc: một màn hình một việc chính · chữ ngắn · không vẽ lại ô đang gõ · nút phụ chỉ hiện khi rê chuột.
 */
(function () {
  "use strict";
  function boot() {
    const A = window.ASI;
    const { api, on, $, $$, esc, I, toast, confetti, flashMood, fmtDue, fmtFull, fmtAgo, hm, bytes, shortPath, isComposing, typing, micBtn, dictate } = A;
    const view = $("#view");
    const here = (t) => A.mode === "expanded" && A.tab === t;

    // ═════════ ASI ═════════
    function greet() {
      const h = new Date().getHours();
      return h < 5 ? "Khuya rồi" : h < 11 ? "Chào buổi sáng" : h < 14 ? "Chào buổi trưa" : h < 18 ? "Chào buổi chiều" : "Chào buổi tối";
    }
    function agendaHtml() {
      const s = A.status, now = Date.now(), rows = [];
      if (!s) return "";
      const late = A.tasks.filter((t) => !t.done_at && t.due && t.due < now);
      if (late.length) rows.push(["tasks", "red", `${late.length} việc trễ hạn`, esc(late[0].title)]);
      if (s.next_event) rows.push(["tasks", "blue", s.next_event.start <= now ? "Đang diễn ra" : fmtDue(s.next_event.start), esc(s.next_event.title)]);
      if (s.next) rows.push(["tasks", "", fmtDue(s.next.due), esc(s.next.title)]);
      if (s.focus_until) rows.unshift(["home", "violet", "Đang tập trung", `còn ${Math.max(1, Math.round((s.focus_until * 1000 - now) / 60000))} phút`]);
      if (!rows.length) return `<div class="agenda empty-line">Không có việc hay lịch nào sắp tới.</div>`;
      return `<div class="agenda">${rows.slice(0, 3).map(([go, c, k, v]) => `<button class="ag-row" data-go="${go}"><span class="ag-k ${c}">${k}</span><span class="ag-v">${v}</span>${I.chevron}</button>`).join("")}</div>`;
    }
    function suggestions() {
      const s = A.status;
      const late = s && s.lateness.overdue;
      if (A.aiReady) {
        // Có Apple Intelligence: gợi ý cho thấy Asi làm được việc của một trợ lý thật
        return [late ? "Việc nào trễ, nên làm gì trước?" : "Hôm nay mình nên ưu tiên gì?", "Viết tin nhắn xin dời lịch họp sang chiều", "Tóm tắt mục mình vừa chép", "Máy mình có ổn không?"];
      }
      const list = [late ? "Việc nào đang trễ?" : "Hôm nay có gì?", "Máy mình thế nào?", "Mục vừa chép là gì?"];
      list.push(A.modOn("clean") && s && s.disk && s.disk.level !== "ok" ? "Dọn rác giúp mình" : "Nhắc tôi 3 giờ chiều gọi khách");
      return list;
    }
    function viewHome() {
      view.innerHTML = `<div class="asi-home">
        ${A.updateBanner()}
        <div class="asi-body" id="chatLog"></div>
        <div class="composer">
          ${micBtn("chatMic", "Nói với Asi")}
          <input class="field" id="chatInput" placeholder="${A.aiReady ? "Hỏi Asi bất cứ điều gì…" : "Hỏi Asi, hoặc gõ “nhắc tôi mai 9h họp”…"}" />
          <button class="send" id="chatSend" title="Gửi">${I.send}</button>
        </div></div>`;
      const inp = $("#chatInput");
      const send = () => { const v = inp.value; inp.value = ""; A.askAsi(v, false); };
      inp.addEventListener("keydown", (e) => { if (e.key === "Enter" && !isComposing(e)) send(); });
      $("#chatSend").addEventListener("click", send);
      $("#chatMic").addEventListener("click", (e) => dictate(e.currentTarget, (text) => A.askAsi(text, false)));
      drawChat();
      setTimeout(() => inp.focus(), 80);
    }
    function drawChat() {
      const log = $("#chatLog");
      if (!log) return;
      const chat = A.chat;
      if (!chat.length) {
        log.innerHTML = `<div class="hello">
            <div class="big-face" id="bigFace"><span class="eye"></span><span class="eye"></span></div>
            <div><h1>${greet()}</h1><p>${fmtFull(Date.now()).split(" · ")[0]}</p></div></div>
          ${agendaHtml()}
          <div class="sugg">${suggestions().map((t) => `<button class="chip-btn" data-say="${esc(t)}">${esc(t)}</button>`).join("")}</div>`;
        return;
      }
      log.innerHTML = `<div class="chat">${chat.map((m) => m.who === "me"
        ? `<div class="bubble me">${esc(m.text)}</div>`
        : `<div class="bubble asi">${m.thinking ? '<span class="dots"><i></i><i></i><i></i></span>' : A.fmtMd(m.text)}${m.reply ? A.replyHtml(m.reply) : ""}</div>`).join("")}</div>
        <button class="link center" data-chat="clear">Cuộc trò chuyện mới</button>`;
      log.scrollTop = log.scrollHeight;
    }
    document.addEventListener("asi-chat", () => { if (here("home")) drawChat(); });
    document.addEventListener("asi-status", () => {
      if (!here("home") || A.chat.length) return;
      drawChat(); // chỉ vẽ lại phần trên, không đụng ô đang gõ
      const inp = $("#chatInput");
      if (inp && A.aiReady) inp.placeholder = "Hỏi Asi bất cứ điều gì…";
    });
    view.addEventListener("click", (e) => {
      if (A.tab !== "home") return;
      const say = e.target.closest("[data-say]");
      if (say) return A.askAsi(say.dataset.say, false);
      const go = e.target.closest("[data-go]");
      if (go) return A.switchTab(go.dataset.go);
      if (e.target.closest('[data-chat="clear"]')) { A.chat.length = 0; api("asi_reset").catch(() => {}); drawChat(); $("#chatInput")?.focus(); }
    });
    A.bindReplyClicks(view);

    // ═════════ VIỆC ═════════
    let showDone = false;
    function viewTasks() {
      view.innerHTML = `<div class="adder">
          <span class="ic">${I.plus}</span>
          <input class="field" id="taskInput" placeholder="Thêm việc — vd: mai 9h gọi khách" />
          ${micBtn("taskMic", "Nói để thêm việc")}
        </div>
        <div class="hint" id="taskHint"></div>
        <div id="eventList"></div>
        <div id="taskList"></div>`;
      const inp = $("#taskInput");
      $("#taskMic").addEventListener("click", (e) => dictate(e.currentTarget, (text) => { inp.value = text; inp.dispatchEvent(new Event("input")); inp.focus(); }));
      let deb;
      inp.addEventListener("input", () => {
        clearTimeout(deb);
        deb = setTimeout(async () => {
          const v = inp.value.trim();
          const h = $("#taskHint");
          if (!v) { h.innerHTML = ""; return; }
          const p = await api("parse_task", { text: v });
          h.innerHTML = p.due ? `${I.clock}<b>${fmtFull(p.due)}</b><span>· Enter để thêm</span>` : `<span>Không hẹn giờ · Enter để thêm</span>`;
        }, 120);
      });
      inp.addEventListener("keydown", async (e) => {
        if (e.key !== "Enter" || isComposing(e) || !inp.value.trim()) return;
        try {
          const t = await api("add_task", { text: inp.value });
          inp.value = "";
          $("#taskHint").innerHTML = `<span class="ok">${I.check}Đã thêm${t.due ? " · " + fmtDue(t.due) : ""}</span>`;
          flashMood("happy", 1200);
          await A.refreshTasks();
        } catch (err) { toast(String(err)); }
      });
      drawEvents();
      drawTasks();
      setTimeout(() => inp.focus(), 80);
    }
    async function drawEvents() {
      const box = $("#eventList");
      if (!box || !A.settings.show_calendar) return;
      let evs = [];
      try { evs = await api("get_events"); } catch (_) { return; }
      const now = Date.now(), end = new Date(); end.setHours(23, 59, 59, 999);
      const today = evs.filter((e) => e.end > now && e.start <= end.getTime());
      box.innerHTML = today.length ? `<div class="sec">Lịch hôm nay</div>` + today.map((e) => `<div class="row ev-row">
        <span class="ev"></span><span class="title">${esc(e.title)}${e.location ? `<span class="dim"> · ${esc(e.location)}</span>` : ""}</span>
        <span class="due ${e.start <= now ? "live" : ""}">${e.all_day ? "Cả ngày" : e.start <= now ? "Đang diễn ra" : hm(new Date(e.start))}</span></div>`).join("") : "";
    }
    function taskRow(t, now) {
      const late = !t.done_at && t.due && t.due < now;
      return `<div class="row task ${t.done_at ? "done" : ""}" data-id="${t.id}">
        <button class="check" data-t="toggle" title="${t.done_at ? "Bỏ đánh dấu" : "Xong"}">${I.check}</button>
        <span class="title" data-t="title" title="Bấm đúp để sửa">${esc(t.title)}</span>
        ${t.notes ? `<span class="t-note" title="${esc(t.notes)}">${I.note}</span>` : ""}
        <span class="acts">${t.done_at ? "" : `<button class="ib txt" data-t="s60" title="Hoãn 1 giờ">+1h</button><button class="ib txt" data-t="stm" title="Chuyển sang mai">Mai</button>`}<button class="ib" data-t="del" title="Xoá">${I.trash}</button></span>
        ${t.due && !t.done_at ? `<span class="due ${late ? "late" : ""}">${fmtDue(t.due, now)}</span>` : ""}
      </div>`;
    }
    function drawTasks() {
      const box = $("#taskList");
      if (!box) return;
      const now = Date.now(), tasks = A.tasks;
      const open = tasks.filter((t) => !t.done_at);
      const done = tasks.filter((t) => t.done_at).sort((a, b) => b.done_at - a.done_at);
      const by = (a, b) => (a.due ?? 9e15) - (b.due ?? 9e15);
      const endToday = new Date(); endToday.setHours(23, 59, 59, 999);
      const groups = [
        ["Trễ hạn", open.filter((t) => t.due && t.due < now).sort(by), "red"],
        ["Hôm nay", open.filter((t) => t.due && t.due >= now && t.due <= endToday).sort(by), ""],
        ["Sắp tới", open.filter((t) => t.due && t.due > endToday).sort(by), ""],
        ["Chưa hẹn giờ", open.filter((t) => !t.due).sort((a, b) => b.created - a.created), ""],
      ];
      let html = "";
      for (const [name, list, cls] of groups) {
        if (list.length) html += `<div class="sec ${cls}">${name}<span>${list.length}</span></div>` + list.map((t) => taskRow(t, now)).join("");
      }
      if (!open.length) html += `<div class="empty"><div class="big-face sm"><span class="eye"></span><span class="eye"></span></div><b>Hết việc rồi!</b><span>Gõ vào ô phía trên để thêm việc mới.</span></div>`;
      if (done.length) {
        html += `<button class="sec toggle" data-t="showdone">Đã xong<span>${done.length}</span>${showDone ? '<span class="link" data-t="cleardone">Xoá hết</span>' : ""}</button>`;
        if (showDone) html += done.slice(0, 30).map((t) => taskRow(t, now)).join("");
      }
      box.innerHTML = html;
    }
    document.addEventListener("asi-tasks", () => { if (here("tasks") && !(typing() && document.activeElement?.closest?.("#taskList"))) drawTasks(); });
    setInterval(() => { if (here("tasks") && !typing()) drawTasks(); }, 30000);
    view.addEventListener("click", async (e) => {
      if (A.tab !== "tasks") return;
      const b = e.target.closest("[data-t]");
      if (!b) return;
      const act = b.dataset.t;
      if (act === "cleardone") { e.stopPropagation(); await api("clear_done"); return A.refreshTasks(); }
      if (act === "showdone") { showDone = !showDone; return drawTasks(); }
      const row = b.closest(".task");
      if (!row) return;
      const id = Number(row.dataset.id);
      if (act === "title") return;
      if (act === "toggle") {
        const t = await api("toggle_task", { id });
        if (t && t.done_at) { row.classList.add("done"); confetti(16); flashMood("happy"); await new Promise((r) => setTimeout(r, 420)); }
      }
      if (act === "s60") await api("snooze_task", { id, minutes: 60 });
      if (act === "stm") await api("snooze_task", { id, minutes: -1 });
      if (act === "del") await api("delete_task", { id });
      await A.refreshTasks();
    });
    view.addEventListener("dblclick", (e) => {
      const el = e.target.closest('.task [data-t="title"]');
      if (!el || A.tab !== "tasks") return;
      el.contentEditable = "true";
      el.focus();
      document.getSelection().selectAllChildren(el);
      const finish = async (save) => {
        el.contentEditable = "false";
        el.onblur = null;
        if (save && el.textContent.trim()) await api("rename_task", { id: Number(el.closest(".task").dataset.id), title: el.textContent.trim() });
        await A.refreshTasks();
      };
      el.onkeydown = (k) => { if (isComposing(k)) return; if (k.key === "Enter") { k.preventDefault(); finish(true); } if (k.key === "Escape") { k.stopPropagation(); finish(false); } };
      el.onblur = () => finish(true);
    });

    // ═════════ GHI CHÚ ═════════
    let notes = [], noteId = null, noteQuery = "";
    async function viewNotes() {
      notes = await api("list_notes");
      if (noteId == null && notes.length) noteId = notes[0].id;
      const cur = notes.find((n) => n.id === noteId);
      view.innerHTML = `<div class="notes">
        <aside class="n-side">
          <div class="n-search">${I.search}<input class="field" id="noteSearch" placeholder="Tìm" value="${esc(noteQuery)}" /><button class="ib" data-n="new" title="Ghi chú mới (⌘N)">${I.plus}</button></div>
          <div class="n-list" id="noteItems"></div>
        </aside>
        <div class="n-edit">
          <textarea id="noteText" placeholder="Viết gì đó… dòng đầu là tiêu đề">${esc(cur ? cur.text : "")}</textarea>
          <div class="n-foot">
            ${micBtn("noteMic", "Nói để ghi")}
            <span class="dim grow" id="noteState">${cur ? "Đã lưu " + fmtAgo(cur.updated) : ""}</span>
            <span id="noteActs" class="${cur ? "" : "hide"}">
              <button class="ib ${cur && cur.pinned ? "on" : ""}" data-n="pin" title="Ghim lên đầu">${I.pin}</button>
              <button class="ib" data-n="copy" title="Sao chép">${I.copy}</button>
              <button class="ib" data-n="del" title="Xoá">${I.trash}</button></span>
          </div>
        </div></div>`;
      drawNoteItems();
      const ta = $("#noteText");
      let deb;
      const save = () => {
        clearTimeout(deb);
        deb = setTimeout(async () => {
          if (A.composing) return save(); // chờ bộ gõ ghép xong chữ
          if (!ta.value.trim() && noteId == null) return;
          const n = await api("save_note", { id: noteId, text: ta.value });
          noteId = n.id;
          notes = await api("list_notes");
          drawNoteItems();
          $("#noteState").textContent = "Đã lưu";
          $("#noteActs").classList.remove("hide"); // KHÔNG vẽ lại ô soạn thảo → không mất chữ đang gõ
        }, 500);
      };
      ta.addEventListener("input", () => { $("#noteState").textContent = "Đang lưu…"; save(); });
      ta.addEventListener("compositionend", save);
      ta.addEventListener("keydown", (e) => { if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "n") { e.preventDefault(); newNote(); } });
      $("#noteSearch").addEventListener("input", (e) => { noteQuery = e.target.value; drawNoteItems(); });
      $("#noteMic").addEventListener("click", (e) => dictate(e.currentTarget, (text) => {
        const pos = ta.selectionStart ?? ta.value.length;
        const before = ta.value.slice(0, pos);
        ta.value = before + (before && !/\s$/.test(before) ? " " : "") + text + ta.value.slice(pos);
        ta.dispatchEvent(new Event("input"));
        ta.focus();
      }));
      setTimeout(() => ta.focus(), 80);
    }
    function drawNoteItems() {
      const q = noteQuery.trim().toLowerCase();
      const list = notes.filter((n) => !q || n.text.toLowerCase().includes(q));
      $("#noteItems").innerHTML = list.length
        ? list.map((n) => {
            const [first, ...rest] = n.text.split("\n");
            return `<button class="n-item ${n.id === noteId ? "on" : ""}" data-n="open" data-id="${n.id}">
              <b>${n.pinned ? `<span class="pin">${I.pin}</span>` : ""}${esc(first.trim() || "Ghi chú trống")}</b>
              <span>${fmtAgo(n.updated)}${rest.join(" ").trim() ? " · " + esc(rest.join(" ").trim().slice(0, 40)) : ""}</span></button>`;
          }).join("")
        : `<div class="empty sm">${q ? "Không tìm thấy" : "Chưa có ghi chú"}</div>`;
    }
    function newNote() {
      noteId = null;
      const ta = $("#noteText");
      ta.value = "";
      ta.focus();
      $("#noteState").textContent = "";
      $("#noteActs").classList.add("hide");
      $$(".n-item").forEach((x) => x.classList.remove("on"));
    }
    view.addEventListener("click", async (e) => {
      if (A.tab !== "notes") return;
      const b = e.target.closest("[data-n]");
      if (!b) return;
      const a = b.dataset.n;
      if (a === "open") { noteId = Number(b.dataset.id); viewNotes(); }
      if (a === "new") newNote();
      if (a === "pin") { await api("pin_note", { id: noteId }); viewNotes(); }
      if (a === "copy") { try { await navigator.clipboard.writeText($("#noteText").value); toast("Đã sao chép"); } catch { toast("Không sao chép được"); } }
      if (a === "del") {
        if (b.dataset.sure !== "1") { b.dataset.sure = "1"; b.classList.add("danger"); b.title = "Bấm lần nữa để xoá"; setTimeout(() => { b.dataset.sure = ""; b.classList.remove("danger"); }, 2500); return; }
        await api("delete_note", { id: noteId }); noteId = null; viewNotes();
      }
    });

    // ═════════ DỌN MÁY ═════════
    let scan = null, scanning = false, cleanResult = null, confirmClean = false;
    let sel = { rules: new Set(), paths: new Set() };
    const openRules = new Set();
    function ring(d) {
      if (!d) return "";
      const used = 1 - d.free_percent / 100, Cc = 2 * Math.PI * 26;
      const col = d.level === "critical" ? "var(--red)" : d.level === "warn" ? "var(--amber)" : "var(--green)";
      return `<div class="ring"><svg viewBox="0 0 64 64"><circle cx="32" cy="32" r="26" stroke="rgba(255,255,255,.1)"/>
        <circle cx="32" cy="32" r="26" stroke="${col}" stroke-dasharray="${Cc}" stroke-dashoffset="${Cc * (1 - used)}"/></svg><b>${Math.round(d.free_percent)}%</b></div>`;
    }
    function selectedBytes() {
      if (!scan) return 0;
      let n = 0;
      for (const r of scan.report.rules) { if (r.locked) continue; for (const it of r.items) if (sel.rules.has(r.id) || sel.paths.has(it.path)) n += it.bytes; }
      return n;
    }
    function selectSafe() {
      sel = { rules: new Set(), paths: new Set() };
      for (const r of scan.report.rules) if (!r.locked && r.level === "safe" && r.id !== "system-trash" && r.bytes > 0) sel.rules.add(r.id);
    }
    function viewClean() {
      const disk = scan ? scan.disk : A.status && A.status.disk;
      const head = `<div class="disk">${ring(disk)}
        <div class="grow">${disk ? `<div class="num">${bytes(disk.free_bytes)} <span>trống / ${bytes(disk.total_bytes)}</span></div>
          <div class="dim">${disk.level === "ok" ? "Ổ đĩa còn thoải mái" : disk.level === "warn" ? "Sắp đầy — nên dọn bớt" : "Gần hết chỗ!"}</div>` : "<b>Ổ đĩa</b>"}</div>
        ${scan && !cleanResult ? `<button class="btn" data-c="scan" ${scanning ? "disabled" : ""}>${scanning ? '<span class="spin"></span>' : "Quét lại"}</button>` : ""}</div>`;
      if (cleanResult) {
        const r = cleanResult;
        view.innerHTML = head + `<div class="empty"><b class="big ok">Đã giải phóng ${bytes(r.deleted_bytes + (r.trashed_bytes || 0))}</b>
          <span>${r.trashed_bytes ? `${bytes(r.trashed_bytes)} file của bạn nằm trong Thùng rác (khôi phục được)` : `${r.items_done} mục`}</span>
          ${r.failures.length ? `<span class="warn">${r.failures.length} mục không xoá được (đang dùng hoặc thiếu quyền)</span>` : ""}
          <button class="btn" data-c="again">Xong</button></div>`;
        return;
      }
      if (!scan) {
        view.innerHTML = head + `<div class="empty"><b>Tìm file rác trên máy</b><span>Cache, nhật ký, file cài đặt cũ… Quét chỉ đọc — không xoá gì cho tới khi bạn bấm Dọn.</span>
          <button class="btn primary" data-c="scan" ${scanning ? "disabled" : ""}>${scanning ? '<span class="spin"></span>Đang quét…' : "Quét máy"}</button></div>`;
        return;
      }
      const rep = scan.report;
      const rows = rep.rules.filter((r) => r.bytes > 0).map((r) => {
        const all = r.items.every((it) => sel.paths.has(it.path)), some = r.items.some((it) => sel.paths.has(it.path));
        const st = r.locked ? "" : sel.rules.has(r.id) || all ? "on" : some ? "part" : "";
        const open = openRules.has(r.id);
        const tag = r.locked ? `<span class="tag violet">Pro</span>` : r.level === "safe" ? "" : `<span class="tag amber">Xem lại</span>`;
        return `<div class="rule ${r.locked ? "locked" : ""}">
          <div class="rule-head">
            <button class="box ${st}" data-c="rule" data-id="${r.id}" ${r.locked ? "disabled" : ""}>${I.check}</button>
            <button class="rule-name" data-c="open" data-id="${r.id}"><span class="chev ${open ? "open" : ""}">${I.chevron}</span>${esc(r.name)}${tag}${r.action === "trash" ? `<span class="tag" title="Vào Thùng rác">${I.trash}</span>` : ""}</button>
            <span class="sz">${bytes(r.bytes)}</span>
          </div>
          ${open ? `<div class="rule-items">${r.items.slice(0, 40).map((it) => `<div class="ritem">
              <button class="box ${sel.rules.has(r.id) || sel.paths.has(it.path) ? "on" : ""}" data-c="item" data-rule="${r.id}" data-p="${esc(it.path)}" ${r.locked ? "disabled" : ""}>${I.check}</button>
              <span class="p" title="${esc(it.path)}">&lrm;${esc(shortPath(it.path))}</span><span class="dim">${it.age_days} ngày</span><b>${bytes(it.bytes)}</b></div>`).join("")}
              ${r.items.length > 40 ? `<div class="dim">… và ${r.items.length - 40} mục khác</div>` : ""}</div>` : ""}
        </div>`;
      }).join("");
      const n = selectedBytes();
      const fda = rep.skipped.some((s) => s.reason.includes("Full Disk Access"));
      view.innerHTML = head + `
        <div class="sec">Có thể dọn<span>${bytes(rep.available_bytes)}</span><span class="grow"></span><button class="link" data-c="safe">Chọn mục an toàn</button><button class="link" data-c="none">Bỏ chọn</button></div>
        <div class="rules">${rows || '<div class="empty sm">Máy sạch sẽ, không có file rác</div>'}</div>
        ${rep.locked_bytes > 0 ? `<button class="upsell" data-c="pro">${I.sparkle}<span class="grow"><b>Bản Pro dọn thêm ${bytes(rep.locked_bytes)}</b><span>File cài đặt cũ, thư mục build, sao lưu iPhone…</span></span>${I.chevron}</button>` : ""}
        ${fda && A.geo.platform === "macos" ? `<button class="link" data-c="fda">Cấp quyền Full Disk Access để quét sâu hơn</button>` : ""}
        <div class="foot">
          ${confirmClean
            ? `<span class="grow">Cache sẽ <b>xoá hẳn</b> (app tự tạo lại) · file của bạn vào <b>Thùng rác</b></span><button class="btn ghost" data-c="cancel">Huỷ</button><button class="btn danger" data-c="go">Dọn ${bytes(n)}</button>`
            : `<span class="grow dim">Đã chọn ${bytes(n)}</span><button class="btn primary" data-c="ask" ${n ? "" : "disabled"}>Dọn ${bytes(n)}</button>`}
        </div>`;
    }
    view.addEventListener("click", async (e) => {
      if (A.tab !== "clean") return;
      const b = e.target.closest("[data-c]");
      if (!b || b.disabled) return;
      const a = b.dataset.c;
      const rule = scan && scan.report.rules.find((r) => r.id === b.dataset.id || r.id === b.dataset.rule);
      if (a === "scan") {
        scanning = true; cleanResult = null; confirmClean = false;
        viewClean();
        flashMood("focus", 60000);
        try { scan = await api("clean_scan"); selectSafe(); } catch (err) { toast(String(err)); }
        scanning = false;
        flashMood(scan && scan.report.available_bytes > 2e9 ? "worried" : "happy", 2000);
      }
      if (a === "rule" && rule) {
        const isOn = sel.rules.has(rule.id) || rule.items.every((it) => sel.paths.has(it.path));
        sel.rules.delete(rule.id);
        rule.items.forEach((it) => sel.paths.delete(it.path));
        if (!isOn) sel.rules.add(rule.id);
      }
      if (a === "item" && rule) {
        if (sel.rules.has(rule.id)) { sel.rules.delete(rule.id); rule.items.forEach((it) => sel.paths.add(it.path)); }
        sel.paths.has(b.dataset.p) ? sel.paths.delete(b.dataset.p) : sel.paths.add(b.dataset.p);
      }
      if (a === "open") openRules.has(b.dataset.id) ? openRules.delete(b.dataset.id) : openRules.add(b.dataset.id);
      if (a === "safe") selectSafe();
      if (a === "none") sel = { rules: new Set(), paths: new Set() };
      if (a === "ask") confirmClean = true;
      if (a === "cancel") confirmClean = false;
      if (a === "pro") return A.openSettings("pro");
      if (a === "fda") return api("open_disk_settings");
      if (a === "again") { cleanResult = null; scan = null; }
      if (a === "go") {
        confirmClean = false;
        b.disabled = true;
        b.innerHTML = '<span class="spin"></span>Đang dọn…';
        try {
          cleanResult = await api("clean_run", { rules: [...sel.rules], paths: [...sel.paths] });
          confetti(40);
          flashMood("love", 4000);
          A.status = await api("get_status");
          if (scan) scan.disk = A.status.disk;
        } catch (err) { toast(String(err)); }
      }
      viewClean();
    });

    Object.assign(A.views, { home: viewHome, tasks: viewTasks, notes: viewNotes, clean: viewClean });
  }
  if (window.ASI) boot();
  else document.addEventListener("asi-ready", boot, { once: true });
})();
