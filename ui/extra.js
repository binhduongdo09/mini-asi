/* ───────── Mini-Asi — tab Clipboard & Nhật ký ───────── */
(function () {
  "use strict";
  function boot() {
    const A = window.ASI;
    const { api, on, $, $$, esc, I, toast, confetti, flashMood, fmtDue, fmtAgo, hm, bytes } = A;
    const ago = fmtAgo || ((ms) => (Date.now() - ms < 45000 ? "vừa xong" : fmtDue(ms).replace("trễ ", "") + " trước"));
    let searchTimer = null;

    // ═════════ CLIPBOARD ═════════
    let cq = "";
    let ckind = "all";
    let csel = new Set();
    const thumbs = new Map();
    const KINDS = [["all", "Tất cả"], ["text", "Chữ"], ["image", "Ảnh"], ["files", "File"], ["pinned", "Đã ghim"]];

    async function viewClips() {
      $("#view").innerHTML = `
        <div class="add-wrap">
          <div class="row">
            <div class="add-row grow">${I.search}<input class="input" id="clipSearch" placeholder="Tìm trong những gì đã chép…" value="${esc(cq)}" autocomplete="off" /></div>
          </div>
          <div class="row" style="margin-top:6px">
            ${KINDS.map(([k, n]) => `<button class="chip-btn ${ckind === k ? "on" : ""}" data-ck="${k}">${n}</button>`).join("")}
            <span class="grow"></span>
            <span id="mergeBox"></span>
            <button class="btn sm ghost" data-cl="clear" title="Xoá lịch sử, giữ mục đã ghim">${I.trash}Xoá lịch sử</button>
          </div>
        </div>
        <div id="clipList"></div>`;
      $("#clipSearch").addEventListener("input", (e) => {
        cq = e.target.value;
        clearTimeout(searchTimer);
        searchTimer = setTimeout(drawClips, 100);
      });
      setTimeout(() => $("#clipSearch") && $("#clipSearch").focus(), 80);
      drawClips();
    }

    async function drawClips() {
      const box = $("#clipList");
      if (!box) return;
      const list = await api("clip_list", { query: cq, kind: ckind });
      const auto = A.settings.clip_autopaste;
      $("#mergeBox").innerHTML = csel.size >= 2 ? `<button class="btn sm blue" data-cl="merge">${I.copy}Gộp ${csel.size} mục</button>` : "";
      if (!list.length) {
        box.innerHTML = `<div class="empty"><div class="big">📋</div>${cq || ckind !== "all" ? "Không tìm thấy mục nào" : "Chưa có gì. Hãy chép (⌘C) chữ, ảnh hay file bất kỳ — Asi sẽ nhớ giúp bạn."}
          <div class="small" style="margin-top:6px">Bấm một mục để chép lại · bấm đúp để dán luôn · ghim để giữ mãi</div></div>`;
        return;
      }
      box.innerHTML = list
        .map((c) => {
          let body = "";
          if (c.kind === "text") body = `<div class="cl-text">${esc(c.preview)}</div>`;
          if (c.kind === "image") body = `<div class="cl-img"><img data-img="${c.id}" alt="" />${thumbs.has(c.id) ? "" : ""}</div>`;
          if (c.kind === "files")
            body = `<div class="cl-files">${c.files.map((f) => `<span class="chip ${f.exists ? "" : "red"}" title="${esc(f.path)}">${I.file}${esc(f.name)}</span>`).join("")}</div>`;
          const meta = [
            ago(c.used),
            c.kind === "text" ? `${c.chars} ký tự` : c.kind === "image" ? bytes(c.bytes) : `${c.files.length} file`,
            c.uses ? `dán ${c.uses} lần` : "",
          ].filter(Boolean).join(" · ");
          return `<div class="clip ${c.pinned ? "pinned" : ""}" data-id="${c.id}">
            ${c.kind === "text" ? `<button class="box ${csel.has(c.id) ? "on" : ""}" data-cl="sel" title="Chọn để gộp">${I.check}</button>` : `<span class="cl-ic">${c.kind === "image" ? I.image : I.folder}</span>`}
            <div class="cl-body" data-cl="${auto ? "paste" : "copy"}" title="${auto ? "Bấm để dán" : "Bấm để chép · bấm đúp để dán"}">${body}<div class="muted small">${c.pinned ? "📌 " : ""}${meta}</div></div>
            <div class="acts">
              <button class="icon-btn" data-cl="copy" title="Chép">${I.copy}</button>
              <button class="icon-btn" data-cl="paste" title="Dán vào app đang dùng">${I.paste}</button>
              <button class="icon-btn ${c.pinned ? "on" : ""}" data-cl="pin" title="Ghim">${I.pin}</button>
              ${c.kind !== "image" ? `<button class="icon-btn" data-cl="note" title="Lưu thành ghi chú">${I.note}</button>` : ""}
              ${c.kind === "text" ? `<button class="icon-btn" data-cl="task" title="Tạo việc">${I.tasks}</button>` : ""}
              <button class="icon-btn" data-cl="del" title="Xoá">${I.trash}</button>
            </div></div>`;
        })
        .join("");
      await Promise.all(
        $$("img[data-img]", box).map(async (img) => {
          const id = Number(img.dataset.img);
          if (!thumbs.has(id)) thumbs.set(id, await api("clip_image", { id }));
          if (thumbs.get(id)) img.src = thumbs.get(id);
        })
      );
    }

    async function copyClip(id, paste) {
      try {
        const pasted = await api("clip_copy", { id, paste });
        if (pasted) {
          A.setMode("collapsed");
          toast("Đã dán ✓");
        } else if (paste) {
          toast("Đã chép — ⌘V để dán (bật quyền Trợ năng để tự dán)");
        } else {
          toast("Đã chép — ⌘V để dán ở bất kỳ đâu");
        }
        flashMood("happy", 900);
        drawClips();
      } catch (err) {
        toast(String(err));
      }
    }

    let clickTimer = null;
    $("#view").addEventListener("click", async (e) => {
      if (A.tab !== "clips") return;
      const k = e.target.closest("[data-ck]");
      if (k) { ckind = k.dataset.ck; csel.clear(); viewClips(); return; }
      const b = e.target.closest("[data-cl]");
      if (!b) return;
      const a = b.dataset.cl;
      const id = Number(b.closest(".clip")?.dataset.id);
      if (a === "clear") { await api("clip_clear"); thumbs.clear(); csel.clear(); drawClips(); toast("Đã xoá lịch sử (giữ mục ghim)"); return; }
      if (a === "merge") { await api("clip_merge", { ids: [...csel] }); csel.clear(); toast("Đã gộp & chép — ⌘V để dán"); drawClips(); return; }
      if (a === "sel") { csel.has(id) ? csel.delete(id) : csel.add(id); drawClips(); return; }
      if (a === "copy" && b.classList.contains("cl-body")) {
        // chờ xem có bấm đúp không
        clearTimeout(clickTimer);
        clickTimer = setTimeout(() => copyClip(id, false), 230);
        return;
      }
      if (a === "copy") return copyClip(id, false);
      if (a === "paste") { clearTimeout(clickTimer); return copyClip(id, true); }
      if (a === "pin") { await api("clip_pin", { id }); drawClips(); }
      if (a === "del") { await api("clip_delete", { id }); thumbs.delete(id); csel.delete(id); drawClips(); }
      if (a === "note") { await api("clip_to_note", { id }); toast("Đã lưu vào Ghi chú"); }
      if (a === "task") {
        try { const t = await api("clip_to_task", { id }); toast("Đã tạo việc" + (t.due ? " · " + fmtDue(t.due) : "")); A.refreshTasks(); } catch (err) { toast(String(err)); }
      }
    });
    $("#view").addEventListener("dblclick", (e) => {
      if (A.tab !== "clips") return;
      const body = e.target.closest(".cl-body");
      if (!body) return;
      clearTimeout(clickTimer);
      copyClip(Number(body.closest(".clip").dataset.id), true);
    });
    on("clip", () => { if (A.mode === "expanded" && A.tab === "clips" && !(A.typing() && document.activeElement?.closest?.("#clipList"))) drawClips(); });

    // ═════════ NHẬT KÝ ═════════
    const MOODS = [[1, "😢", "Tệ"], [2, "😕", "Hơi buồn"], [3, "😐", "Bình thường"], [4, "🙂", "Vui"], [5, "😄", "Tuyệt vời"]];
    const pad = (n) => String(n).padStart(2, "0");
    const dstr = (d) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
    const WDN = ["Chủ nhật", "Thứ 2", "Thứ 3", "Thứ 4", "Thứ 5", "Thứ 6", "Thứ 7"];
    let jdate = dstr(new Date());
    let jv = null; // {entry, questions, streak, history}
    let showHistory = false;
    let freeMode = false;

    function factsChips(f) {
      const c = [];
      if (f.tasks_done.length) c.push(`<span class="chip green" title="${esc(f.tasks_done.join(" · "))}">${I.check}${f.tasks_done.length} việc xong</span>`);
      if (f.events.length) c.push(`<span class="chip blue" title="${esc(f.events.join(" · "))}">${I.cal}${f.events.length} sự kiện</span>`);
      if (f.screen_minutes) c.push(`<span class="chip">${I.clock}${A.mins(f.screen_minutes)} dùng máy</span>`);
      if (f.notes_written) c.push(`<span class="chip">${I.note}${f.notes_written} ghi chú</span>`);
      if (f.clips) c.push(`<span class="chip">${I.clip}${f.clips} lần chép</span>`);
      if (f.tasks_late) c.push(`<span class="chip red">${f.tasks_late} việc còn trễ</span>`);
      return c.join("");
    }

    async function saveEntry() {
      jv.entry = await api("journal_save", { entry: jv.entry });
    }

    async function viewJournal() {
      jv = await api("journal_get", { date: jdate });
      const e = jv.entry;
      const d = new Date(jdate + "T12:00:00");
      const isToday = jdate === dstr(new Date());
      const qs = jv.questions;
      const answered = e.answers.filter((x) => x.a.trim()).length;
      const nextIdx = qs.findIndex((q) => !e.answers.some((x) => x.q === q));
      const done = nextIdx === -1;

      let chat = `<div class="bubble asi">${isToday ? "Chào bạn 👋 Mình trò chuyện chút về hôm nay nhé — chỉ 3 câu thôi." : `Nhật ký ngày ${pad(d.getDate())}/${pad(d.getMonth() + 1)}`}
        ${e.facts.tasks_done.length ? `<br><span class="muted small">Hôm nay bạn đã xong: ${esc(e.facts.tasks_done.slice(0, 4).join(", "))}${e.facts.tasks_done.length > 4 ? "…" : ""}</span>` : ""}</div>`;
      for (const qa of e.answers) {
        chat += `<div class="bubble asi">${esc(qa.q)}</div>`;
        chat += qa.a.trim()
          ? `<div class="bubble me">${esc(qa.a).replace(/\n/g, "<br>")}</div>`
          : `<div class="bubble me skipped">(bỏ qua)</div>`;
      }
      if (!done) {
        chat += `<div class="bubble asi typing-in">${esc(qs[nextIdx])}${jv.hints && jv.hints[nextIdx] ? `<span class="j-hint">${esc(jv.hints[nextIdx])}</span>` : ""}</div>`;
      } else {
        chat += `<div class="bubble asi">${esc(jv.closing || (answered ? "Mình sẽ ghi lại nhật ký ngày hôm nay của bạn nhé." : "Khi nào muốn kể thì quay lại với Asi nhé."))}</div>`;
        if (e.text.trim()) chat += `<div class="bubble me">${esc(e.text).replace(/\n/g, "<br>")}</div>`;
      }

      const showInput = !done || freeMode;
      $("#view").innerHTML = `
        <div class="j-head">
          <button class="icon-btn" data-j="prev" title="Ngày trước">‹</button>
          <div class="grow"><b>${WDN[d.getDay()]}, ${pad(d.getDate())}/${pad(d.getMonth() + 1)}/${d.getFullYear()}</b>
            ${jv.streak ? `<span class="chip amber" style="margin-left:6px">🔥 ${jv.streak} ngày liên tục</span>` : ""}</div>
          <button class="icon-btn" data-j="next" title="Ngày sau" ${isToday ? "disabled" : ""}>›</button>
          <button class="btn sm ghost" data-j="hist">${showHistory ? "Ẩn" : "Các ngày trước"}</button>
          <button class="btn sm ghost" data-j="export" title="Xuất toàn bộ nhật ký ra file Markdown">Xuất .md</button>
        </div>
        <div class="row" style="gap:6px;margin:8px 0 4px">
          <span class="muted small" style="white-space:nowrap">Tâm trạng</span>
          ${MOODS.map(([v, em, name]) => `<button class="mood ${e.mood === v ? "on" : ""}" data-j="mood" data-v="${v}" title="${name}">${em}</button>`).join("")}
        </div>
        <div class="facts">${factsChips(e.facts)}</div>
        ${showHistory ? `<div class="card j-hist">${jv.history.length ? jv.history.map((h) => `<div class="j-hrow" data-j="open" data-d="${h.date}">
            <span>${h.mood ? MOODS[h.mood - 1][1] : "·"}</span><b>${h.date.split("-").reverse().slice(0, 2).join("/")}</b><span class="muted grow">${esc(h.preview)}</span></div>`).join("") : '<div class="muted small">Chưa có ngày nào</div>'}</div>` : ""}
        <div class="chat" id="chat">${chat}</div>
        ${showInput ? `<div class="j-input">
            <textarea class="input" id="jText" rows="2" placeholder="${freeMode ? "Viết thêm điều bạn muốn nhớ…" : "2–3 câu thôi — gõ hoặc bấm mic để nói…"}"></textarea>
            <div class="j-tools">${A.langPill()}${A.micBtn("jMic")}
              ${!done ? `<button class="btn sm ghost" data-j="skip">Bỏ qua</button>` : ""}
              <button class="btn sm primary" data-j="send">${I.send}${freeMode ? "Lưu" : "Gửi"}</button></div>
          </div>` : `<div class="row" style="justify-content:center;margin-top:8px"><button class="btn sm" data-j="more">${I.note}Viết thêm</button>
            ${e.answers.length ? `<button class="btn sm ghost" data-j="redo">Trả lời lại</button>` : ""}</div>`}`;
      const chatEl = $("#chat");
      chatEl.scrollTop = chatEl.scrollHeight;
      const toEnd = () => { chatEl.scrollTop = chatEl.scrollHeight; const v = $("#view"); v.scrollTop = v.scrollHeight; }; // ô nhập dính đáy → cuộn cả khung để không che câu hỏi
      toEnd();
      setTimeout(toEnd, 350); // sau hiệu ứng hiện câu hỏi
      const ta = $("#jText");
      if (ta) {
        if (freeMode) ta.value = e.text;
        ta.addEventListener("keydown", (k) => {
          if (k.key === "Enter" && !k.shiftKey && !A.isComposing(k)) { k.preventDefault(); send(); }
        });
        $("#jMic").addEventListener("click", (ev) => A.dictate(ev.currentTarget, (text) => {
          ta.value = (ta.value ? ta.value.trimEnd() + " " : "") + text;
          ta.focus();
        }));
        setTimeout(() => ta.focus(), 80);
        if (window.__asiJournalTalk) {
          window.__asiJournalTalk = false;
          setTimeout(() => $("#jMic") && $("#jMic").click(), 400);
        }
      }
    }

    async function send(skip = false) {
      const ta = $("#jText");
      const text = skip ? "" : (ta ? ta.value.trim() : "");
      const e = jv.entry;
      const wasFree = freeMode;
      if (freeMode) {
        e.text = text;
        freeMode = false;
      } else {
        const q = jv.questions.find((q) => !e.answers.some((x) => x.q === q));
        if (!q) return;
        if (!text && !skip) return;
        e.answers.push({ q, a: text });
      }
      await saveEntry();
      const finished = !freeMode && jv.questions.every((q) => e.answers.some((x) => x.q === q));
      await viewJournal();
      if (finished && !wasFree) {
        if (e.answers.some((x) => x.a.trim())) { confetti(18); flashMood("love", 2500); }
        api("asi_speak", { text: jv.closing }); // Asi nói câu kết
      }
    }

    $("#view").addEventListener("click", async (ev) => {
      if (A.tab !== "journal") return;
      const b = ev.target.closest("[data-j]");
      if (!b) return;
      const a = b.dataset.j;
      const shift = (n) => { const d = new Date(jdate + "T12:00:00"); d.setDate(d.getDate() + n); return dstr(d); };
      if (a === "send") return send();
      if (a === "skip") return send(true);
      if (a === "mood") { jv.entry.mood = Number(b.dataset.v); await saveEntry(); flashMood(jv.entry.mood >= 4 ? "happy" : jv.entry.mood <= 2 ? "worried" : "idle", 1500); return viewJournal(); }
      if (a === "prev") { jdate = shift(-1); freeMode = false; return viewJournal(); }
      if (a === "next") { if (jdate < dstr(new Date())) { jdate = shift(1); freeMode = false; viewJournal(); } return; }
      if (a === "open") { jdate = b.dataset.d; showHistory = false; return viewJournal(); }
      if (a === "hist") { showHistory = !showHistory; return viewJournal(); }
      if (a === "more") { freeMode = true; return viewJournal(); }
      if (a === "redo") { jv.entry.answers = []; await saveEntry(); return viewJournal(); }
      if (a === "export") { try { const p = await api("journal_export"); toast("Đã lưu " + p.split(/[\\/]/).pop()); } catch (err) { toast(String(err)); } return; }
      if (a === "plans") {
        const qa = jv.entry.answers.find((x) => x.q === jv.questions[4]);
        const n = await api("journal_make_tasks", { text: qa ? qa.a : "" });
        toast(n ? `Đã tạo ${n} việc cho ngày mai ✓` : "Không tìm thấy việc nào");
        if (n) { confetti(12); A.refreshTasks(); }
      }
    });

    A.views.clips = viewClips;
    A.views.journal = () => { jdate = jdate || dstr(new Date()); viewJournal(); };
  }
  if (window.ASI) boot();
  else document.addEventListener("asi-ready", boot, { once: true });
})();

