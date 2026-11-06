/* ───────── Bộ gõ tiếng Việt của Mini-Asi (Telex / VNI) ─────────
 * Dùng khi bộ gõ tiếng Việt của macOS không ghép chữ được trong cửa sổ tai thỏ.
 * Mini-Asi tạm chuyển bàn phím hệ thống sang ABC khi mở bảng, tự gõ dấu ở đây,
 * rồi trả lại bộ gõ cũ khi đóng bảng.
 */
(function (root) {
  "use strict";
  const ROWS = {
    a: "aáàảãạ", "a^": "âấầẩẫậ", "a(": "ăắằẳẵặ", e: "eéèẻẽẹ", "e^": "êếềểễệ", i: "iíìỉĩị",
    o: "oóòỏõọ", "o^": "ôốồổỗộ", "o+": "ơớờởỡợ", u: "uúùủũụ", "u+": "ưứừửữự", y: "yýỳỷỹỵ",
  };
  const INFO = {}; // ký tự → {base, mark, tone, up}
  for (const [k, row] of Object.entries(ROWS)) {
    [...row].forEach((ch, tone) => {
      const v = { base: k[0], mark: k[1] || "", tone };
      INFO[ch] = { ...v, up: false };
      INFO[ch.toUpperCase()] = { ...v, up: true };
    });
  }
  const compose = (base, mark, tone, up) => {
    const c = ROWS[base + mark][tone];
    return up ? c.toUpperCase() : c;
  };
  const isVowel = (ch) => !!INFO[ch];
  const FINALS = ["", "c", "ch", "m", "n", "ng", "nh", "p", "t"];

  // Tách từ: phụ âm đầu | cụm nguyên âm | phụ âm cuối
  function split(chars) {
    const low = chars.map((c) => (INFO[c] ? INFO[c].base : c.toLowerCase()));
    let s = 0;
    while (s < chars.length && !isVowel(chars[s])) s++;
    // "qu", "gi" → u / i thuộc phụ âm đầu
    if (s < chars.length && ((low[s] === "u" && s > 0 && low[s - 1] === "q") || (low[s] === "i" && s > 0 && low[s - 1] === "g" && s + 1 < chars.length && isVowel(chars[s + 1])))) s++;
    let e = s;
    while (e < chars.length && isVowel(chars[e])) e++;
    const tail = low.slice(e).join("");
    return { s, e, tail, valid: s < e && FINALS.includes(tail) && chars.slice(e).every((c) => !isVowel(c)) };
  }

  function currentTone(chars, sp) {
    for (let i = sp.s; i < sp.e; i++) if (INFO[chars[i]].tone) return INFO[chars[i]].tone;
    return 0;
  }

  function tonePos(chars, sp) {
    const idx = [];
    for (let i = sp.s; i < sp.e; i++) idx.push(i);
    const marked = idx.filter((i) => INFO[chars[i]].mark);
    if (marked.length) return marked[marked.length - 1]; // ươ → ơ
    if (idx.length === 1) return idx[0];
    if (idx.length === 2) return sp.tail ? idx[1] : idx[0];
    return idx[1];
  }

  function setTone(chars, tone) {
    const sp = split(chars);
    if (sp.s >= sp.e) return chars;
    const out = chars.map((c) => (INFO[c] ? compose(INFO[c].base, INFO[c].mark, 0, INFO[c].up) : c));
    if (tone) {
      const p = tonePos(out, sp);
      const v = INFO[out[p]];
      out[p] = compose(v.base, v.mark, tone, v.up);
    }
    return out;
  }

  function setMark(chars, i, mark) {
    const v = INFO[chars[i]];
    chars[i] = compose(v.base, mark, v.tone, v.up);
  }

  const TELEX_TONE = { s: 1, f: 2, r: 3, x: 4, j: 5, z: 0 };
  const VNI_TONE = { 1: 1, 2: 2, 3: 3, 4: 4, 5: 5, 0: 0 };

  /** Gõ phím `key` sau từ `word`. Trả về chuỗi mới cho từ (có thể chỉ là word + key). */
  function apply(word, key, mode) {
    const chars = [...word];
    const k = key.toLowerCase();
    const plain = word + key;
    if (!chars.length) return k === "w" && mode === "telex" ? (key === "W" ? "Ư" : "ư") : plain;
    const sp = split(chars);
    const tone = currentTone(chars, sp);
    const tones = mode === "vni" ? VNI_TONE : TELEX_TONE;

    // 1. Dấu thanh
    if (k in tones && sp.valid) {
      const t = tones[k];
      if (t === 0) return tone ? setTone(chars, 0).join("") : plain;
      if (tone === t) return setTone(chars, 0).join("") + key; // gõ lặp → bỏ dấu, giữ phím
      return setTone(chars, t).join("");
    }
    // 2. đ
    if ((mode === "telex" && k === "d") || (mode === "vni" && k === "9")) {
      const f = chars[0];
      if (f === "d" || f === "D") { chars[0] = f === "d" ? "đ" : "Đ"; return chars.join(""); }
      if (f === "đ" || f === "Đ") { chars[0] = f === "đ" ? "d" : "D"; return chars.join("") + key; }
      return plain;
    }
    if (sp.s >= sp.e) {
      return mode === "telex" && k === "w" && sp.tail.length <= 2 ? word + (key === "W" ? "Ư" : "ư") : plain;
    }
    const low = (i) => INFO[chars[i]].base;
    const mk = (i) => INFO[chars[i]].mark;
    const cl = [];
    for (let i = sp.s; i < sp.e; i++) cl.push(i);
    const clStr = cl.map(low).join("");

    // 3. Mũ ^ : aa ee oo (Telex) hoặc 6 (VNI)
    const circ = (mode === "telex" && ["a", "e", "o"].includes(k)) || (mode === "vni" && k === "6");
    if (circ) {
      const target = [...cl].reverse().find((i) => (mode === "vni" ? ["a", "e", "o"].includes(low(i)) : low(i) === k));
      if (target === undefined) return plain;
      if (mk(target) === "^") { setMark(chars, target, ""); return chars.join("") + key; }
      setMark(chars, target, "^");
      return setTone(chars, tone).join("");
    }
    // 4. Móc / trăng: w (Telex), 7 = ơ ư, 8 = ă (VNI)
    const horn = (mode === "telex" && k === "w") || (mode === "vni" && (k === "7" || k === "8"));
    if (horn) {
      let targets = [];
      const uo = clStr.indexOf("uo");
      if (mode === "vni" && k === "8") {
        const a = [...cl].reverse().find((i) => low(i) === "a");
        if (a !== undefined) targets = [[a, "("]];
      } else if (uo >= 0 && (mode === "telex" || k === "7")) {
        targets = [[cl[uo], "+"], [cl[uo + 1], "+"]];
      } else if (mode === "telex" && clStr.startsWith("ua") && sp.s > 0) {
        targets = [[cl[0], "+"]]; // mưa
      } else if (mode === "telex" && clStr.includes("oa")) {
        targets = [[cl[clStr.indexOf("oa") + 1], "("]]; // hoặc
      } else {
        const order = mode === "vni" ? ["o", "u"] : ["a", "o", "u"];
        const t = [...cl].reverse().find((i) => order.includes(low(i)));
        if (t !== undefined) targets = [[t, low(t) === "a" ? "(" : "+"]];
      }
      if (!targets.length) return plain;
      if (targets.every(([i, m]) => mk(i) === m)) {
        targets.forEach(([i]) => setMark(chars, i, ""));
        return chars.join("") + key; // gõ lặp → trả lại chữ thường
      }
      targets.forEach(([i, m]) => setMark(chars, i, m));
      return setTone(chars, tone).join("");
    }
    // 5. Chữ thường: đặt lại vị trí dấu thanh (hòa + n → hoàn)
    if (tone && /^[a-z]$/i.test(key)) {
      const next = [...chars, key];
      const sp2 = split(next);
      if (sp2.valid) return setTone(next, tone).join("");
    }
    return plain;
  }

  // ───────── Gắn vào ô nhập ─────────
  const WORD = /[A-Za-zÀ-ỹĐđ]+$/;
  const engine = {
    mode: null, // "telex" | "vni" | null (tắt)
    suspended: false,
    apply,
    attach(doc) {
      doc.addEventListener("compositionstart", () => { if (engine.mode) engine.suspended = true; }, true);
      doc.addEventListener("beforeinput", (e) => {
        if (!engine.mode || engine.suspended || e.isComposing) return;
        if (e.inputType !== "insertText" || !e.data || e.data.length !== 1) return;
        const el = e.target;
        if (el && el.isContentEditable) return editable(e);
        if (!el || !(el.tagName === "INPUT" || el.tagName === "TEXTAREA") || el.type === "password" || el.type === "number") return;
        const start = el.selectionStart, end = el.selectionEnd;
        if (start == null || start !== end) return;
        const before = el.value.slice(0, start);
        const m = before.match(WORD);
        const word = m ? m[0] : "";
        const out = apply(word, e.data, engine.mode);
        if (out === word + e.data) return; // để trình duyệt gõ bình thường
        e.preventDefault();
        const head = before.slice(0, before.length - word.length);
        el.value = head + out + el.value.slice(end);
        const caret = head.length + out.length;
        el.setSelectionRange(caret, caret);
        el.dispatchEvent(new Event("input", { bubbles: true }));
      }, true);
    },
  };
  // Ô soạn thảo dạng contenteditable (khung viết bài của Threads, Notes web…)
  function editable(e) {
    const sel = document.getSelection();
    if (!sel || !sel.rangeCount || !sel.isCollapsed) return;
    const r = sel.getRangeAt(0);
    const node = r.startContainer;
    let word = "", before = "";
    if (node.nodeType === 3) {
      before = node.data.slice(0, r.startOffset);
      const m = before.match(WORD);
      word = m ? m[0] : "";
    }
    const out = apply(word, e.data, engine.mode);
    if (out === word + e.data) return;
    e.preventDefault();
    if (word) {
      const rr = document.createRange();
      rr.setStart(node, r.startOffset - word.length);
      rr.setEnd(node, r.startOffset);
      sel.removeAllRanges();
      sel.addRange(rr);
    }
    document.execCommand("insertText", false, out);
  }
  root.VNKey = engine;
  if (typeof document !== "undefined") engine.attach(document);
  if (typeof module !== "undefined") module.exports = engine;
})(typeof window !== "undefined" ? window : globalThis);
