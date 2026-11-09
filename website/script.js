/**
 * Mini-Asi Landing Page Interactive Script
 */

document.addEventListener("DOMContentLoaded", () => {
  // Simulator Elements
  const notch = document.getElementById("notchIsland");
  const tabButtons = document.querySelectorAll(".tab-btn");
  const views = document.querySelectorAll(".notch-view");
  const recTimer = document.getElementById("recTimer");

  // Tab Switching Logic
  tabButtons.forEach((btn) => {
    btn.addEventListener("click", () => {
      const tabId = btn.getAttribute("data-tab");

      // Update button styles
      tabButtons.forEach((b) => b.classList.remove("active"));
      btn.classList.add("active");

      // Update notch dimensions by state class
      notch.className = `notch-island state-${tabId}`;

      // Update active view
      views.forEach((view) => {
        if (view.classList.contains(`view-${tabId}`)) {
          view.classList.add("active");
        } else {
          view.classList.remove("active");
        }
      });
    });
  });

  // Meeting timer simulation
  let seconds = 278; // 04:38
  setInterval(() => {
    seconds++;
    const m = Math.floor(seconds / 60);
    const s = seconds % 60;
    if (recTimer) {
      recTimer.textContent = `${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
    }
  }, 1000);

  // Smooth anchor scrolling
  document.querySelectorAll('a[href^="#"]').forEach((anchor) => {
    anchor.addEventListener("click", function (e) {
      const targetId = this.getAttribute("href");
      if (targetId && targetId !== "#") {
        const target = document.querySelector(targetId);
        if (target) {
          e.preventDefault();
          target.scrollIntoView({ behavior: "smooth", block: "start" });
        }
      }
    });
  });

  // Fetch latest release tag if accessible
  const REPO = "binhduongdo09/mini-asi";
  fetch(`https://api.github.com/repos/${REPO}/releases/latest`)
    .then((res) => {
      if (res.ok) return res.json();
      throw new Error("Release not public or accessible");
    })
    .then((data) => {
      const verText = data.name || data.tag_name;
      const osVersion = document.querySelector(".os-version");
      if (osVersion && verText) {
        osVersion.textContent = `${verText} • ~7.8 MB`;
      }
    })
    .catch(() => {
      // Fallback already in HTML
    });
});
