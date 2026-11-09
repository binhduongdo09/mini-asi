/* ───────── Mini-Asi — tab Clipboard · Nhật ký · Threads ─────────
 * Clipboard: bấm = chép lại · bấm đúp = dán vào app đang dùng · ghim để giữ mãi.
 * Nhật ký (mô-đun bật trong Cài đặt): Asi hỏi 3 câu, bạn trả lời 2–3 câu (gõ hoặc nói).
 * Threads (mô-đun): mở Threads trong cửa sổ riêng; nâng cao: thống kê & đăng bài qua Threads API.
 */
(function () {
  "use strict";
  function boot() {
    const A = window.ASI;
    const { api, on, $, $$, esc, I, toast, confetti, flashMood, fmtDue, fmtAgo, bytes, mins, isComposing, typing, micBtn, dictate, WDN, pad } = A;
    const view = $("#view");
    const here = (t) => A.mode === "expanded" && A.tab === t;

    // ═════════ CLIPBOARD ═════════
    let cq = "", ckind = "all", searchTimer = null, clickTimer = null;
    const thumbs = new Map();
    const KINDS = [["all", "Tất cả"], ["text", "Chữ"], ["image", "Ảnh"], ["files", "File"]];

    function viewClips() {
      view.innerHTML = `<div class="search">${I.search}<input class="field" id="clipSearch" placeholder="Tìm trong những gì đã chép (cả tên mục ghim)" value="${esc(cq)}" /></div>
        <div class="filters">${KINDS.map(([k, n]) => `<button class="chip-btn ${ckind === k ? "on" : ""}" data-ck="${k}">${n}</button>`).join("")}
          <span class="grow"></span><button class="link" data-cl="clear" title="Xoá lịch sử, giữ mục đã ghim">Xoá lịch sử</button></div>
        <div id="clipList"></div>`;
      $("#clipSearch").addEventListener("input", (e) => { cq = e.target.value; clearTimeout(searchTimer); searchTimer = setTimeout(drawClips, 100); });
      setTimeout(() => $("#clipSearch") && $("#clipSearch").focus(), 80);
      drawClips();
    }
    function clipRow(c, auto) {
      let body = "";
      if (c.kind === "text") body = `<div class="cl-text" translate="no">${esc(c.preview)}</div>`;
      if (c.kind === "image") body = `<img class="cl-img" data-img="${c.id}" alt="" />`;
      if (c.kind === "files") body = `<div class="cl-files">${c.files.map((f) => `<span class="fchip ${f.exists ? "" : "gone"}" title="${esc(f.path)}">${I.file}${esc(f.name)}</span>`).join("")}</div>`;
      const meta = c.pinned
        ? (c.uses ? `đã dùng ${c.uses} lần` : "chưa dùng")
        : [fmtAgo(c.used), c.kind === "text" ? `${c.chars} ký tự` : c.kind === "image" ? bytes(c.bytes) : `${c.files.length} file`].join(" · ");
      return `<div class="row clip ${c.pinned ? "pinned" : ""}" data-id="${c.id}">
        <div class="cl-body" data-cl="${auto ? "paste" : "copy"}" title="${auto ? "Bấm để dán" : "Bấm để chép · bấm đúp để dán"}">
          ${c.label ? `<b class="cl-label" translate="no">${esc(c.label)}</b>` : ""}${body}<span class="meta">${meta}</span></div>
        <span class="acts">
          ${c.pinned ? `<button class="ib" data-cl="label" title="Đặt tên">${I.note}</button>` : c.kind === "text" ? `<button class="ib" data-cl="note" title="Lưu thành ghi chú">${I.note}</button>` : ""}
          ${c.kind !== "files" ? `<button class="ib" data-cl="cap" title="Tạo việc · lịch từ mục này">${I.cal}</button>` : ""}
          <button class="ib ${c.pinned ? "on" : ""}" data-cl="pin" title="${c.pinned ? "Bỏ ghim" : "Ghim — giữ mãi, không bị xoá"}">${I.pin}</button>
          <button class="ib" data-cl="del" title="Xoá">${I.trash}</button>
        </span></div>`;
    }
    async function drawClips() {
      const box = $("#clipList");
      if (!box) return;
      const auto = A.settings.clip_autopaste;
      // Tất cả + không tìm: mục ghim luôn nằm riêng ở đầu (theo thứ tự ghim), bên dưới là lịch sử
      const split = ckind === "all" && !cq.trim();
      const [pinned, list] = split
        ? await Promise.all([api("clip_list", { query: "", kind: "pinned" }), api("clip_list", { query: "", kind: "history" })])
        : [[], await api("clip_list", { query: cq, kind: ckind })];
      if (!list.length && !pinned.length) {
        box.innerHTML = cq || ckind !== "all"
          ? `<div class="empty sm">Không tìm thấy mục nào</div>`
          : `<div class="empty"><b>Chưa có gì</b><span>Chép (⌘C) chữ, ảnh hay file — Asi nhớ giúp bạn.<br>Bấm một mục để chép lại · bấm đúp để dán luôn · ghim để giữ mãi.</span></div>`;
        return;
      }
      let html = "";
      if (split && pinned.length) html += `<div class="sec">Đã ghim<span>${pinned.length}</span><span class="grow"></span><span class="hint-txt">giữ mãi cho tới khi bỏ ghim</span></div>` + pinned.map((c) => clipRow(c, auto)).join("");
      if (split && pinned.length && list.length) html += `<div class="sec">Gần đây<span>${list.length}</span></div>`;
      html += list.map((c) => clipRow(c, auto)).join("");
      box.innerHTML = html;
      await Promise.all($$("img[data-img]", box).map(async (img) => {
        const id = Number(img.dataset.img);
        if (!thumbs.has(id)) thumbs.set(id, await api("clip_image", { id }));
        if (thumbs.get(id)) img.src = thumbs.get(id);
      }));
    }
    /** Ô sửa tên ngay tại chỗ cho mục ghim */
    function editLabel(row) {
      const id = Number(row.dataset.id);
      const old = row.querySelector(".cl-label")?.textContent || "";
      const body = row.querySelector(".cl-body");
      const inp = document.createElement("input");
      inp.className = "field cl-label-input";
      inp.value = old;
      inp.placeholder = "Tên gợi nhớ, vd. STK công ty";
      body.prepend(inp);
      row.querySelector(".cl-label")?.remove();
      inp.focus();
      let done = false;
      const finish = async (save) => {
        if (done) return;
        done = true;
        if (save) await api("clip_label", { id, label: inp.value });
        drawClips();
      };
      inp.addEventListener("keydown", (k) => { if (A.isComposing(k)) return; if (k.key === "Enter") finish(true); if (k.key === "Escape") { k.stopPropagation(); finish(false); } });
      inp.addEventListener("blur", () => finish(true));
      inp.addEventListener("click", (e) => e.stopPropagation());
    }
    async function copyClip(id, paste) {
      try {
        const pasted = await api("clip_copy", { id, paste });
        if (pasted) { A.setMode("collapsed"); toast("Đã dán"); }
        else toast(paste ? "Đã chép — ⌘V để dán (cấp quyền Trợ năng để tự dán)" : "Đã chép — ⌘V để dán");
        flashMood("happy", 900);
        drawClips();
      } catch (err) { toast(String(err)); }
    }
    view.addEventListener("click", async (e) => {
      if (A.tab !== "clips") return;
      const k = e.target.closest("[data-ck]");
      if (k) { ckind = k.dataset.ck; return viewClips(); }
      const b = e.target.closest("[data-cl]");
      if (!b) return;
      const a = b.dataset.cl, id = Number(b.closest(".clip")?.dataset.id);
      if (a === "clear") {
        if (b.dataset.sure !== "1") { b.dataset.sure = "1"; b.textContent = "Bấm lần nữa để xoá"; b.classList.add("danger"); return; }
        await api("clip_clear"); thumbs.clear(); toast("Đã xoá lịch sử (giữ mục ghim)"); return viewClips();
      }
      if (a === "label") return editLabel(b.closest(".clip"));
      if (a === "copy") { if (e.target.closest(".cl-label-input")) return; clearTimeout(clickTimer); clickTimer = setTimeout(() => copyClip(id, false), 230); return; } // chờ xem có bấm đúp không
      if (a === "paste") { clearTimeout(clickTimer); return copyClip(id, true); }
      if (a === "pin") { await api("clip_pin", { id }); drawClips(); }
      if (a === "del") { await api("clip_delete", { id }); thumbs.delete(id); drawClips(); }
      if (a === "note") { await api("clip_to_note", { id }); toast("Đã lưu vào Ghi chú"); }
      if (a === "cap") { try { window.__asi.openCapture(await api("capture_from_clip", { id })); } catch (err) { toast(String(err)); } }
    });
    view.addEventListener("dblclick", (e) => {
      if (A.tab !== "clips") return;
      const body = e.target.closest(".cl-body");
      if (!body) return;
      clearTimeout(clickTimer);
      copyClip(Number(body.closest(".clip").dataset.id), true);
    });
    on("clip", () => { if (here("clips") && !(typing() && document.activeElement?.closest?.("#clipList"))) drawClips(); });

    // ═════════ NHẬT KÝ ═════════
    const MOODS = [[1, "😢", "Tệ"], [2, "😕", "Hơi buồn"], [3, "😐", "Bình thường"], [4, "🙂", "Vui"], [5, "😄", "Tuyệt vời"]];
    const dstr = (d) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
    let jdate = dstr(new Date()), jv = null, showHistory = false, freeMode = false;

    function factsLine(f) {
      const c = [];
      if (f.tasks_done && f.tasks_done.length) c.push(`${f.tasks_done.length} việc xong`);
      if (f.events && f.events.length) c.push(`${f.events.length} sự kiện`);
      if (f.screen_minutes) c.push(`${mins(f.screen_minutes)} dùng máy`);
      if (f.notes_written) c.push(`${f.notes_written} ghi chú`);
      return c.join(" · ");
    }
    async function viewJournal() {
      jv = await api("journal_get", { date: jdate });
      const e = jv.entry, qs = jv.questions;
      const d = new Date(jdate + "T12:00:00");
      const isToday = jdate === dstr(new Date());
      const nextIdx = qs.findIndex((q) => !e.answers.some((x) => x.q === q));
      const done = nextIdx === -1;
      const answered = e.answers.filter((x) => x.a.trim()).length;
      let chat = `<div class="bubble asi">${isToday ? "Mình trò chuyện chút về hôm nay nhé — chỉ 3 câu thôi." : `Nhật ký ngày ${pad(d.getDate())}/${pad(d.getMonth() + 1)}`}</div>`;
      for (const qa of e.answers) {
        chat += `<div class="bubble asi">${esc(qa.q)}</div>`;
        chat += qa.a.trim() ? `<div class="bubble me">${esc(qa.a).replace(/\n/g, "<br>")}</div>` : `<div class="bubble me skipped">Bỏ qua</div>`;
      }
      if (!done) chat += `<div class="bubble asi new">${esc(qs[nextIdx])}${jv.hints && jv.hints[nextIdx] ? `<span class="hint-in">${esc(jv.hints[nextIdx])}</span>` : ""}</div>`;
      else {
        chat += `<div class="bubble asi">${esc(jv.closing || (answered ? "Asi đã ghi lại ngày hôm nay của bạn." : "Khi nào muốn kể thì quay lại nhé."))}</div>`;
        if (e.text.trim()) chat += `<div class="bubble me">${esc(e.text).replace(/\n/g, "<br>")}</div>`;
      }
      const showInput = !done || freeMode;
      const facts = factsLine(e.facts || {});
      view.innerHTML = `<div class="journal">
        <div class="j-head">
          <button class="ib" data-j="prev" title="Ngày trước">${I.chevron}</button>
          <div class="j-date"><b>${isToday ? "Hôm nay" : WDN[d.getDay()]}, ${pad(d.getDate())}/${pad(d.getMonth() + 1)}</b>${jv.streak ? `<span class="tag amber">${jv.streak} ngày liền</span>` : ""}</div>
          <button class="ib" data-j="next" title="Ngày sau" ${isToday ? "disabled" : ""}>${I.chevron}</button>
          <span class="grow"></span>
          <button class="link" data-j="hist">${showHistory ? "Đóng" : "Các ngày trước"}</button>
          <button class="link" data-j="export" title="Xuất toàn bộ nhật ký ra file Markdown">Xuất</button>
        </div>
        ${showHistory ? `<div class="j-hist">${jv.history.length ? jv.history.map((h) => `<button class="j-hrow" data-j="open" data-d="${h.date}"><span>${h.mood ? MOODS[h.mood - 1][1] : "·"}</span><b>${h.date.split("-").reverse().slice(0, 2).join("/")}</b><span class="dim">${esc(h.preview)}</span></button>`).join("") : '<div class="dim">Chưa có ngày nào</div>'}</div>` : ""}
        <div class="j-mood"><span class="dim">Tâm trạng</span>${MOODS.map(([v, em, name]) => `<button class="mood ${e.mood === v ? "on" : ""}" data-j="mood" data-v="${v}" title="${name}">${em}</button>`).join("")}${facts ? `<span class="grow"></span><span class="dim">${facts}</span>` : ""}</div>
        <div class="chat" id="jChat">${chat}</div>
        ${showInput ? `<div class="composer">
            ${micBtn("jMic", "Nói để trả lời")}
            <textarea class="field" id="jText" rows="1" placeholder="${freeMode ? "Viết thêm điều bạn muốn nhớ…" : "2–3 câu thôi…"}"></textarea>
            ${!done && !freeMode ? `<button class="link" data-j="skip">Bỏ qua</button>` : ""}
            <button class="send" data-j="send" title="${freeMode ? "Lưu" : "Gửi"}">${I.send}</button>
          </div>` : `<div class="j-done"><button class="btn" data-j="more">${I.note}Viết thêm</button>${e.answers.length ? `<button class="link" data-j="redo">Trả lời lại</button>` : ""}</div>`}
      </div>`;
      const chatEl = $("#jChat");
      const toEnd = () => { chatEl.scrollTop = chatEl.scrollHeight; };
      toEnd(); setTimeout(toEnd, 350);
      const ta = $("#jText");
      if (ta) {
        if (freeMode) ta.value = e.text;
        const grow = () => { ta.style.height = "auto"; ta.style.height = Math.min(96, ta.scrollHeight) + "px"; };
        ta.addEventListener("input", grow);
        ta.addEventListener("keydown", (k) => { if (k.key === "Enter" && !k.shiftKey && !isComposing(k)) { k.preventDefault(); send(); } });
        $("#jMic").addEventListener("click", (ev) => dictate(ev.currentTarget, (text) => { ta.value = (ta.value ? ta.value.trimEnd() + " " : "") + text; grow(); ta.focus(); }));
        setTimeout(() => ta.focus(), 80);
        if (window.__asiJournalTalk) { window.__asiJournalTalk = false; setTimeout(() => $("#jMic") && $("#jMic").click(), 400); }
      }
    }
    async function send(skip = false) {
      const ta = $("#jText");
      const text = skip ? "" : ta ? ta.value.trim() : "";
      const e = jv.entry, wasFree = freeMode;
      if (freeMode) { e.text = text; freeMode = false; }
      else {
        const q = jv.questions.find((q) => !e.answers.some((x) => x.q === q));
        if (!q || (!text && !skip)) return;
        e.answers.push({ q, a: text });
      }
      jv.entry = await api("journal_save", { entry: e });
      const finished = jv.questions.every((q) => jv.entry.answers.some((x) => x.q === q));
      await viewJournal();
      if (finished && !wasFree && !skip) {
        if (jv.entry.answers.some((x) => x.a.trim())) { confetti(18); flashMood("love", 2500); }
        if (jv.closing) api("asi_speak", { text: jv.closing });
      }
    }
    view.addEventListener("click", async (ev) => {
      if (A.tab !== "journal") return;
      const b = ev.target.closest("[data-j]");
      if (!b || b.disabled) return;
      const a = b.dataset.j;
      const shift = (n) => { const d = new Date(jdate + "T12:00:00"); d.setDate(d.getDate() + n); return dstr(d); };
      if (a === "send") return send();
      if (a === "skip") return send(true);
      if (a === "mood") {
        jv.entry.mood = Number(b.dataset.v);
        jv.entry = await api("journal_save", { entry: jv.entry });
        flashMood(jv.entry.mood >= 4 ? "happy" : jv.entry.mood <= 2 ? "worried" : "idle", 1500);
        $$(".mood").forEach((m) => m.classList.toggle("on", m === b));
        return;
      }
      if (a === "prev") { jdate = shift(-1); freeMode = false; return viewJournal(); }
      if (a === "next") { if (jdate < dstr(new Date())) { jdate = shift(1); freeMode = false; viewJournal(); } return; }
      if (a === "open") { jdate = b.dataset.d; showHistory = false; return viewJournal(); }
      if (a === "hist") { showHistory = !showHistory; return viewJournal(); }
      if (a === "more") { freeMode = true; return viewJournal(); }
      if (a === "redo") { jv.entry.answers = []; jv.entry = await api("journal_save", { entry: jv.entry }); return viewJournal(); }
      if (a === "export") { try { const p = await api("journal_export"); toast("Đã lưu " + p.split(/[\\/]/).pop()); } catch (err) { toast(String(err)); } }
    });
    A.views.clips = viewClips;
    A.views.journal = viewJournal;
  }
  if (window.ASI) boot();
  else document.addEventListener("asi-ready", boot, { once: true });
})();

