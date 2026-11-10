// Bản giả lập backend — CHỈ dùng khi mở ui/index.html bằng trình duyệt để xem thử giao diện.
// App thật (Tauri) không nạp file này.
(function () {
  const H = 3600e3, now = Date.now();
  const q = new URLSearchParams(location.search);
  const listeners = {};
  const emit = (ev, payload) => (listeners[ev] || []).forEach((cb) => cb({ payload }));
  let id = 100, lastDrop = "text";
  const tasks = [
    { id: 1, title: "Gửi báo giá cho anh Tuấn", due: now - 5 * H, created: now - 9 * H, done_at: null },
    { id: 2, title: "Duyệt banner chiến dịch Q4", due: now - 0.6 * H, created: now - 3 * H, done_at: null },
    { id: 3, title: "Họp team marketing", due: now + 0.4 * H, created: now, done_at: null },
    { id: 4, title: "Gọi chị Mai chốt lịch chụp", due: now + 20 * H, created: now, done_at: null },
    { id: 5, title: "Nộp báo cáo tuần", due: now + 50 * H, created: now, done_at: null },
    { id: 6, title: "Mua quà sinh nhật mẹ", due: null, created: now, done_at: null },
    { id: 7, title: "Đặt vé máy bay Đà Nẵng", due: now - 30 * H, created: now - 40 * H, done_at: now - 2 * H },
  ];
  if (q.get("late") === "0") tasks.forEach((t) => { if (t.due && t.due < now) t.done_at = now; });
  if (q.get("late") === "3") for (let i = 0; i < 4; i++) tasks.push({ id: 50 + i, title: "Việc tồn " + (i + 1), due: now - (26 + i) * H, created: 0, done_at: null });
  const notes = [
    { id: 11, text: "Ý tưởng video TikTok\n- AI viết caption\n- So sánh 3 app ghi chú", updated: now - 0.3 * H, pinned: true },
    { id: 12, text: "Wifi homestay: TronGlamping2026", updated: now - 26 * H, pinned: false },
    { id: 9001, text: "# Họp agency Q4\n09/11/2026 12:47 · 45:10\n\n## Tóm tắt\n- Chốt kế hoạch truyền thông quý 4\n\n## Việc cần làm\n- [ ] Gửi báo giá TikTok Ads — anh Tuấn (hạn: thứ 6)\nNghe lại ghi âm & toàn văn: Mini-Asi › Họp", updated: now - 2 * H, pinned: false, folder: "meetings", meeting: "m1" },
  ];
  const settings = { break_enabled: true, break_minutes: 50, water_enabled: true, water_minutes: 60, sleep_hour: 23, focus_mode: false, launch_at_login: true, license_key: null, pdf_quality: "ebook", shortcut: "CmdOrCtrl+Shift+Space",
    clip_enabled: true, clip_max: 200, clip_shortcut: "Control+Shift+V", clip_autopaste: false, sync_reminders: true, reminders_list: "Mini-Asi", sync_notes: true, notes_folder: "Mini-Asi",
    show_calendar: true, event_alert_minutes: 10, voice_lang: "vi-VN", journal_enabled: true, journal_hour: 21,
    display_mode: q.get("disp") || "notch", vocab_enabled: true, vocab_count: 3, wing_size: q.get("wing") || "s", notch_w: 0, notch_h: 0, icon_size: q.get("icons") || "large", idle_items: q.get("idle") ? q.get("idle").split(",") : ["focus", "overdue", "event", "next", "tasks", "mic"], bubble_x: 0, bubble_y: 0, threads_embed_beta: false, viet_engine: "auto", modules: q.get("mods") ? q.get("mods").split(",").filter(Boolean) : ["threads", "clean", "focus"], apple_ai_enabled: true, ai_provider: q.get("brain") || "apple", ai_model_openai: "", ai_model_gemini: "", ai_model_claude: "", ai_cloud_context: true, auto_update_check: true, update_repo: "binhduongdo09/mini-asi", voice_reply: false, tts_engine: "nghi", tts_voice: "Mai Phương", meeting_ai: "apple", meeting_auto_summary: true, meeting_system_audio: true, meeting_live: "auto", weather_place: "", ui_lang: q.get("lang") || "vi", peek_overdue: true, hover_mic: true };
  const today0 = new Date(); today0.setHours(0, 0, 0, 0);
  const at = (h, m = 0) => today0.getTime() + h * H + m * 60e3;
  const events = [
    { id: "e1", title: "Standup team marketing", start: at(9, 30), end: at(9, 45), all_day: false, location: "Google Meet", calendar: "Công việc" },
    { id: "e2", title: "Họp khách hàng Hoiana", start: now + 0.2 * H, end: now + 1.2 * H, all_day: false, location: "Phòng họp 2", calendar: "Công việc" },
    { id: "e3", title: "Sinh nhật Lan", start: at(0), end: at(24), all_day: true, location: "", calendar: "Gia đình" },
  ];
  const clips = [
    { id: 201, kind: "text", preview: "https://tronglamping.vn/khuyen-mai-mua-thu — Ưu đãi 20% cho nhóm từ 4 người, áp dụng tới 30/11.", chars: 96, files: [], bytes: 96, created: now - 120e3, used: now - 120e3, pinned: false, uses: 2 },
    { id: 202, kind: "files", preview: "", chars: 0, files: [{ name: "Hop-dong-Hoiana.pdf", path: "/Users/binh/Desktop/Hop-dong-Hoiana.pdf", exists: true }, { name: "Bao-gia-Q4.xlsx", path: "/Users/binh/Desktop/Bao-gia-Q4.xlsx", exists: true }], bytes: 3.1e6, created: now - 0.3 * H, used: now - 0.3 * H, pinned: true, uses: 5 },
    { id: 203, kind: "image", preview: "", chars: 0, files: [], bytes: 412e3, created: now - 0.8 * H, used: now - 0.8 * H, pinned: false, uses: 0 },
    { id: 204, kind: "text", preview: "Số tài khoản công ty: liên hệ kế toán chị Hương 0912 xxx xxx", chars: 58, files: [], bytes: 58, created: now - 2 * H, used: now - 2 * H, pinned: true, uses: 9 },
    { id: 205, kind: "text", preview: "Chào anh, em gửi lại báo giá gói sự kiện D'Hoi mùa 2 như đã trao đổi. Anh xem giúp em nhé, có gì em chỉnh ngay ạ.", chars: 118, files: [], bytes: 118, created: now - 5 * H, used: now - 5 * H, pinned: false, uses: 1 },
  ];
  const journal = {};
  let pro = false, focusUntil = null, session = Number(q.get("session") || 34);
  const disk = { total_bytes: 245e9, free_bytes: Number(q.get("free") || 21.4e9), free_percent: 0, level: "ok" };
  disk.free_percent = (disk.free_bytes / disk.total_bytes) * 100;
  disk.level = disk.free_percent < 5 ? "critical" : disk.free_percent < 15 ? "warn" : "ok";

  function lateness() {
    const t = Date.now();
    const od = tasks.filter((x) => !x.done_at && x.due && x.due < t);
    const worst = Math.max(0, ...od.map((x) => (t - x.due) / H));
    const score = od.reduce((s, x) => s + 1 + Math.min((t - x.due) / H / 12, 2), 0);
    const level = !od.length ? 0 : od.length >= 5 || worst >= 24 ? 3 : od.length >= 3 || worst >= 4 ? 2 : 1;
    return { overdue: od.length, level, heat: Math.min(score / 8, 1) };
  }
  function status() {
    const t = Date.now();
    const open = tasks.filter((x) => !x.done_at);
    const next = open.filter((x) => x.due && x.due >= t).sort((a, b) => a.due - b.due)[0];
    return { lateness: lateness(), next: next ? { id: next.id, title: next.title, due: next.due } : null, open_count: open.length, note_count: notes.length, clip_count: clips.length, disk, session_minutes: session, today_minutes: 312, break_minutes: settings.break_minutes, focus_until: focusUntil, sleepy: q.get("sleepy") === "1", focus_mode: settings.focus_mode, pro, now: t, threads_unread: q.get("threads") === "1" ? 4 : 0,
      next_event: events.filter((e) => !e.all_day && e.end > t).sort((a, b) => a.start - b.start)[0] || null };
  }
  function parse(text) {
    const m = text.match(/(\d{1,2})h(\d{2})?/);
    const d = new Date();
    if (/mai/.test(text)) d.setDate(d.getDate() + 1);
    if (m) { d.setHours(+m[1], +(m[2] || 0), 0, 0); return { title: text.replace(/mai|\d{1,2}h(\d{2})?/g, "").trim(), due: d.getTime() }; }
    return { title: text, due: null };
  }
  const rule = (id, name, level, tier, action, items) => ({ id, name, category: "", level, tier, action, locked: tier === "pro" && !pro, bytes: items.reduce((s, i) => s + i.bytes, 0), items });
  const it = (p, gb, age = 12) => ({ rule_id: "", path: "/Users/binh/" + p, bytes: gb * 1e9, action: "delete", age_days: age });
  function report() {
    const rules = [
      rule("browser-chrome", "Cache Google Chrome", "safe", "free", "delete", [it("Library/Caches/Google/Chrome/Default", 1.84)]),
      rule("browser-coccoc", "Cache Cốc Cốc", "safe", "free", "delete", [it("Library/Caches/Coccoc/Default", 0.62)]),
      rule("dev-xcode-deriveddata", "Xcode DerivedData", "safe", "free", "delete", [it("Library/Developer/Xcode/DerivedData/MiniAsi-abc", 4.1), it("Library/Developer/Xcode/DerivedData/TronApp-x1", 2.3)]),
      rule("dev-package-caches", "Cache npm, yarn, pip, Homebrew, CocoaPods, Go", "safe", "free", "delete", [it(".npm/_cacache", 1.2), it("Library/Caches/Homebrew", 0.9)]),
      rule("dev-project-artifacts", "Thư mục build của dự án lâu không đụng tới", "review", "pro", "delete", [it("Projects/landing-tron/node_modules", 0.74, 40), it("Projects/hoiana-360/node_modules", 1.1, 61)]),
      rule("files-installers", "File cài đặt đã dùng xong (.dmg, .pkg, .iso, .xip)", "review", "pro", "trash", [it("Downloads/Zalo-24.9.dmg", 0.31, 50), it("Downloads/Photoshop_2025.dmg", 3.2, 90)]),
      rule("files-ios-backups", "Bản sao lưu iPhone/iPad cũ trên máy", "review", "pro", "trash", [it("Library/Application Support/MobileSync/Backup/00008110", 18.6, 210)]),
      rule("system-logs", "Nhật ký ứng dụng (> 7 ngày)", "safe", "free", "delete", [it("Library/Logs/Zoom", 0.21, 30)]),
      rule("system-user-caches", "Cache ứng dụng (tự tạo lại khi cần)", "safe", "free", "delete", [it("Library/Caches/com.spotify.client", 2.8, 3), it("Library/Caches/com.tinyspeck.slackmacgap", 0.7, 2), it("Library/Caches/zalo", 1.6, 5)]),
      rule("system-trash", "Thùng rác", "review", "free", "delete", [it(".Trash/old-video.mp4", 1.3, 20)]),
    ];
    const total = rules.reduce((s, r) => s + r.bytes, 0);
    const locked = rules.filter((r) => r.locked).reduce((s, r) => s + r.bytes, 0);
    return { rules, skipped: [{ rule_id: "x", path: "/Users/binh/Library/Caches/com.apple.Safari", reason: "đang được ứng dụng sử dụng" }], total_bytes: total, available_bytes: total - locked, locked_bytes: locked, safe_bytes: 0 };
  }
  const wait = (ms) => new Promise((r) => setTimeout(r, ms));
  const H_ = {
    get_geometry: () => ({ has_notch: q.get("notch") !== "0", notch_width: 185, notch_height: 32, screen_x: 0, screen_y: 0, screen_width: 1512, screen_height: 982, platform: "macos" }),
    set_frame: () => null, set_interactive: ({ on }) => (on ? (settings.viet_engine === "off" ? null : settings.viet_engine === "vni" ? "vni" : "telex") : null),
    get_status: status,
    parse_task: ({ text }) => parse(text),
    add_task: ({ text }) => { const p = parse(text); const t = { id: ++id, title: p.title, due: p.due, created: Date.now(), done_at: null }; tasks.push(t); emit("status", status()); return t; },
    list_tasks: () => tasks.map((t) => ({ ...t })),
    toggle_task: ({ id }) => { const t = tasks.find((x) => x.id === id); t.done_at = t.done_at ? null : Date.now(); emit("status", status()); return { ...t }; },
    snooze_task: ({ id, minutes }) => { const t = tasks.find((x) => x.id === id); t.due = Date.now() + (minutes < 0 ? 18 * H : minutes * 60e3); emit("status", status()); return t; },
    rename_task: ({ id, title }) => { tasks.find((x) => x.id === id).title = title; },
    delete_task: ({ id }) => { tasks.splice(tasks.findIndex((x) => x.id === id), 1); emit("status", status()); },
    clear_done: () => { for (let i = tasks.length - 1; i >= 0; i--) if (tasks[i].done_at) tasks.splice(i, 1); },
    list_notes: () => notes.slice().sort((a, b) => b.pinned - a.pinned || b.updated - a.updated),
    save_note: ({ id: nid, text }) => { let n = notes.find((x) => x.id === nid); if (!n) { n = { id: ++id, text, updated: Date.now(), pinned: false }; notes.push(n); } n.text = text; n.updated = Date.now(); return n; },
    pin_note: ({ id }) => { const n = notes.find((x) => x.id === id); n.pinned = !n.pinned; },
    delete_note: ({ id }) => notes.splice(notes.findIndex((x) => x.id === id), 1),
    file_info: ({ paths }) => paths.map((p) => ({ path: p, name: p.split("/").pop(), size: 4.2e6, is_dir: !p.includes("."), is_pdf: p.endsWith(".pdf"), is_archive: /\.(zip|rar|7z)$/.test(p), is_image: /\.(png|jpe?g|heic|tiff?|gif|webp)$/i.test(p), is_text: /\.(txt|md|docx|rtf|html?)$/i.test(p) })),
    file_kinds: ({ paths }) => ({ images: paths.filter((p) => /\.(png|jpe?g|heic)$/i.test(p)).length + (paths.some((p) => !p.includes(".")) ? 24 : 0), texts: paths.filter((p) => /\.(txt|md|docx)$/i.test(p)).length }),
    file_convert_images: async ({ format, merge }) => { await wait(900); return { output: merge ? "/Users/binh/Desktop/Ảnh chuyến đi - 25 ảnh.pdf" : "/Users/binh/Desktop/Ảnh chuyến đi - " + format.toUpperCase() + "/IMG_0001." + format, before: 84e6, after: merge ? 31e6 : 22e6, message: merge ? "Đã gộp 25 ảnh thành 1 PDF" : "Đã chuyển 25 file sang " + format.toUpperCase() }; },
    file_text_pdf: async () => { await wait(700); return { output: "/Users/binh/Desktop/Biên bản.pdf", before: 4200, after: 690e3, message: "Đã chuyển sang PDF" }; },
    pick_files: () => ["/Users/binh/Desktop/Hop-dong-Hoiana.pdf", "/Users/binh/Desktop/Bao-gia-Q4.pdf", "/Users/binh/Downloads/Anh-su-kien.zip"],
    file_zip: async () => { await wait(700); return { output: "/Users/binh/Desktop/Lưu trữ.zip", before: 12.4e6, after: 8.1e6, message: "Đã nén xong" }; },
    file_extract: async () => { await wait(600); return { output: "/Users/binh/Downloads/Anh-su-kien", before: 4.2e6, after: 4.6e6, message: "Đã giải nén xong" }; },
    file_compress_pdf: async ({ path }) => { await wait(900); return { output: path.replace(".pdf", "-nen.pdf"), before: 8.4e6, after: 1.9e6, message: "Giảm 77% (macOS)" }; },
    file_merge_pdf: async () => { await wait(600); return { output: "/Users/binh/Desktop/Hop-dong-Hoiana-gop.pdf", before: 12e6, after: 11e6, message: "Đã gộp 2 file" }; },
    file_airdrop: async () => { await wait(200); return null; },
    cursor_side: () => "left",
    media_status: () => (q.get("media") === "0" ? { playing: false, title: "", artist: "", app: "", output_volume: 60, output_muted: false, mic_volume: 80, mic_muted: false } : { playing: true, title: "Lo-fi buổi chiều", artist: "Playlist tập trung", app: "Spotify", output_volume: 65, output_muted: false, mic_volume: 80, mic_muted: false, cam_granted: true }),
    media_control: async () => ({ playing: false, title: "Lo-fi buổi chiều", artist: "Playlist tập trung", app: "Spotify", output_volume: 65, output_muted: false, mic_volume: 80, mic_muted: false, cam_granted: true }),
    toggle_mic_mute: async () => ({ playing: true, title: "Lo-fi buổi chiều", artist: "Playlist tập trung", app: "Spotify", output_volume: 65, output_muted: false, mic_volume: 0, mic_muted: true, cam_granted: true }),
    ensure_camera_perm: async () => true,
    clip_copy_image: () => null,
    reveal: () => null,
    clean_scan: async () => { await wait(1200); return { report: report(), disk, pro }; },
    clean_run: async ({ rules, paths }) => { await wait(900); return { dry_run: false, deleted_bytes: 15.95e9, trashed_bytes: 0, items_done: 11, failures: [], locked_rules: [] }; },
    clean_history: () => [], open_disk_settings: () => null,
    health_break_done: () => { session = 0; }, health_snooze: () => null,
    focus_start: ({ minutes }) => { focusUntil = Math.floor(Date.now() / 1000) + minutes * 60; emit("status", status()); }, focus_stop: () => { focusUntil = null; },
    get_settings: () => ({ ...settings }), save_settings: ({ settings: s }) => { Object.assign(settings, s); emit("settings-changed", { ...settings }); emit("status", status()); },
    patch_settings: ({ patch }) => { for (const k in patch) if (k in settings) settings[k] = patch[k]; emit("settings-changed", { ...settings }); emit("status", status()); return { ...settings }; },
    license_info: () => (pro ? { pro: true, email: "binh@example.vn", expires: null } : { pro: false }),
    activate_license: ({ key }) => (key.startsWith("MASI1.") ? ((pro = true), { pro: true, email: "binh@example.vn" }) : { pro: false, error: "Khoá không đúng định dạng" }),
    deactivate_license: () => { pro = false; }, quit: () => null,
    clip_list: ({ query, kind }) => clips.filter((c) => (kind === "all" || !kind || c.kind === kind || (kind === "pinned" && c.pinned) || (kind === "history" && !c.pinned)) && (!query || ((c.label || "") + c.preview + c.files.map((f) => f.name).join()).toLowerCase().includes(query.toLowerCase()))),
    clip_label: ({ id, label }) => { const c = clips.find((x) => x.id === id); c.label = label.trim() || null; c.pinned = true; },
    clip_copy: ({ id }) => { const c = clips.find((x) => x.id === id); c.uses++; c.used = Date.now(); return false; },
    clip_merge: () => null, clip_copy_files: () => null,
    clip_pin: ({ id }) => { const c = clips.find((x) => x.id === id); c.pinned = !c.pinned; if (!c.pinned) c.label = null; },
    clip_delete: ({ id }) => clips.splice(clips.findIndex((x) => x.id === id), 1), clip_clear: () => null,
    clip_image: () => "data:image/svg+xml;base64," + btoa('<svg xmlns="http://www.w3.org/2000/svg" width="240" height="120"><defs><linearGradient id="g"><stop offset="0" stop-color="#ff9a8b"/><stop offset="1" stop-color="#6a82fb"/></linearGradient></defs><rect width="240" height="120" rx="10" fill="url(#g)"/><text x="20" y="70" font-family="sans-serif" font-size="22" fill="white">Screenshot</text></svg>'),
    clip_to_note: () => notes[0], clip_to_task: () => tasks[2],
    accessibility_status: () => false, open_privacy: () => null,
    voice_start: async () => { let k = 0; const iv = setInterval(() => { emit("voice-level", [Math.random() * 0.8, k / 10]); if (++k > 22) { clearInterval(iv); emit("voice-ended", null); } }, 100); },
    voice_stop: async ({ lang } = {}) => { await wait(700); if (lang === "en-US") return "budge it"; return settings.voice_lang.startsWith("en") ? "tomorrow 10am send the proposal to Hoiana" : "mai 10h gửi đề xuất cho Hoiana"; },
    voice_cancel: () => null,
    sync_now: async () => { await wait(800); return { tasks_pushed: 2, tasks_pulled: 1, notes_pushed: 1, notes_pulled: 0, events: events.length, errors: [], at: Date.now() }; },
    sync_info: () => ({ last_run: Date.now() - 4 * 60e3, last_error: null, linked_tasks: 6, linked_notes: 2 }),
    get_events: () => events, add_to_calendar: async () => { await wait(300); },
    // thả nội dung → việc / lịch (?drop=text|image|contact|url)
    drop_read: ({ paths }) => {
      const k = paths && paths.length ? "image" : q.get("drop") || "text";
      lastDrop = k;
      if (k === "image") return { kind: "image", preview: "", image: "data:image/svg+xml;base64," + btoa('<svg xmlns="http://www.w3.org/2000/svg" width="160" height="100"><rect width="160" height="100" fill="#2c3e66"/><text x="12" y="40" font-family="sans-serif" font-size="14" fill="white">VN123 HAN-DAD</text><text x="12" y="64" font-family="sans-serif" font-size="12" fill="#ccc">20/11 07:45</text></svg>'), contacts: [] };
      if (k === "contact") return { kind: "contact", preview: "", image: null, contacts: ["Nguyễn Văn An"] };
      if (k === "url") return { kind: "url", preview: "https://www.facebook.com/events/123", image: null, contacts: [] };
      return { kind: "text", preview: "Anh ơi mai 14h họp với đối tác bên Hoà Bình nhé\nĐịa điểm: 12 Lý Thái Tổ, Hoàn Kiếm", image: null, contacts: [] };
    },
    // ghi cuộc họp (?meet=rec để giả lập đang ghi)
    popup_done: () => null,
    stock_alerts: () => [], stock_alert_delete: () => null,
    meeting_status: () => (q.get("meet") === "rec" ? { id: "m-live", title: "Họp agency Q4", secs: 754, system_audio: true, stt: "on-device", pending: 1 } : null),
    meeting_start: async ({ system }) => { await wait(400); const m = { id: "m-live", title: "Họp agency Q4", secs: 0, system_audio: system, stt: "on-device", pending: 0 }; setTimeout(() => emit("meeting-state", m), 50); let s = 0; window.__mt = setInterval(() => { s += 0.3; emit("meeting-level", [s, Math.random() * 0.7, Math.random() * 0.5]); }, 300); setTimeout(() => emit("meeting-update", { id: "m-live", pending: 0, lines: [{ at: 0, who: "me", text: "Chào mọi người, hôm nay mình chốt kế hoạch truyền thông quý 4." }, { at: 0, who: "others", text: "Bên em đề xuất chạy thử TikTok Ads hai tuần trước." }] }), 1500); return m; },
    meeting_stop: async () => { clearInterval(window.__mt); await wait(300); setTimeout(() => emit("meeting-state", null), 0); return "m1"; },
    meeting_list: () => [
      { id: "m1", title: "Họp agency Q4", started: Date.now() - 2 * H, duration: 2710, lines: 84, summarized: true, actions: 3, has_audio: true },
      { id: "m2", title: "Cuộc họp 09:30 05/11", started: Date.now() - 4 * 24 * H, duration: 1260, lines: 31, summarized: false, actions: 0, has_audio: true },
    ],
    meeting_get: ({ id }) => ({ id, title: id === "m1" ? "Họp agency Q4" : "Cuộc họp 09:30 05/11", started: Date.now() - 2 * H, duration: 2710, system_audio: true, locale: "vi-VN", stt: "on-device", pending: 0, failed: 0, last_error: "", has_audio: true, recording: false,
      audio_path: "data:audio/wav;base64,UklGRiQAAABXQVZFZm10IBAAAAABAAEAQB8AAIA+AAACABAAZGF0YQAAAAA=", live: id === "m2" ? [{ from: 0, to: 280, summary: { overview: ["Bàn kế hoạch tuyển dụng quý 1"], decisions: [], actions: [{ task: "Đăng tin tuyển dụng", owner: "chị Hà", due: "thứ 2" }] } }] : [],
      lines: [{ at: 4, who: "me", text: "Chào mọi người, hôm nay mình chốt kế hoạch truyền thông quý 4." }, { at: 4, who: "others", text: "Bên em đề xuất chạy thử TikTok Ads hai tuần trước rồi mới tăng ngân sách." }, { at: 52, who: "me", text: "Ok, anh Tuấn gửi báo giá trước thứ sáu nhé." }],
      summary: id === "m1" ? { overview: ["Chốt kế hoạch truyền thông quý 4", "Ngân sách Meta giữ nguyên, thử thêm TikTok Ads"], decisions: ["Chạy thử TikTok Ads 2 tuần trước khi tăng ngân sách"], actions: [{ task: "Gửi báo giá TikTok Ads", owner: "anh Tuấn", due: "thứ 6" }, { task: "Làm brief video ngắn", owner: "", due: "" }, { task: "Chốt lịch quay", owner: "Lan", due: "20/11" }], engine: "Apple Intelligence", at: Date.now(), note: "" } : null }),
    meeting_summarize: async () => { await wait(1200); return { overview: ["Ý chính A", "Ý chính B"], decisions: [], actions: [{ task: "Gửi biên bản", owner: "Bình", due: "mai" }], engine: "Apple Intelligence", at: Date.now(), note: "Chưa có API key Gemini — đã tóm tắt bằng Apple Intelligence." }; },
    meeting_add_tasks: ({ items }) => items.length, meeting_to_note: () => 9001, meeting_update_summary: ({ summary }) => ({ ...summary, engine: "Apple Intelligence · đã sửa", at: Date.now() }), meeting_edit_line: () => null, meeting_item_note: () => 9002, meeting_copy: () => null, meeting_reveal: () => null, meeting_delete: () => null, meeting_rename: () => null,
    capture_supports: ({ paths }) => paths.length === 1 && /\.(png|jpe?g|heic|vcf|txt|md)$/i.test(paths[0]),
    capture_from_clip: ({ id }) => { const c = clips.find((x) => x.id === id); lastDrop = c.kind === "image" ? "image" : "text"; return c.kind === "image" ? { kind: "image", preview: "", image: null, contacts: [] } : { kind: "text", preview: c.preview, image: null, contacts: [] }; },
    capture_set_text: ({ text }) => ({ kind: "text", preview: text, image: null, contacts: [] }),
    capture_analyze: async ({ ai }) => {
      const tm = (d, h, m) => { const x = new Date(); x.setDate(x.getDate() + d); x.setHours(h, m, 0, 0); return x.getTime(); };
      if (lastDrop === "image") {
        if (!ai) throw "Không có nội dung để tạo";
        await wait(900);
        return { kind: "event", title: "Bay Hà Nội → Đà Nẵng (VN123)", start: tm(11, 7, 45), end: tm(11, 9, 5), all_day: false, location: "Sân bay Nội Bài, T1", notes: "Mã đặt chỗ ABC123 · Ghế 12A", engine: "Apple Intelligence", ocr: "VN123 HAN→DAD\n20/11 07:45", calendar_id: "" };
      }
      if (lastDrop === "contact") return { kind: "event", title: "Sinh nhật Nguyễn Văn An", start: tm(46, 0, 0), end: tm(47, 0, 0), all_day: true, location: "", notes: "Trốn Glamping\n☎ 0901 234 567", engine: "rule", ocr: "", calendar_id: "" };
      if (lastDrop === "url") return { kind: "task", title: "Xem lại facebook.com", start: null, end: null, all_day: false, location: "", notes: "https://www.facebook.com/events/123", engine: "rule", ocr: "", calendar_id: "" };
      if (ai) await wait(1200);
      return { kind: "event", title: ai ? "Họp đối tác Hoà Bình" : "Họp với đối tác bên Hoà Bình nhé", start: tm(1, 14, 0), end: tm(1, 15, 0), all_day: false, location: "12 Lý Thái Tổ, Hoàn Kiếm", notes: "Anh ơi mai 14h họp với đối tác bên Hoà Bình nhé\nĐịa điểm: 12 Lý Thái Tổ, Hoàn Kiếm", engine: ai ? "Apple Intelligence" : "rule", ocr: "", calendar_id: "" };
    },
    calendar_list: async () => { await wait(200); return [{ id: "c1", title: "Lịch", source: "iCloud", default: true }, { id: "c2", title: "Công việc", source: "iCloud", default: false }, { id: "g1", title: "binhduong@gmail.com", source: "Google", default: false }]; },
    capture_create: async ({ d }) => { await wait(300); if (d.kind === "task") { const t = { id: ++id, title: d.title, due: d.start, created: Date.now(), done_at: null, notes: d.notes }; tasks.push(t); emit("status", status()); return { task_id: t.id, event: false, message: "Đã thêm việc: " + d.title }; } events.push({ id: "e" + Date.now(), title: d.title, start: d.start, end: d.end, all_day: d.all_day, location: d.location, calendar: "Lịch" }); return { task_id: null, event: true, message: "Đã thêm vào lịch: " + d.title }; },
    journal_get: ({ date }) => {
      const e = journal[date] || { date, mood: null, answers: [], text: "", facts: {}, updated: 0 };
      e.facts = { tasks_done: ["Đặt vé máy bay Đà Nẵng", "Duyệt nội dung TikTok"], tasks_open: 5, tasks_late: 2, events: ["09:30 Standup team marketing", "Họp khách hàng Hoiana"], screen_minutes: 312, notes_written: 2, clips: 14 };
      if (q.get("journal") === "1" && !journal[date]) { e.mood = 4; e.answers = [{ q: "Hôm nay bạn thấy thế nào? 🌤", a: "Khá bận nhưng vui, chốt được lịch với khách." }, { q: "Điều gì làm bạn mỉm cười hôm nay?", a: "Video TikTok về AI lên 10k view 🎉" }]; journal[date] = e; }
      return { entry: e, questions: ["Hôm nay bạn thấy thế nào? 🌤", "Điều gì làm bạn mỉm cười hôm nay?", "Có điều gì còn làm bạn trăn trở không?"], hints: ["Kể Asi nghe 2–3 câu thôi — vui hay mệt đều được.", "Một khoảnh khắc nhỏ thôi cũng được.", "Nói ra cho nhẹ lòng nhé — không có thì bỏ qua."], closing: new Date().getHours() >= 21 ? "Mình sẽ ghi lại nhật ký ngày hôm nay của bạn nhé. Chúc bạn ngủ ngon 🌙" : "Mình sẽ ghi lại nhật ký ngày hôm nay của bạn nhé. Chúc bạn buổi tối thật nhẹ nhàng ✨", streak: 4,
        history: [{ date: "2026-11-03", mood: 5, preview: "Đi chạy bộ buổi sáng, chiều xong slide cho D'Hoi" }, { date: "2026-11-02", mood: 3, preview: "Hơi mệt, ngủ ít" }] };
    },
    journal_save: ({ entry }) => { journal[entry.date] = entry; return entry; },
    journal_delete: () => null, journal_snooze: () => null, journal_skip_today: () => null,
    journal_make_tasks: ({ text }) => text.split(/[,\n]| và /).filter((x) => x.trim()).length,
    journal_export: () => "/Users/binh/Documents/Nhật ký Mini-Asi.md",
    threads_get: () => q.get("threads") === "1" ? ({ connected: true, has_app: true, app_id: "1234", redirect: "https://localhost/", notify: true, expires: Date.now() + 50 * 86400e3,
      cache: { profile: { id: "1", username: "binhduong.mkt", name: "Đỗ Bình Dương", is_verified: false }, overview: { followers: 1840, views: 12400, likes: 386, replies: 52 },
        mentions: [{ id: "m1", username: "tronglamping", text: "Cảm ơn @binhduong.mkt đã chia sẻ kinh nghiệm chạy ads mùa thu 🍂", timestamp: new Date(Date.now() - 3 * H).toISOString().replace("Z", "+0000") }],
        posts: [
          { id: "p1", text: "3 cách mình dùng AI để viết content nhanh gấp đôi mà vẫn giữ giọng thương hiệu 👇\n1. Brief rõ ràng\n2. Cho AI đọc 5 bài cũ\n3. Luôn tự sửa câu mở đầu", timestamp: new Date(Date.now() - 5 * H).toISOString().replace("Z", "+0000"), permalink: "https://www.threads.net/@binhduong.mkt/post/x", metrics: { views: 5320, likes: 214, replies: 31, reposts: 12, quotes: 2 }, unread: 3 },
          { id: "p2", text: "Đi Trốn Glamping cuối tuần, cánh đồng hoa sắp nở rồi mọi người ơi 🌼", timestamp: new Date(Date.now() - 30 * H).toISOString().replace("Z", "+0000"), metrics: { views: 2210, likes: 98, replies: 7, reposts: 3, quotes: 0 }, unread: 0 },
        ], unread: 4, mention_unread: 1, fetched: Date.now(), error: null } })
      : ({ connected: false, has_app: false, app_id: "", redirect: "https://localhost/", notify: true, expires: 0, cache: { posts: [], mentions: [], unread: 0 } }),
    threads_save_app: () => null, threads_login: () => "https://threads.net/oauth/authorize?...", threads_finish: () => null, threads_set_token: () => null, threads_logout: () => null,
    threads_replies: async () => { await wait(400); return [{ id: "r1", username: "lan.content", text: "Mục 2 hay quá anh ơi, em thử ngay", timestamp: new Date(Date.now() - 2 * H).toISOString().replace("Z", "+0000") }, { id: "r2", username: "minh.ads", text: "Anh có template brief không ạ?", timestamp: new Date(Date.now() - 1 * H).toISOString().replace("Z", "+0000") }]; },
    set_display: ({ mode }) => { settings.display_mode = mode; emit("settings-changed", { ...settings }); const u = new URL(location.href); u.searchParams.set("disp", mode); history.replaceState(null, "", u); },
    move_bubble: ({ x, y, save }) => { if (save) { settings.bubble_x = x <= 0 && y <= 0 ? 0 : x; settings.bubble_y = x <= 0 && y <= 0 ? 0 : y; } }, threads_close: () => null,
    update_info: () => ({
      current_version: "0.10.0",
      repo: "binhduongdo09/mini-asi",
      auto_check: settings.auto_update_check,
      status: {
        phase: q.get("update") === "1" ? "available" : "up_to_date",
        percent: q.get("update") === "1" ? 0 : 100,
        downloaded_bytes: 0,
        total_bytes: q.get("update") === "1" ? 14.2e6 : 0,
        message: q.get("update") === "1" ? "Đã có phiên bản mới v0.11.0" : "Bạn đang dùng bản mới nhất (v0.10.0)",
        error: null,
        release: q.get("update") === "1" ? { current_version: "0.10.0", latest_version: "0.11.0", tag: "v0.11.0", title: "Mini-Asi v0.11.0", notes: "Bổ sung tự động cập nhật trong app & ghim vị trí bất kỳ trên màn hình", published_at: new Date().toISOString(), html_url: "https://github.com/binhduongdo09/mini-asi/releases", has_update: true, asset: { id: 1, name: "Mini-Asi_0.11.0_universal.dmg", size: 14.2e6, browser_download_url: "", api_url: "" } } : null,
      },
    }),
    update_check: async () => { await wait(500); return H_.update_info(); },
    update_install: async () => {
      for (const p of [20, 55, 90, 100]) {
        await wait(300);
        emit("update-progress", { phase: p < 100 ? "downloading" : "restarting", percent: p, downloaded_bytes: (p / 100) * 14.2e6, total_bytes: 14.2e6, message: p < 100 ? `Đang tải v0.11.0… ${p}%` : "Đã nâng cấp xong! Đang khởi động lại Mini-Asi…", error: null, release: null });
      }
    },
    tts_info: () => {
      const ph = q.get("tts") || "ready";
      const dl = ph === "ready" ? ["Mai Phương"] : [];
      return { status: { phase: ph, step: "4/4 · Tải giọng Mai Phương (≈ 61 MB)", detail: "", error: null, warm: true, size_mb: 158, voices: dl },
        voice: settings.tts_voice || "Mai Phương", first_mb: 156, voice_mb: 61,
        voices: [["Mai Phương", "female", "Nữ · rõ chữ, nhẹ nhàng"], ["Minh Quang", "male", "Nam · trầm, chắc giọng"]].map(([name, gender, desc]) => ({ name, gender, desc, downloaded: dl.includes(name) })) };
    },
    vocab_batch: () => ({ learned: 12, total: 190, cards: [{"word": "schedule", "pos": "noun", "vi": "lịch trình", "details": {"ipa_us": "/ˈskɛdʒuːl/", "ipa_uk": "/ˈʃɛdjuːl/", "definition": "A plan for carrying out a process.", "example": "I have a busy schedule this week."}, "progress": {"learned": false, "correct": 0, "tries": 0, "best": 0, "shown": 1}}, {"word": "resilient", "pos": "adjective", "vi": "kiên cường", "details": {"ipa_us": "/rɪˈzɪliənt/", "ipa_uk": null, "definition": "Able to recover quickly from difficulties.", "example": "Kids are remarkably resilient."}, "progress": {"learned": false, "correct": 0, "tries": 0, "best": 0, "shown": 1}}, {"word": "prioritize", "pos": "verb", "vi": "ưu tiên", "details": {"ipa_us": "/praɪˈɒrɪtaɪz/", "ipa_uk": null, "definition": "To arrange in order of importance.", "example": "You need to prioritize your tasks."}, "progress": {"learned": false, "correct": 0, "tries": 0, "best": 0, "shown": 1}}] }),
    vocab_play: () => null, vocab_play_mine: () => null, vocab_learned: () => null,
    vocab_check: ({ word, heard }) => heard.toLowerCase().includes(word) ? { score: 96, verdict: "great", heard, tip: "Chuẩn rồi! 🎉" } : { score: 74, verdict: "close", heard, tip: `Gần đúng — Asi nghe thành “${heard}”.` },
    tts_install: () => null, tts_uninstall: () => null, tts_use: ({ voice }) => { settings.tts_voice = voice; return null; }, asi_reset: () => null,
    ai_info: () => [["openai", "ChatGPT", "gpt-5-mini", "https://platform.openai.com/api-keys"], ["gemini", "Gemini", "gemini-flash-latest", "https://aistudio.google.com/apikey"], ["claude", "Claude", "claude-haiku-4-5", "https://console.anthropic.com/settings/keys"]]
      .map(([id, name, dm, url]) => ({ id, name, has_key: id === "gemini", model: settings["ai_model_" + id] || dm, default_model: dm, key_url: url })),
    ai_set_key: () => null, ai_test: async ({ provider }) => { await wait(500); return provider === "gemini" ? "gemini-flash-latest" : "gpt-5-mini"; },
    ime_info: () => ({ current: "com.lacviet.inputmethod.VietTelex", recognized: "telex", enabled: ["com.apple.keylayout.ABC", "com.lacviet.inputmethod.VietTelex"] }),
    threads_post: async () => { await wait(500); return "p9"; }, threads_mark_read: () => null, open_link: () => null, threads_web: () => null, social_open: () => null, social_open_list: () => ["messenger"], threads_embed: () => null,
    asi_ask: async ({ text }) => { await wait(600); const r = await H_._ask_unused({ text }); if (r.engine) return r; if (r.intent === "unknown") { r.text = "Có, mai Hà Nội có mưa rào nhẹ buổi chiều (khoảng 60%). Bạn nhớ mang ô khi ra ngoài sau 3 giờ nhé."; r.actions = []; r.mood = "happy"; r.engine = "Gemini · gemini-flash-latest"; } else r.engine = "Lệnh cơ bản"; return r; },
    _ask_unused: async ({ text }) => { const t = text.toLowerCase();
      if (t.includes("tin ")) return { intent: "unknown", text: "Tin công nghệ trên VnExpress:", speak: "", mood: "happy", engine: "VnExpress", actions: [],
        cards: [{ kind: "links", items: [{ title: "Apple ra mắt MacBook Pro chip M6 với màn hình OLED", url: "https://vnexpress.net/a.html", meta: "35 phút trước" }, { title: "Việt Nam thử nghiệm 5G độc lập tại Hà Nội và TP HCM", url: "https://vnexpress.net/b.html", meta: "2 giờ trước" }, { title: "Ứng dụng AI tóm tắt cuộc họp bằng tiếng Việt", url: "https://vnexpress.net/c.html", meta: "3 giờ trước" }] }] };
      if (t.includes("nhiệt độ") || t.includes("thời tiết")) return { intent: "unknown", text: "Cầu Giấy lúc 14:00: 27°C, nhiều mây (cảm giác như 30°C). Hôm nay 22–29°C, khả năng mưa 20%.", speak: "", mood: "happy", engine: "Open-Meteo", actions: [],
        cards: [{ kind: "metrics", items: [{ label: "Bây giờ", value: "27°C", level: 0, ratio: null }, { label: "Độ ẩm", value: "70%", level: 0, ratio: null }, { label: "Hôm nay", value: "22–29°C", level: 0, ratio: null }, { label: "Khả năng mưa", value: "20%", level: 0, ratio: 0.2 }, { label: "UV", value: "6", level: 1, ratio: null }] }] };
      if (t.includes("fpt") || t.includes("vn-index")) return { intent: "unknown", text: "FPT 51.000đ ▲ +200đ (+0.39%)", speak: "", mood: "happy", engine: "DNSE Entrade · cập nhật 14:45 09/11",
        cards: [{ kind: "metrics", items: [{ label: "FPT", value: "51.000đ +0.39%", level: 0, ratio: null }] }, { kind: "text", text: "FPT: cao nhất 51.500đ · thấp nhất 50.600đ · tham chiếu 50.800đ · KL 1.234.500" }], actions: [{ label: "Báo khi FPT > 53.600đ", cmd: "ask", arg: "báo cho tôi khi FPT vượt 53600" }] };
      if (t.includes("báo cho")) return { intent: "unknown", text: "Asi sẽ báo khi FPT vượt 100.000đ hoặc FPT giảm dưới 50.000đ. Hiện tại: FPT 51.000đ ▲ +200đ (+0.39%). Asi kiểm tra mỗi phút trong giờ giao dịch (9:00–15:00, thứ 2–6).", speak: "", mood: "happy", engine: "Cảnh báo giá", cards: [], actions: [{ label: "Xem các cảnh báo", cmd: "ask", arg: "xem cảnh báo giá" }, { label: "Huỷ cảnh báo FPT", cmd: "ask", arg: "huỷ cảnh báo giá FPT" }] };
      if (t.includes("vàng")) return { intent: "unknown", text: "Asi chưa có nguồn giá vàng trực tiếp nên không đoán con số — bấm để xem giá SJC / nhẫn mới nhất.", speak: "", mood: "happy", engine: "Không có dữ liệu trực tiếp", cards: [], actions: [{ label: "Xem trên web", cmd: "open_url", arg: "https://www.google.com/search?q=gi%C3%A1%20v%C3%A0ng" }] };
      if (t.includes("máy") || t.includes("mac")) return { intent: "status", text: "Asi thấy 2 điều cần để ý: ổ đĩa sắp đầy (còn 9%); 2 việc đang trễ.", speak: "", mood: "worried",
        cards: [{ kind: "metrics", items: [{ label: "Ổ đĩa trống", value: "21.4 GB (9%)", level: 1, ratio: 0.91 }, { label: "CPU", value: "23%", level: 0, ratio: 0.23 }, { label: "RAM", value: "71% / 16 GB", level: 0, ratio: 0.71 }, { label: "Pin", value: "64% ⚡", level: 0, ratio: 0.64 }, { label: "Đã bật", value: "3.2 ngày", level: 0, ratio: null }] }],
        actions: [{ label: "Dọn rác ngay", cmd: "ask", arg: "dọn rác" }, { label: "Xem việc trễ", cmd: "ask", arg: "việc nào trễ" }] };
      if (t.includes("trễ")) return { intent: "overdue", text: "Có 2 việc đang trễ. Lâu nhất là “Gửi báo giá cho anh Tuấn”.", speak: "", mood: "angry", cards: [{ kind: "tasks", items: [{ id: 1, title: "Gửi báo giá cho anh Tuấn", due: Date.now() - 5 * H }, { id: 2, title: "Duyệt banner chiến dịch Q4", due: Date.now() - 0.6 * H }] }], actions: [] };
      if (t.includes("rác")) return { intent: "clean", text: "Asi tìm thấy 13.6 GB rác an toàn (cache, log…). Dọn luôn nhé?", speak: "", mood: "focus", cards: [{ kind: "text", text: "Xcode DerivedData · 6.4 GB\nCache ứng dụng · 5.1 GB\nCache Google Chrome · 1.8 GB" }], actions: [{ label: "Dọn ngay", cmd: "clean_safe", arg: "" }, { label: "Xem chi tiết", cmd: "open_tab", arg: "clean" }] };
      return { intent: "unknown", text: "Hmm, Asi chưa hiểu ý bạn. Thử một trong mấy câu này nhé:", speak: "", mood: "idle", cards: [], actions: [{ label: "Máy mình thế nào?", cmd: "ask", arg: "máy mình thế nào" }] }; },
    asi_action: async () => { await wait(700); return { text: "Xong! Đã giải phóng 13.6 GB 🎉", speak: "", mood: "love", cards: [], actions: [] }; },
    asi_speak: () => null, asi_stop_speaking: () => null, threads_embed_ime: () => null, threads_embed_nav: () => null,
    permissions_status: () => [
      { id: "notifications", name: "Thông báo", why: "Nhắc việc, nhắc nghỉ, báo giờ họp", state: "granted", required: true },
      { id: "microphone", name: "Micro", why: "Ghi chú, thêm việc, nhật ký bằng giọng nói", state: "granted", required: true },
      { id: "speech", name: "Nhận dạng giọng nói", why: "Chuyển giọng nói tiếng Việt / Anh thành chữ", state: "granted", required: true },
      { id: "camera", name: "Camera", why: "Gương xem nhanh & bật/tắt camera trên tai thỏ (tuỳ chọn)", state: "granted", required: false },
      { id: "calendar", name: "Lịch", why: "Hiện lịch họp, báo trước giờ họp", state: "granted" },
      { id: "automation", name: "Lời nhắc & Ghi chú", why: "Đồng bộ việc và ghi chú với app Apple", state: "manual" },
      { id: "accessibility", name: "Trợ năng", why: "Tự dán từ Clipboard", state: "denied" },
      { id: "disk", name: "Full Disk Access", why: "Quét dọn rác sâu hơn", state: "denied" }],
    request_permissions: async () => { await wait(500); return H_.permissions_status(); },
    backup_info: () => ({ dirs: [{ label: "Google Drive (binhduong.iam.neu@gmail.com)", path: "/Users/binh/Library/CloudStorage/GoogleDrive-binhduong.iam.neu@gmail.com/My Drive" }, { label: "iCloud Drive", path: "/Users/binh/Library/Mobile Documents/com~apple~CloudDocs" }],
      current: "/Users/binh/Library/CloudStorage/GoogleDrive-binhduong.iam.neu@gmail.com/My Drive", chosen: "", auto_backup: true, device_sync: true, device_name: "MacBook của Bình Dương", last_backup: Date.now() - 3 * H, last_device_sync: Date.now() - 600e3,
      snapshots: [{ path: "/x/1.zip", name: "1", device: "MacBook của Bình Dương", modified: Date.now() - 3 * H, size: 2.4e6 }, { path: "/x/2.zip", name: "2", device: "MacBook của Bình Dương", modified: Date.now() - 27 * H, size: 2.3e6 }],
      devices: [{ name: "a" }, { name: "b" }] }),
    backup_pick_dir: () => null, backup_set: () => null, backup_now: async () => { await wait(600); return ["/Users/binh/Library/CloudStorage/GoogleDrive-x/My Drive/Mini-Asi/backups/a.zip", 2.4e6]; },
    device_sync_now: async () => { await wait(600); return { devices: ["iMac văn phòng"], stats: { tasks: 3, notes: 1, journal: 0, clips: 5, removed: 0 } }; }, backup_restore: async () => { await wait(500); },
    open_settings_window: async ({ section } = {}) => { window.open(`settings.html${section ? `?sec=${encodeURIComponent(section)}` : ""}`, "_blank", "width=860,height=650"); return null; },
    apple_ai_status: async () => (q.get("ai") === "0" ? { available: false, engine: "Apple Intelligence", detail: "Cần macOS 26 trở lên trên máy chip Apple, đã bật Apple Intelligence" } : { available: true, engine: "Apple Intelligence", detail: "Sẵn sàng · chạy ngay trên máy, không gửi dữ liệu đi" }),
  };
  window.__TAURI__ = {
    core: { invoke: async (cmd, args) => { if (!(cmd in H_)) throw "mock: thiếu lệnh " + cmd; return H_[cmd](args || {}); } },
    event: { listen: async (ev, cb) => { (listeners[ev] ||= []).push(cb); return () => {}; } },
  };
  window.__mock = { emit, tasks, status };
})();
