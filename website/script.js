/* Mini-Asi, trang giới thiệu: đổi ảnh sản phẩm theo tab đang chọn (bàn phím: mũi tên lên / xuống). */
(() => {
  const tabs = [...document.querySelectorAll(".tab")];
  const pane = document.getElementById("pane");
  const img = document.getElementById("paneImg");
  if (!tabs.length || !pane || !img) return;

  const shots = {
    asi: ["assets/screens/asi.png", "Tab Asi trả lời giá cổ phiếu FPT từ DNSE Entrade"],
    tasks: ["assets/screens/tasks.png", "Tab Việc: lịch hôm nay, việc trễ hạn, việc hôm nay"],
    capture: ["assets/screens/capture.png", "Bảng tạo sự kiện lịch từ ảnh vé máy bay vừa thả vào tai thỏ"],
    clips: ["assets/screens/clips.png", "Tab Clipboard với các mục đã ghim"],
  };
  // tải trước để đổi tab không nháy
  Object.values(shots).forEach(([src]) => { const i = new Image(); i.src = src; });

  const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
  const select = (tab) => {
    tabs.forEach((t) => t.setAttribute("aria-selected", String(t === tab)));
    const [src, alt] = shots[tab.dataset.shot];
    if (reduce) { img.src = src; img.alt = alt; return; }
    pane.classList.add("is-swapping");
    setTimeout(() => { img.src = src; img.alt = alt; pane.classList.remove("is-swapping"); }, 180);
  };

  tabs.forEach((tab, i) => {
    tab.addEventListener("click", () => select(tab));
    tab.addEventListener("keydown", (e) => {
      if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
      e.preventDefault();
      const next = tabs[(i + (e.key === "ArrowDown" ? 1 : -1) + tabs.length) % tabs.length];
      next.focus();
      select(next);
    });
  });
})();
