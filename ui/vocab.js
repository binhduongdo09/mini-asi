/* ───────── Tab Từ mới (mô-đun, bật trong Cài đặt › Tính năng) ─────────
 * Mỗi lần mở bảng: 2–3 từ (Cài đặt). Mỗi từ: phiên âm US/UK, nghĩa tiếng Việt, ví dụ (tra online, có cache),
 * nghe mẫu, đọc thử để Asi chấm phát âm, nghe lại giọng mình. Đọc chuẩn 2 lần → tự đánh dấu đã thuộc.
 * Backend: vocab_batch / vocab_play / vocab_check / vocab_learned / vocab_play_mine (src-tauri/src/vocab.rs).
 */
(function () {
  "use strict";
  function boot() {
    const A = window.ASI;
    const { api, on, $, esc, I, toast, confetti, flashMood } = A;
    const view = $("#view");
    let vb = null; // { cards, learned, total }
    const results = {}; // từ → kết quả chấm gần nhất

    const load = async (fresh) => (vb = await api("vocab_batch", { fresh: !!fresh }));
    const ipa = (d) => [d.ipa_us && `<span><i>US</i>${esc(d.ipa_us)}</span>`, d.ipa_uk && d.ipa_uk !== d.ipa_us && `<span><i>UK</i>${esc(d.ipa_uk)}</span>`].filter(Boolean).join("");

    function cardHtml(c) {
      const d = c.details || {}, r = results[c.word];
      return `<div class="w-card ${c.progress.learned ? "learned" : ""}" data-w="${esc(c.word)}">
        <div class="w-top">
          <div class="grow"><span class="w-word">${esc(c.word)}</span><span class="dim"> ${esc(c.pos)}</span>
            <div class="w-ipa">${ipa(d) || '<span class="dim">đang tra phiên âm…</span>'}</div></div>
          <button class="ib txt" data-wa="us" title="Nghe giọng Mỹ">${I.play}US</button>
          <button class="ib txt" data-wa="uk" title="Nghe giọng Anh">${I.play}UK</button>
        </div>
        <div class="w-vi">${esc(c.vi)}</div>
        ${d.example ? `<div class="w-ex">“${esc(d.example)}”</div>` : d.definition ? `<div class="w-ex">${esc(d.definition)}</div>` : ""}
        <div class="w-acts">
          <button class="btn sm" data-wa="rec">${I.mic}Đọc thử</button>
          ${r ? `<button class="link" data-wa="mine">Nghe lại giọng mình</button>` : ""}
          <span class="grow"></span>
          <button class="link ${c.progress.learned ? "ok" : ""}" data-wa="learned">${c.progress.learned ? "Đã thuộc ✓" : "Đánh dấu đã thuộc"}</button>
        </div>
        ${r ? `<div class="w-res ${r.verdict}"><b>${r.score}%</b> ${esc(r.tip)}</div>` : ""}
      </div>`;
    }
    async function render(fresh) {
      if (!vb || fresh) await load(fresh);
      view.innerHTML = `<div class="sec">Từ mới hôm nay<span>${vb.learned}/${vb.total} đã thuộc</span><span class="grow"></span><button class="link" data-wa="more">Đổi từ khác</button></div>
        <div class="w-list">${vb.cards.map(cardHtml).join("")}</div>
        <div class="foot-note">Nghe mẫu rồi bấm Đọc thử — đọc chuẩn 2 lần là tự đánh dấu đã thuộc. Phiên âm & ví dụ: Free Dictionary API.</div>`;
    }
    function redrawCard(word) {
      const el = view.querySelector(`.w-card[data-w="${CSS.escape(word)}"]`);
      const c = vb && vb.cards.find((x) => x.word === word);
      if (el && c) el.outerHTML = cardHtml(c);
    }
    view.addEventListener("click", async (e) => {
      if (A.tab !== "words") return;
      const b = e.target.closest("[data-wa]");
      if (!b) return;
      const a = b.dataset.wa, card = b.closest(".w-card"), word = card && card.dataset.w;
      if (a === "more") return render(true);
      if (a === "us" || a === "uk") { api("vocab_play", { word, accent: a }).catch((err) => toast(String(err))); return; }
      if (a === "mine") { api("vocab_play_mine").catch((err) => toast(String(err))); return; }
      const c = vb.cards.find((x) => x.word === word);
      if (a === "learned") {
        c.progress.learned = !c.progress.learned;
        await api("vocab_learned", { word, learned: c.progress.learned });
        vb.learned += c.progress.learned ? 1 : -1;
        if (c.progress.learned) confetti(10);
        return render(false);
      }
      if (a === "rec") {
        b.innerHTML = `${I.mic}Đang nghe… bấm để xong`;
        A.dictate(b, async (heard) => {
          const r = await api("vocab_check", { word, heard });
          results[word] = r;
          if (r.verdict === "great") {
            c.progress.correct += 1;
            if (c.progress.correct >= 2 && !c.progress.learned) { c.progress.learned = true; vb.learned += 1; confetti(14); }
            flashMood("happy", 1500);
          } else flashMood(r.verdict === "close" ? "idle" : "worried", 1200);
          redrawCard(word);
        }, { lang: "en-US", keep: true });
      }
    });
    // vừa tra xong phiên âm/ví dụ online → vẽ lại đúng thẻ đó
    on("vocab-updated", (word) => api("vocab_batch", { fresh: false }).then((nv) => { vb = nv; if (A.tab === "words" && A.mode === "expanded") redrawCard(word); }));
    A.views.words = () => render(A.takeFreshOpen());
  }
  if (window.ASI) boot();
  else document.addEventListener("asi-ready", boot, { once: true });
})();