/* ───────── Tab Threads ───────── */
(function () {
  "use strict";
  function boot() {
    const A = window.ASI;
    const { api, on, $, esc, I, toast, confetti, flashMood, fmtDue, micBtn, dictate } = A;
    const view = $("#view");
    let tv = null, openPost = null, repliesOf = {}, replyTo = null, confirmPost = false, loading = false, draft = "";
    const tsMs = (t) => (t ? Date.parse(t.replace(/\+0000$/, "Z")) : 0);
    const ago = (t) => { const ms = tsMs(t); return ms ? A.fmtAgo(ms) : ""; };
    const num = (n) => (n >= 1e6 ? (n / 1e6).toFixed(1) + "M" : n >= 1e3 ? (n / 1e3).toFixed(1) + "K" : String(n || 0));

    async function load(refresh) {
      loading = true;
      if (A.tab === "threads") draw();
      try { tv = await api("threads_get", { refresh }); } catch (err) { toast(String(err)); }
      loading = false;
      if (A.tab === "threads" && A.mode === "expanded" && !(A.typing() && document.activeElement?.closest?.("#view"))) draw();
    }
    // ───────── Trang chính tab Social: mỗi mạng một cửa sổ dạng điện thoại ─────────
    const NETS = [
      ["facebook", "Facebook", "Fb"], ["messenger", "Messenger", "Ms"], ["instagram", "Instagram", "Ig"], ["threads", "Threads", "Th"],
      ["x", "X", "X"], ["tiktok", "TikTok", "Tk"], ["youtube", "YouTube", "Yt"],
    ];
    let openNets = [];
    const draw = () => (A.threadsInline() ? inlineView() : window.__thApi ? drawApi() : socialHome());
    function socialHome() {
      const c = tv && tv.connected ? tv.cache : null, user = c && c.profile ? c.profile.username || "" : "";
      const unread = c ? (c.unread || 0) + (c.mention_unread || 0) : 0;
      view.innerHTML = `<div class="soc">
        <div class="soc-head"><b>Social</b><span class="dim">Bấm để mở dạng cửa sổ điện thoại · đăng nhập một lần là nhớ · bấm lần nữa để ẩn</span></div>
        <div class="soc-grid">${NETS.map(([id, name, ab]) => `<button class="soc-tile ${openNets.includes(id) ? "on" : ""}" data-soc="${id}" title="Mở ${name}">
            <span class="soc-ic">${ab}</span><b>${name}</b>${openNets.includes(id) ? `<i class="soc-dot"></i>` : ""}</button>`).join("")}</div>
        <div class="soc-th">
          <span class="soc-ic sm">Th</span>
          <span class="grow">${c ? `<b>@${esc(user)}</b><span class="dim">${unread ? `${unread} trả lời / nhắc đến mới` : "Đăng bài, xem trả lời & nhắc đến ngay trong tai thỏ"}</span>` : `<b>Threads nâng cao</b><span class="dim">Đăng bài, thống kê, báo trả lời mới (cần Threads API)</span>`}</span>
          ${unread ? `<span class="tag red">${unread}</span>` : ""}
          <button class="btn sm" data-thn="api">${c ? "Mở" : "Kết nối"}</button>
        </div>
        ${A.settings.threads_embed_beta && A.DISP !== "bubble" ? `<button class="link center" data-thn="inline">Xem Threads ngay trong tai thỏ (thử nghiệm)</button>` : ""}
      </div>`;
    }
    async function refreshOpen() {
      try { openNets = await api("social_open_list"); } catch (_) { openNets = []; }
      if (A.tab === "threads" && A.mode === "expanded" && !window.__thApi && !A.threadsInline()) socialHome();
    }
    view.addEventListener("click", async (e) => {
      if (A.tab !== "threads") return;
      const b = e.target.closest("[data-soc]");
      if (!b) return;
      A.setMode("collapsed");
      try { await api("social_open", { network: b.dataset.soc }); } catch (err) { toast(String(err)); }
    });

    function connectView() {
      return `<div class="hero">
          <div class="th-logo">@</div>
          <b>Threads trong tầm tay</b>
          <span>Mở Threads trong một cửa sổ nhỏ riêng — đăng nhập một lần, lần sau tự nhớ.</span>
          <button class="btn primary" data-thn="feed">Mở Threads</button>
        </div>
        <details class="adv"><summary>Nâng cao · thống kê bài viết & báo trả lời mới (cần Meta app)</summary>
          <div class="adv-body">
            <div class="field-row"><input class="field" id="thToken" placeholder="Dán Threads access token" /><button class="btn primary sm" data-th="token">Kết nối</button></div>
            <p class="dim">Lấy token: developers.facebook.com › app của bạn › Use cases › Access the Threads API › User Token Generator. <button class="link" data-th="dev">Mở Meta for Developers</button></p>
            <div class="sec">Hoặc đăng nhập OAuth</div>
            <div class="th-grid">
              <input class="field" id="thApp" placeholder="App ID" value="${esc(tv ? tv.app_id : "")}" />
              <input class="field" id="thSecret" type="password" placeholder="${tv && tv.has_app ? "App Secret (đã lưu)" : "App Secret"}" />
              <input class="field" id="thRedirect" placeholder="Redirect URI" value="${esc(tv ? tv.redirect : "https://localhost/")}" />
            </div>
            <div class="field-row"><button class="btn sm" data-th="login">Mở trang đăng nhập</button>
              <input class="field" id="thPaste" placeholder="Dán địa chỉ trang hiện ra sau khi bấm Cho phép" /><button class="btn sm" data-th="finish">Hoàn tất</button></div>
          </div></details>`;
    }
    function postCard(p, mention) {
      const m = p.metrics || {}, open = openPost === p.id, rs = repliesOf[p.id];
      return `<div class="th-post ${p.unread ? "unread" : ""}" data-pid="${esc(p.id)}">
        <div class="th-meta"><span class="grow">${mention ? `<b>@${esc(p.username || "")}</b> nhắc đến bạn · ` : ""}${ago(p.timestamp)}${p.unread ? `<span class="tag red">${p.unread} trả lời mới</span>` : ""}</span>${p.permalink ? `<button class="ib" data-th="open" data-url="${esc(p.permalink)}" title="Mở trên Threads">${I.ext}</button>` : ""}</div>
        <div class="th-text">${esc(p.text || "(bài không có chữ)")}</div>
        <div class="th-stats">${mention ? "" : `<span>${I.eye}${num(m.views)}</span><span>${I.heart}${num(m.likes)}</span><button class="link" data-th="replies">${I.message}${num(m.replies)} trả lời</button>`}
          <span class="grow"></span><button class="link" data-th="reply" data-rid="${esc(p.id)}" data-name="${esc(mention ? p.username || "" : "bài của bạn")}">Trả lời</button></div>
        ${open ? `<div class="th-replies">${!rs ? '<span class="spin"></span>' : rs.length ? rs.map((r) => `<div class="th-reply"><b>@${esc(r.username || "ẩn danh")}</b> <span class="dim">${ago(r.timestamp)}</span><div>${esc(r.text || "")}</div>
            ${r.is_reply_owned_by_me ? "" : `<button class="link" data-th="reply" data-rid="${esc(r.id)}" data-name="${esc(r.username || "")}">Trả lời</button>`}</div>`).join("") : '<div class="dim">Chưa có trả lời</div>'}</div>` : ""}
      </div>`;
    }
    function drawApi() {
      if (!tv) { view.innerHTML = `<div class="empty"><span class="spin"></span></div>`; return; }
      if (!tv.connected) { view.innerHTML = `<div class="th-bar"><button class="link" data-thn="home">${I.chevron}Social</button></div>` + connectView(); return; }
      const c = tv.cache, pr = c.profile || {}, ov = c.overview || {}, user = pr.username || "";
      view.innerHTML = `<div class="th-head">
          <button class="ib" data-thn="home" title="Về Social">${I.chevron}</button>
          <div class="th-logo sm">${esc((user[0] || "@").toUpperCase())}</div>
          <div class="grow"><b>@${esc(user)}</b><div class="dim">${ov.followers != null ? `${num(ov.followers)} người theo dõi · ` : ""}2 ngày: ${num(ov.views)} lượt xem · ${num(ov.likes)} thích</div></div>
          <button class="btn sm primary" data-thn="feed">Mở bảng tin</button>
          <button class="ib" data-th="refresh" title="Làm mới">${loading ? '<span class="spin"></span>' : I.refresh}</button>
          <button class="link" data-th="logout">Đăng xuất</button>
        </div>
        ${c.error ? `<div class="banner err">${esc(c.error)}</div>` : ""}
        <div class="th-compose">
          ${replyTo ? `<div class="th-replyto">Trả lời @${esc(replyTo.name)}<button class="link" data-th="noreply">Huỷ</button></div>` : ""}
          <textarea class="field" id="thText" rows="2" maxlength="500" placeholder="${replyTo ? "Viết trả lời…" : "Có gì mới?"}"></textarea>
          <div class="th-cbar">${micBtn("thMic", "Nói để viết")}<span class="dim grow" id="thCount">0/500</span>
            ${confirmPost ? `<span class="warn-txt">Đăng công khai với tên <b>@${esc(user)}</b>?</span><button class="btn sm ghost" data-th="cancel">Huỷ</button><button class="btn sm primary" data-th="go">Đăng</button>`
              : `<button class="btn sm primary" data-th="ask">${replyTo ? "Trả lời" : "Đăng"}</button>`}</div>
        </div>
        ${c.mentions.length ? `<div class="sec">Nhắc đến bạn<span>${c.mentions.length}</span></div>` + c.mentions.slice(0, 5).map((p) => postCard(p, true)).join("") : ""}
        <div class="sec">Bài gần đây${c.unread ? `<span class="grow"></span><button class="link" data-th="read">Đánh dấu đã đọc</button>` : ""}</div>
        ${c.posts.length ? c.posts.map((p) => postCard(p, false)).join("") : '<div class="empty sm">Chưa có bài nào</div>'}`;
      const ta = $("#thText");
      ta.value = draft;
      $("#thCount").textContent = `${[...draft].length}/500`;
      ta.addEventListener("input", () => { draft = ta.value; $("#thCount").textContent = `${[...draft].length}/500`; });
      $("#thMic").addEventListener("click", (e) => dictate(e.currentTarget, (text) => { ta.value = (ta.value ? ta.value.trimEnd() + " " : "") + text; ta.dispatchEvent(new Event("input")); ta.focus(); }));
      if (replyTo) setTimeout(() => ta.focus(), 50);
    }
    view.addEventListener("click", async (e) => {
      if (A.tab !== "threads") return;
      const b = e.target.closest("[data-th]");
      if (!b) return;
      const a = b.dataset.th, pid = b.closest(".th-post")?.dataset.pid;
      try {
        if (a === "dev") return api("open_link", { url: "https://developers.facebook.com/apps" });
        if (a === "open") { A.setMode("collapsed"); api("threads_web", { url: b.dataset.url }).catch((err) => toast(String(err))); return; }
        if (a === "token") { b.disabled = true; tv = await api("threads_set_token", { token: $("#thToken").value }); confetti(20); toast("Đã kết nối Threads"); }
        if (a === "login") {
          await api("threads_save_app", { appId: $("#thApp").value, secret: $("#thSecret").value, redirect: $("#thRedirect").value });
          await api("threads_login");
          toast("Đã mở trình duyệt — bấm Cho phép rồi chép địa chỉ trang hiện ra");
          return;
        }
        if (a === "finish") { b.disabled = true; tv = await api("threads_finish", { pasted: $("#thPaste").value }); confetti(20); toast("Đã kết nối Threads"); }
        if (a === "logout") {
          if (b.dataset.sure !== "1") { b.dataset.sure = "1"; b.textContent = "Bấm lần nữa"; return; }
          await api("threads_logout"); tv = await api("threads_get", { refresh: false });
        }
        if (a === "refresh") return load(true);
        if (a === "read") { await api("threads_mark_read"); tv = await api("threads_get", { refresh: false }); }
        if (a === "replies") {
          openPost = openPost === pid ? null : pid;
          draw();
          if (openPost) { repliesOf[pid] = await api("threads_replies", { id: pid }); tv = await api("threads_get", { refresh: false }); }
        }
        if (a === "reply") { replyTo = { id: b.dataset.rid, name: b.dataset.name }; confirmPost = false; }
        if (a === "noreply") { replyTo = null; confirmPost = false; }
        if (a === "ask") { if (!draft.trim()) { toast("Chưa có nội dung"); return; } confirmPost = true; }
        if (a === "cancel") confirmPost = false;
        if (a === "go") {
          // Chỉ đăng khi người dùng bấm Đăng VÀ xác nhận
          confirmPost = false;
          b.disabled = true;
          await api("threads_post", { text: draft, replyTo: replyTo ? replyTo.id : null });
          if (replyTo && openPost) repliesOf[openPost] = await api("threads_replies", { id: openPost });
          draft = ""; replyTo = null;
          tv = await api("threads_get", { refresh: false });
          confetti(24); flashMood("happy", 2000); toast("Đã đăng lên Threads");
        }
      } catch (err) { toast(String(err)); confirmPost = false; }
      draw();
    });
    on("threads", () => { if (A.tab === "threads" && A.mode === "expanded") load(false); });

    // Threads nhúng trong bảng (thử nghiệm, bật trong Cài đặt › Tính năng)
    function inlineView() {
      view.innerHTML = `<div class="th-bar">
          <button class="ib" data-thn="back" title="Quay lại">${I.chevron}</button>
          <button class="ib" data-thn="reload" title="Tải lại">${I.refresh}</button>
          <span class="grow"></span>
          <button class="link" data-thn="window">Mở cửa sổ riêng</button>
          <button class="link" data-thn="api">Thống kê</button>
          <button class="link" data-thn="home">Social</button>
        </div>
        <div id="thSlot" class="th-slot"><span class="spin"></span></div>`;
      setTimeout(() => A.placeEmbed(), 480);
      setTimeout(() => A.placeEmbed(), 1100);
    }
    view.addEventListener("click", async (e) => {
      if (A.tab !== "threads") return;
      const b = e.target.closest("[data-thn]");
      if (!b) return;
      const a = b.dataset.thn;
      if (a === "back" || a === "reload") return api("threads_embed_nav", { action: a });
      if (a === "window") { A.hideEmbed(); A.setMode("collapsed"); api("threads_web", {}).catch((err) => toast(String(err))); return; }
      if (a === "api") { window.__thApi = true; window.__thInline = false; A.hideEmbed(); A.switchTab("threads"); return; }
      if (a === "home") { window.__thApi = false; window.__thInline = false; A.hideEmbed(); A.switchTab("threads"); return; }
      if (a === "feed" || a === "inline") {
        window.__thApi = false;
        window.__thInline = true;
        if (!A.threadsInline()) { window.__thInline = false; A.setMode("collapsed"); api("threads_web", {}).catch((err) => toast(String(err))); return; }
        A.switchTab("threads");
      }
    });
    window.addEventListener("resize", () => { if (A.tab === "threads" && A.mode === "expanded" && $("#thSlot")) A.placeEmbed(); });
    A.views.threads = () => {
      if (A.threadsInline()) return inlineView();
      draw();
      if (!window.__thApi) refreshOpen();
      load(false);
    };
  }
  if (window.ASI) boot();
  else document.addEventListener("asi-ready", boot, { once: true });
})();