/* ───────── Tab Threads ───────── */
(function () {
  "use strict";
  function boot() {
    const A = window.ASI;
    const { api, on, $, $$, esc, I, toast, confetti, flashMood, fmtDue } = A;
    let tv = null; // ThreadsView
    let openPost = null;
    let repliesOf = {};
    let replyTo = null; // {id, name}
    let confirmPost = null; // "main" | "reply"
    let loading = false;
    const tsMs = (t) => (t ? Date.parse(t.replace(/\+0000$/, "Z")) : 0);
    const ago = (t) => { const ms = tsMs(t); return ms ? fmtDue(ms).replace("trễ ", "") + " trước" : ""; };
    const num = (n) => (n >= 1e6 ? (n / 1e6).toFixed(1) + "M" : n >= 1e3 ? (n / 1e3).toFixed(1) + "K" : String(n || 0));

    async function load(refresh) {
      loading = true;
      if (A.tab === "threads") draw();
      try { tv = await api("threads_get", { refresh }); } catch (err) { toast(String(err)); }
      loading = false;
      if (A.tab === "threads" && A.mode === "expanded" && !(A.typing() && document.activeElement?.closest?.("#view"))) draw();
    }

    function connectView() {
      return `
        <div class="th-hero">
          <div class="th-logo big">@</div>
          <b style="font-size:15px">Threads ngay trên tai thỏ</b>
          <div class="muted small" style="max-width:420px;text-align:center">Đăng nhập bằng tài khoản Instagram / Threads như trên điện thoại. Threads mở trong một cửa sổ nhỏ riêng (bản mobile) — kéo, đổi cỡ, thu nhỏ như cửa sổ thường, không đè lên app khác. Đăng nhập 1 lần, lần sau tự nhớ.</div>
          <div class="row" style="justify-content:center;margin-top:6px"><button class="btn primary" data-thn="feed" style="height:36px;padding:0 18px">📱 Mở Threads</button>
            <button class="btn ghost" data-thn="close" style="height:36px">Đóng cửa sổ Threads</button></div>
          <span class="muted small">Hoặc từ biểu tượng Mini-Asi trên thanh menu › Lướt Threads / Đóng Threads</span>
        </div>
        <details class="th-adv"><summary>Nâng cao: thống kê bài viết & báo trả lời mới trên tai thỏ (cần Meta app)</summary>
        <div class="row" style="margin:8px 0"><span class="muted small grow">Phần này dùng Threads API chính thức nên cần tạo app miễn phí trên Meta for Developers.</span>
          <button class="btn sm ghost" data-th="dev">${I.ext}Meta for Developers</button></div>
        <div class="sec">Cách 1 · Dán access token (nhanh nhất)</div>
        <div class="card">
          <div class="row"><input class="input" id="thToken" placeholder="Dán Threads access token…" style="height:32px" />
            <button class="btn primary" data-th="token">Kết nối</button></div>
          <div class="muted small" style="margin-top:6px">Lấy token: developers.facebook.com › app của bạn › Use cases › <b>Access the Threads API</b> › Settings › <b>User Token Generator</b> (thêm tài khoản của bạn làm Threads Tester).</div>
        </div>
        <div class="sec">Cách 2 · Đăng nhập bằng tài khoản Threads (OAuth)</div>
        <div class="card">
          <div class="th-grid">
            <input class="input" id="thApp" placeholder="Threads App ID" value="${esc(tv ? tv.app_id : "")}" />
            <input class="input" id="thSecret" type="password" placeholder="${tv && tv.has_app ? "App Secret (đã lưu trong Keychain)" : "Threads App Secret"}" />
            <input class="input" id="thRedirect" placeholder="Redirect URI" value="${esc(tv ? tv.redirect : "https://localhost/")}" />
          </div>
          <div class="row" style="margin-top:8px"><button class="btn" data-th="saveapp">Lưu</button>
            <button class="btn primary" data-th="login">${I.ext}Mở trang đăng nhập</button></div>
          <div class="row" style="margin-top:8px"><input class="input" id="thPaste" placeholder="Dán địa chỉ trình duyệt hiện ra sau khi bấm “Cho phép” (có ?code=…)" style="height:32px" />
            <button class="btn" data-th="finish">Hoàn tất</button></div>
          <div class="muted small" style="margin-top:6px">Nhớ thêm Redirect URI ở trên vào mục <b>Redirect Callback URLs</b> của app. Trang localhost báo lỗi là bình thường — chỉ cần chép địa chỉ của nó.</div>
        </div></details>`;
    }

    function postCard(p, isMention) {
      const m = p.metrics || {};
      const open = openPost === p.id;
      const rs = repliesOf[p.id];
      return `<div class="th-post ${p.unread ? "unread" : ""}" data-pid="${esc(p.id)}">
        <div class="row"><div class="grow">
          ${isMention ? `<b>@${esc(p.username || "")}</b> <span class="muted small">nhắc đến bạn · ${ago(p.timestamp)}</span>` : `<span class="muted small">${ago(p.timestamp)}</span>`}
          ${p.unread ? `<span class="chip red" style="margin-left:6px">${p.unread} trả lời mới</span>` : ""}</div>
          ${p.permalink ? `<button class="icon-btn" data-th="open" data-url="${esc(p.permalink)}" title="Mở trên Threads">${I.ext}</button>` : ""}</div>
        <div class="th-text">${esc(p.text || "(bài không có chữ)")}</div>
        <div class="row" style="gap:10px;margin-top:4px">
          ${isMention ? "" : `<span class="muted small">${I.eye} ${num(m.views)}</span><span class="muted small">♥ ${num(m.likes)}</span>
          <span class="muted small">↻ ${num(m.reposts)}</span>`}
          <span class="grow"></span>
          ${isMention ? "" : `<button class="btn sm ghost" data-th="replies">💬 ${num(m.replies)} trả lời ${open ? "▾" : "▸"}</button>`}
          <button class="btn sm ghost" data-th="reply" data-rid="${esc(p.id)}" data-name="${esc(isMention ? p.username || "" : "bài của bạn")}">Trả lời</button>
        </div>
        ${open ? `<div class="th-replies">${!rs ? '<span class="spin"></span>' : rs.length ? rs.map((r) => `<div class="th-reply">
            <b>@${esc(r.username || "ẩn danh")}</b> <span class="muted small">${ago(r.timestamp)}</span>
            <div>${esc(r.text || "")}</div>
            ${r.is_reply_owned_by_me ? "" : `<button class="btn sm ghost" data-th="reply" data-rid="${esc(r.id)}" data-name="${esc(r.username || "")}">Trả lời</button>`}</div>`).join("") : '<div class="muted small">Chưa có trả lời</div>'}</div>` : ""}
      </div>`;
    }

    function draw() {
      const v = $("#view");
      if (!tv) { v.innerHTML = `<div class="empty"><span class="spin"></span></div>`; return; }
      if (!tv.connected) { v.innerHTML = connectView(); return; }
      const c = tv.cache;
      const pr = c.profile || {};
      const ov = c.overview || {};
      const user = pr.username || "";
      v.innerHTML = `
        <div class="row" style="margin-bottom:8px">
          <div class="th-logo">${esc((user[0] || "@").toUpperCase())}</div>
          <div class="grow"><b>@${esc(user)}</b>${pr.is_verified ? " ✔︎" : ""} <span class="muted small">${pr.name ? esc(pr.name) : ""}</span>
            <div class="muted small">${ov.followers != null ? `${num(ov.followers)} người theo dõi · ` : ""}2 ngày qua: ${num(ov.views)} lượt xem · ${num(ov.likes)} thích · ${num(ov.replies)} trả lời</div></div>
          <button class="btn sm primary" data-thn="feed" title="Mở bảng tin Threads trong cửa sổ riêng">📱 Lướt bảng tin</button>
          <button class="btn sm ghost" data-thn="close" title="Đóng cửa sổ Threads">Đóng</button>
          <button class="icon-btn" data-th="refresh" title="Làm mới">${loading ? '<span class="spin"></span>' : I.sync}</button>
          <button class="btn sm ghost" data-th="logout">Đăng xuất</button>
        </div>
        ${c.error ? `<div class="card" style="border-left:3px solid var(--red);margin-bottom:8px"><span class="small">${esc(c.error)}</span></div>` : ""}
        <div class="card th-compose">
          ${replyTo ? `<div class="row small" style="margin-bottom:4px"><span class="chip blue">Trả lời @${esc(replyTo.name)}</span><button class="btn sm ghost" data-th="noreply">Huỷ</button></div>` : ""}
          <textarea class="input" id="thText" rows="2" maxlength="500" placeholder="${replyTo ? "Viết trả lời…" : "Có gì mới? Asi đăng giúp bạn…"}"></textarea>
          <div class="row" style="margin-top:6px">
            <span class="muted small grow" id="thCount">0/500</span>
            ${A.langPill()}${A.micBtn("thMic")}
            ${confirmPost ? `<span class="small">Đăng công khai với tên <b>@${esc(user)}</b>?</span>
              <button class="btn sm ghost" data-th="cancel">Huỷ</button><button class="btn sm primary" data-th="go">Đăng ngay</button>`
              : `<button class="btn sm primary" data-th="ask">${I.send}${replyTo ? "Trả lời" : "Đăng"}</button>`}
          </div>
        </div>
        ${c.mentions.length ? `<div class="sec">Nhắc đến bạn · ${c.mentions.length}${c.mention_unread ? ` <span class="chip red">${c.mention_unread} mới</span>` : ""}</div>` + c.mentions.slice(0, 5).map((p) => postCard(p, true)).join("") : ""}
        <div class="sec">Bài gần đây${c.unread ? ` <button class="btn sm ghost" data-th="read" style="margin-left:auto">Đánh dấu đã đọc</button>` : ""}</div>
        ${c.posts.length ? c.posts.map((p) => postCard(p, false)).join("") : '<div class="empty small">Chưa có bài nào</div>'}`;
      const ta = $("#thText");
      ta.value = draft;
      $("#thCount").textContent = `${[...draft].length}/500`;
      ta.addEventListener("input", () => { draft = ta.value; $("#thCount").textContent = `${[...draft].length}/500`; });
      $("#thMic").addEventListener("click", (e) => A.dictate(e.currentTarget, (text) => {
        ta.value = (ta.value ? ta.value.trimEnd() + " " : "") + text;
        ta.dispatchEvent(new Event("input"));
        ta.focus();
      }));
      if (replyTo) setTimeout(() => ta.focus(), 50);
    }
    let draft = "";

    $("#view").addEventListener("click", async (e) => {
      if (A.tab !== "threads") return;
      const b = e.target.closest("[data-th]");
      if (!b) return;
      const a = b.dataset.th;
      const pid = b.closest(".th-post")?.dataset.pid;
      try {
        if (a === "dev") return api("open_link", { url: "https://developers.facebook.com/apps" });
        if (a === "open") { A.setMode("collapsed"); api("threads_web", { url: b.dataset.url }).catch((e) => toast(String(e))); return; }
        if (a === "web") { A.setMode("collapsed"); api("threads_web", {}).catch((e) => toast(String(e))); return; }
        if (a === "token") {
          b.disabled = true; b.innerHTML = '<span class="spin"></span>';
          tv = await api("threads_set_token", { token: $("#thToken").value });
          confetti(20); flashMood("love", 2500); toast("Đã kết nối Threads ✓");
        }
        if (a === "saveapp") {
          await api("threads_save_app", { appId: $("#thApp").value, secret: $("#thSecret").value, redirect: $("#thRedirect").value });
          tv = await api("threads_get", { refresh: false });
          toast("Đã lưu (App Secret nằm trong Keychain)");
        }
        if (a === "login") {
          await api("threads_save_app", { appId: $("#thApp").value, secret: $("#thSecret").value, redirect: $("#thRedirect").value });
          await api("threads_login");
          toast("Đã mở trình duyệt — bấm Cho phép rồi chép địa chỉ trang hiện ra");
          return;
        }
        if (a === "finish") {
          b.disabled = true; b.innerHTML = '<span class="spin"></span>';
          tv = await api("threads_finish", { pasted: $("#thPaste").value });
          confetti(20); flashMood("love", 2500); toast("Đã kết nối Threads ✓");
        }
        if (a === "logout") { await api("threads_logout"); tv = await api("threads_get", { refresh: false }); }
        if (a === "refresh") return load(true);
        if (a === "read") { await api("threads_mark_read"); tv = await api("threads_get", { refresh: false }); }
        if (a === "replies") {
          openPost = openPost === pid ? null : pid;
          draw();
          if (openPost) { repliesOf[pid] = await api("threads_replies", { id: pid }); tv = await api("threads_get", { refresh: false }); }
        }
        if (a === "reply") { replyTo = { id: b.dataset.rid, name: b.dataset.name }; confirmPost = null; }
        if (a === "noreply") { replyTo = null; confirmPost = null; }
        if (a === "ask") {
          if (!draft.trim()) { toast("Chưa có nội dung"); return; }
          confirmPost = true;
        }
        if (a === "cancel") confirmPost = null;
        if (a === "go") {
          confirmPost = null;
          b.disabled = true; b.innerHTML = '<span class="spin"></span>';
          await api("threads_post", { text: draft, replyTo: replyTo ? replyTo.id : null });
          if (replyTo && openPost) repliesOf[openPost] = await api("threads_replies", { id: openPost });
          draft = ""; replyTo = null;
          tv = await api("threads_get", { refresh: false });
          confetti(24); flashMood("happy", 2000); toast("Đã đăng lên Threads ✓");
        }
      } catch (err) {
        toast(String(err));
        confirmPost = null;
      }
      draw();
    });
    on("threads", () => { if (A.tab === "threads" && A.mode === "expanded") load(false); });

    // ── Threads ngay trong bảng tai thỏ ──
    function inlineView() {
      $("#view").innerHTML = `
        <div class="th-bar">
          <button class="icon-btn" data-thn="back" title="Quay lại">‹</button>
          <button class="icon-btn" data-thn="home" title="Bảng tin">⌂</button>
          <button class="icon-btn" data-thn="reload" title="Tải lại">${I.sync}</button>
          <span class="muted small grow">Kéo thanh ▬ ở đáy để giãn khung</span>
          <button class="btn sm ghost" data-thn="window" title="Mở thành cửa sổ riêng">${I.ext}Cửa sổ riêng</button>
          <button class="btn sm ghost" data-thn="api" title="Thống kê bài viết & báo trả lời (cần Meta app)">Thống kê</button>
        </div>
        <div id="thSlot" class="th-slot"><div class="empty"><span class="spin"></span><div class="small" style="margin-top:6px">Đang mở Threads…</div></div></div>`;
      // đặt khung sau khi hoạt cảnh mở xong, đặt lại lần nữa cho chắc khi cửa sổ đã đổi cỡ
      setTimeout(() => A.placeEmbed(), 480);
      setTimeout(() => A.placeEmbed(), 1100);
    }
    $("#view").addEventListener("click", async (e) => {
      if (A.tab !== "threads") return;
      const b = e.target.closest("[data-thn]");
      if (!b) return;
      const a = b.dataset.thn;
      if (["back", "home", "reload"].includes(a)) return api("threads_embed_nav", { action: a });
      if (a === "close") { A.hideEmbed(); await api("threads_close"); A.toast("Đã đóng Threads"); return; }
      if (a === "window") { A.hideEmbed(); A.setMode("collapsed"); api("threads_web", {}).catch((e) => toast(String(e))); return; }
      if (a === "api") { window.__thApi = true; A.switchTab("threads"); }
      if (a === "feed") {
        window.__thApi = false;
        if (!A.threadsInline()) { A.setMode("collapsed"); api("threads_web", {}).catch((e) => toast(String(e))); return; } // cửa sổ Threads riêng (mặc định)
        A.switchTab("threads");
      }
    });
    window.addEventListener("resize", () => { if (A.tab === "threads" && A.mode === "expanded" && $("#thSlot")) A.placeEmbed(); });
    A.views.threads = () => {
      if (A.threadsInline()) return inlineView();
      draw();
      load(false);
    };
  }
  if (window.ASI) boot();
  else document.addEventListener("asi-ready", boot, { once: true });
})();
