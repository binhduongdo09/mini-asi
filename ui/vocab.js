/* ───────── Học từ mới tiếng Anh ─────────
 * Mỗi lần mở tai thỏ: 2–3 từ (cấu hình trong Cài đặt). Mỗi từ có phiên âm US/UK, nghĩa tiếng Việt,
 * định nghĩa + ví dụ (tra online, có cache), nút nghe mẫu, ghi âm để Asi chấm phát âm và nghe lại giọng mình.
 * Backend: vocab_batch / vocab_play / vocab_check / vocab_learned / vocab_play_mine (src-tauri/src/vocab.rs).
 */
(function () {
  "use strict";
  function boot() {
    const A = window.ASI;
    const { api, on, $, esc, I, toast, confetti, flashMood } = A;
    let vb = null; // { cards, learned, total }
    const results = {}; // từ → kết quả chấm gần nhất

    async function load(fresh) {
      vb = await api("vocab_batch", { fresh: !!fresh });
      return vb;
    }

    const ipa = (d) => [d.ipa_us && `🇺🇸 ${esc(d.ipa_us)}`, d.ipa_uk && d.ipa_uk !== d.ipa_us && `🇬🇧 ${esc(d.ipa_uk)}`].filter(Boolean).join("  ·  ");

    function cardHtml(c) {
      const d = c.details || {}, r = results[c.word];
      return `<div class="w-card ${c.progress.learned ? "learned" : ""}" data-w="${esc(c.word)}">
        <div class="row"><div class="grow"><span class="w-word">${esc(c.word)}</span> <span class="muted small">${esc(c.pos)}</span>
          <div class="w-ipa">${ipa(d) || '<span class="muted small">đang tra phiên âm…</span>'}</div></div>
          <button class="icon-btn" data-wa="us" title="Nghe giọng Mỹ">${I.play}<span class="small">US</span></button>
          <button class="icon-btn" data-wa="uk" title="Nghe giọng Anh">${I.play}<span class="small">UK</span></button></div>
        <div class="w-vi">${esc(c.vi)}</div>
        ${d.definition ? `<div class="muted small">${esc(d.definition)}</div>` : ""}
        ${d.example ? `<div class="w-ex">“${esc(d.example)}”</div>` : ""}
        <div class="row w-acts">
          <button class="btn sm" data-wa="rec">${I.mic}Đọc thử</button>
          ${r ? `<button class="btn sm ghost" data-wa="mine">Nghe lại giọng mình</button>` : ""}
          <span class="grow"></span>
          <button class="btn sm ghost" data-wa="learned">${c.progress.learned ? "✓ Đã thuộc" : "Đánh dấu đã thuộc"}</button>
        </div>
        ${r ? `<div class="w-res ${r.verdict}"><b>${r.score}%</b> ${esc(r.tip)}</div>` : ""}
      </div>`;
    }

    async function view(fresh) {
      const v = $("#view");
      if (!vb || fresh) await load(fresh);
      v.innerHTML = `<div class="row" style="margin-bottom:8px"><b class="grow">${I.words} Từ mới hôm nay</b>
          <span class="chip amber">✓ ${vb.learned}/${vb.total} từ đã thuộc</span>
          <button class="btn sm ghost" data-wa="more">Đổi từ khác</button></div>
        <div class="w-list">${vb.cards.map(cardHtml).join("")}</div>
        <div class="muted small" style="margin-top:6px">Bấm ▶ để nghe mẫu, rồi 🎙 Đọc thử — đọc chuẩn 2 lần là Asi tự đánh dấu đã thuộc. Phát âm & ví dụ lấy từ Free Dictionary API.</div>`;
    }

    // Dải từ mới ở trang chủ (mỗi lần mở tai thỏ đổi từ)
    async function strip(el, fresh) {
      if (!el) return;
      try { await load(fresh); } catch (_) { return; }
      el.innerHTML = `<div class="w-strip"><span class="muted small">${I.words} Từ mới:</span>${vb.cards
        .map((c) => `<button class="w-chip" data-wopen="${esc(c.word)}" title="${esc(c.vi)}"><b>${esc(c.word)}</b> <span class="muted">${esc((c.details && (c.details.ipa_us || c.details.ipa_uk)) || c.vi)}</span></button>`)
        .join("")}</div>`;
    }

    function redrawCard(word) {
      const el = document.querySelector(`.w-card[data-w="${CSS.escape(word)}"]`);
      const c = vb && vb.cards.find((x) => x.word === word);
      if (el && c) el.outerHTML = cardHtml(c);
    }

    $("#view").addEventListener("click", async (e) => {
      const open = e.target.closest("[data-wopen]");
      if (open) { A.switchTab("words"); return; }
      if (A.tab !== "words") return;
      const b = e.target.closest("[data-wa]");
      if (!b) return;
      const a = b.dataset.wa, card = b.closest(".w-card"), word = card && card.dataset.w;
      if (a === "more") { await load(true); return view(false); }
      if (a === "us" || a === "uk") { api("vocab_play", { word, accent: a }).catch((err) => toast(String(err))); return; }
      if (a === "mine") { api("vocab_play_mine").catch((err) => toast(String(err))); return; }
      if (a === "learned") {
        const c = vb.cards.find((x) => x.word === word);
        c.progress.learned = !c.progress.learned;
        await api("vocab_learned", { word, learned: c.progress.learned });
        vb.learned += c.progress.learned ? 1 : -1;
        if (c.progress.learned) confetti(10);
        return view(false);
      }
      if (a === "rec") {
        b.classList.add("rec");
        b.innerHTML = `${I.mic}Đang nghe… bấm để xong`;
        A.dictate(b, async (heard) => {
          const r = await api("vocab_check", { word, heard });
          results[word] = r;
          const c = vb.cards.find((x) => x.word === word);
          if (r.verdict === "great") { c.progress.correct += 1; if (c.progress.correct >= 2 && !c.progress.learned) { c.progress.learned = true; vb.learned += 1; confetti(14); } flashMood("happy", 1500); }
          else flashMood(r.verdict === "close" ? "idle" : "worried", 1200);
          redrawCard(word);
        }, { lang: "en-US", keep: true });
      }
    });

    on("vocab-updated", (word) => {
      // vừa tra xong phiên âm/ví dụ online → vẽ lại đúng thẻ đó
      api("vocab_batch", { fresh: false }).then((nv) => {
        vb = nv;
        if (A.tab === "words" && A.mode === "expanded") redrawCard(word);
        if (A.tab === "home" && A.mode === "expanded") strip($("#vocabStrip"), false);
      });
    });

    A.views.words = () => view(A.takeFreshOpen());
    window.VocabUI = { strip };
  }
  if (window.ASI) boot();
  else document.addEventListener("asi-ready", boot, { once: true });
})();
