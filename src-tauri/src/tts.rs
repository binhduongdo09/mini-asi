//! Giọng đọc của Asi — **một** bộ đọc nhẹ chạy hoàn toàn trên máy, tải về khi người dùng chọn.
//!
//! Bộ đọc: NghiTTS (Piper/VITS qua thư viện `piper-tts`, onnxruntime + eSpeak-ng) — 22 kHz, đọc gần như tức thì
//! trên CPU, RAM ≈ 100 MB khi đang chạy, mỗi giọng ≈ 60 MB. Chỉ dùng giọng "thường" (Mai Phương, Minh Quang),
//! KHÔNG dùng các giọng mang tên người nổi tiếng / giọng thương mại. Chỉ tải giọng người dùng chọn.
//!
//! Đã bỏ (0.13): VieNeu-TTS (nặng), edge-tts & Google Translate TTS (gửi chữ ra mạng, điều khoản không cho dùng
//! thương mại), giọng Linh của macOS cho tiếng Việt (kém tự nhiên).
//!
//! Cài: tải `uv` → Python 3.12 riêng trong `<data_dir>/nghitts/` → `piper-tts` → tải giọng (kiểm SHA-256) → đọc thử.
//! Đọc: tiến trình nền `resources/tts_worker.py` giữ mô hình trong RAM, đọc từng câu ra WAV, câu đầu xong là phát;
//! rảnh 5 phút thì tự tắt để trả RAM.

use serde::Serialize;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

pub const WORKER_PY: &str = include_str!("../resources/tts_worker.py");
pub const PIPER_VERSION: &str = "1.8.0";
/// Mã bộ đọc lưu trong cài đặt (`tts_engine`)
pub const ENGINE_ID: &str = "nghi";
/// (tên, giới tính, mô tả, tên file)
pub const VOICES: &[(&str, &str, &str, &str)] = &[
    ("Mai Phương", "female", "Nữ · rõ chữ, nhẹ nhàng", "mai-phuong"),
    ("Minh Quang", "male", "Nam · trầm, chắc giọng", "minh-quang"),
];
pub const DEFAULT_VOICE: &str = "Mai Phương";
/// Dung lượng ước lượng (MB): phần chung (Python + piper) và mỗi giọng
pub const BASE_MB: u32 = 95;
pub const VOICE_MB: u32 = 61;
const IDLE_KILL: Duration = Duration::from_secs(5 * 60);

pub fn has_voice(v: &str) -> bool {
    VOICES.iter().any(|x| x.0 == v)
}
/// Tên giọng hợp lệ (tên cũ / lạ → giọng mặc định)
pub fn voice_or_default(v: &str) -> &str {
    VOICES.iter().find(|x| x.0 == v).map(|x| x.0).unwrap_or(DEFAULT_VOICE)
}
fn slug(v: &str) -> &'static str {
    VOICES.iter().find(|x| x.0 == v).map(|x| x.3).unwrap_or(VOICES[0].3)
}

/// Xoá các bộ đọc cũ đã bỏ (giải phóng ổ đĩa): `vieneu/` (VieNeu / edge-tts)
pub fn remove_legacy(data_dir: &Path) {
    let old = data_dir.join("vieneu");
    if old.exists() {
        let _ = std::fs::remove_dir_all(old);
    }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Status {
    /// none | installing | ready | error
    pub phase: String,
    pub step: String,
    pub detail: String,
    pub error: Option<String>,
    /// tiến trình nền đã nạp xong
    pub warm: bool,
    pub size_mb: u64,
    /// các giọng đã tải về máy
    pub voices: Vec<String>,
}

struct Worker {
    child: Child,
    stdin: ChildStdin,
    ready: Arc<Mutex<bool>>,
}

pub struct Tts {
    pub dir: PathBuf,
    status: Mutex<Status>,
    worker: Mutex<Option<Worker>>,
    gen: Arc<AtomicU64>,
    last_used: Mutex<Instant>,
    play_tx: Mutex<Option<mpsc::Sender<(u64, String)>>>,
    play_pid: Arc<Mutex<Option<u32>>>,
    last_spawn: Mutex<Option<Instant>>,
}

fn exe(p: &Path) -> PathBuf {
    if cfg!(windows) { p.with_extension("exe") } else { p.to_path_buf() }
}

impl Tts {
    pub fn new(data_dir: &Path) -> Arc<Self> {
        let t = Arc::new(Self {
            dir: data_dir.join("nghitts"),
            status: Mutex::new(Status::default()),
            worker: Mutex::new(None),
            gen: Arc::new(AtomicU64::new(1)),
            last_used: Mutex::new(Instant::now()),
            play_tx: Mutex::new(None),
            play_pid: Arc::new(Mutex::new(None)),
            last_spawn: Mutex::new(None),
        });
        t.status.lock().unwrap().phase = if t.installed() { "ready".into() } else { "none".into() };
        let w = Arc::downgrade(&t);
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(30));
            let Some(t) = w.upgrade() else { return };
            if t.last_used.lock().unwrap().elapsed() > IDLE_KILL {
                t.stop_worker();
            }
        });
        t
    }

    fn python(&self) -> PathBuf {
        if cfg!(windows) { self.dir.join("venv").join("Scripts").join("python.exe") } else { self.dir.join("venv").join("bin").join("python") }
    }
    fn marker(&self) -> PathBuf {
        self.dir.join("installed.json")
    }
    fn voices_dir(&self) -> PathBuf {
        self.dir.join("voices")
    }
    /// Đã cài bộ đọc và có ít nhất 1 giọng
    pub fn installed(&self) -> bool {
        self.marker().exists() && self.python().exists() && !self.downloaded().is_empty()
    }
    pub fn has_downloaded(&self, voice: &str) -> bool {
        self.voices_dir().join(format!("{}.onnx", slug(voice))).exists()
    }
    pub fn downloaded(&self) -> Vec<String> {
        VOICES.iter().filter(|v| self.voices_dir().join(format!("{}.onnx", v.3)).exists()).map(|v| v.0.to_string()).collect()
    }
    /// Giọng sẽ dùng thật: giọng đã chọn nếu đã tải, không thì giọng đầu tiên đã tải
    pub fn usable_voice(&self, wanted: &str) -> String {
        let w = voice_or_default(wanted);
        if self.has_downloaded(w) {
            return w.to_string();
        }
        self.downloaded().into_iter().next().unwrap_or_else(|| w.to_string())
    }

    pub fn status(&self) -> Status {
        let mut s = self.status.lock().unwrap().clone();
        if s.phase != "installing" {
            s.phase = if self.installed() { "ready".into() } else if s.error.is_some() { "error".into() } else { "none".into() };
        }
        s.voices = self.downloaded();
        s.warm = self.worker.lock().unwrap().as_ref().map(|w| *w.ready.lock().unwrap()).unwrap_or(false);
        if s.phase == "ready" && s.size_mb == 0 {
            s.size_mb = dir_size(&self.dir) / 1_000_000;
            self.status.lock().unwrap().size_mb = s.size_mb;
        }
        s
    }

    fn set(&self, step: &str, detail: &str) {
        let mut s = self.status.lock().unwrap();
        s.step = step.into();
        s.detail = detail.into();
    }

    fn uv_env(&self, c: &mut Command) {
        c.env("UV_CACHE_DIR", self.dir.join("cache")).env("UV_PYTHON_INSTALL_DIR", self.dir.join("python")).env("UV_NO_CONFIG", "1");
    }

    fn find_uv(&self) -> Option<PathBuf> {
        let mut cands = vec![exe(&self.dir.join("bin").join("uv"))];
        if let Some(h) = std::env::var_os("HOME").map(PathBuf::from) {
            cands.push(exe(&h.join(".local/bin/uv")));
            cands.push(exe(&h.join(".cargo/bin/uv")));
        }
        cands.push("/opt/homebrew/bin/uv".into());
        cands.push("/usr/local/bin/uv".into());
        cands.into_iter().find(|p| p.exists())
    }

    /// Cài bộ đọc (nếu chưa có) và tải giọng `voice` (chạy trên luồng riêng). `progress` báo từng bước.
    pub fn install(self: &Arc<Self>, voice: &str, progress: impl Fn(Status) + Send + 'static) {
        let voice = voice_or_default(voice).to_string();
        {
            let mut s = self.status.lock().unwrap();
            if s.phase == "installing" {
                return;
            }
            *s = Status { phase: "installing".into(), ..Default::default() };
        }
        let t = self.clone();
        std::thread::spawn(move || {
            let r = t.install_inner(&voice, &progress);
            let mut s = t.status.lock().unwrap();
            match r {
                Ok(()) => *s = Status { phase: "ready".into(), step: "Xong".into(), ..Default::default() },
                Err(e) => {
                    s.phase = "error".into();
                    s.error = Some(e);
                }
            }
            drop(s);
            progress(t.status());
        });
    }

    fn install_inner(&self, voice: &str, progress: &dyn Fn(Status)) -> Result<(), String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let tick = |t: &Self, step: &str, detail: &str| {
            t.set(step, detail);
            progress(t.status.lock().unwrap().clone());
        };
        let base_ready = self.marker().exists() && self.python().exists();
        if !base_ready {
            // 1. uv
            tick(self, "1/4 · Chuẩn bị", "");
            let uv = match self.find_uv() {
                Some(u) => u,
                None => {
                    let bin = self.dir.join("bin");
                    let mut c = if cfg!(windows) {
                        let mut c = Command::new("powershell");
                        c.args(["-ExecutionPolicy", "ByPass", "-NoProfile", "-c", "irm https://astral.sh/uv/install.ps1 | iex"]);
                        c
                    } else {
                        let mut c = Command::new("sh");
                        c.args(["-c", "curl -LsSf https://astral.sh/uv/install.sh | sh"]);
                        c
                    };
                    c.env("UV_UNMANAGED_INSTALL", &bin).env("INSTALLER_NO_MODIFY_PATH", "1");
                    run(c, "tải uv", |l| tick(self, "1/4 · Chuẩn bị", l))?;
                    self.find_uv().ok_or("Không tải được bộ cài — kiểm tra kết nối mạng")?
                }
            };
            // 2. Python riêng (không đụng tới Python của máy)
            if !self.python().exists() {
                tick(self, "2/4 · Tạo môi trường riêng", "");
                let mut c = Command::new(&uv);
                c.args(["venv", "--python", "3.12"]).arg(self.dir.join("venv"));
                self.uv_env(&mut c);
                run(c, "tạo môi trường Python", |l| tick(self, "2/4 · Tạo môi trường riêng", l))?;
            }
            // 3. Piper (onnxruntime + eSpeak-ng đã kèm trong gói)
            tick(self, "3/4 · Cài bộ đọc", "");
            let mut c = Command::new(&uv);
            c.args(["pip", "install", "--python"]).arg(self.python()).arg(format!("piper-tts=={PIPER_VERSION}"));
            self.uv_env(&mut c);
            run(c, "cài piper-tts", |l| tick(self, "3/4 · Cài bộ đọc", l))?;
            prune_python(&self.dir.join("python"));
            let _ = std::fs::remove_dir_all(self.dir.join("cache"));
        }
        // 4. tải giọng + đọc thử
        let step4 = format!("4/4 · Tải giọng {voice} (≈ {VOICE_MB} MB)");
        tick(self, &step4, "");
        let w = self.write_worker()?;
        let out = self.dir.join("warmup.wav");
        let _ = std::fs::remove_file(&out);
        let mut c = Command::new(self.python());
        c.arg(&w).arg("--warmup").arg(&out).env("TTS_VOICE", voice).env("PYTHONUNBUFFERED", "1").env("NGHI_DIR", self.voices_dir());
        let lines = run(c, "tải giọng", |l| tick(self, &step4, l))?;
        if let Some(f) = lines.iter().find(|l| l.contains("\"fatal\"")) {
            return Err(format!("Bộ đọc không chạy được: {f}"));
        }
        if !out.exists() {
            return Err("Đọc thử không ra âm thanh".into());
        }
        let _ = std::fs::remove_file(&out);
        std::fs::write(self.marker(), serde_json::json!({ "engine": ENGINE_ID, "piper": PIPER_VERSION }).to_string()).map_err(|e| e.to_string())?;
        self.status.lock().unwrap().size_mb = 0; // tính lại dung lượng
        Ok(())
    }

    fn write_worker(&self) -> Result<PathBuf, String> {
        let p = self.dir.join("tts_worker.py");
        if std::fs::read_to_string(&p).ok().as_deref() != Some(WORKER_PY) {
            std::fs::write(&p, WORKER_PY).map_err(|e| e.to_string())?;
        }
        Ok(p)
    }

    pub fn uninstall(&self) -> Result<(), String> {
        self.stop_worker();
        if self.dir.exists() {
            std::fs::remove_dir_all(&self.dir).map_err(|e| e.to_string())?;
        }
        *self.status.lock().unwrap() = Status { phase: "none".into(), ..Default::default() };
        Ok(())
    }

    pub fn stop_worker(&self) {
        if let Some(mut w) = self.worker.lock().unwrap().take() {
            let _ = writeln!(w.stdin, "{{\"quit\":true}}");
            let _ = w.child.kill();
            let _ = w.child.wait();
        }
    }

    /// Khởi động tiến trình nền nếu chưa chạy. Trả về true nếu mô hình đã sẵn sàng.
    pub fn ensure_worker(self: &Arc<Self>) -> bool {
        *self.last_used.lock().unwrap() = Instant::now();
        let mut g = self.worker.lock().unwrap();
        if let Some(w) = g.as_mut() {
            if matches!(w.child.try_wait(), Ok(None)) {
                return *w.ready.lock().unwrap();
            }
            *g = None;
        }
        if !self.installed() || self.status.lock().unwrap().phase == "installing" {
            return false;
        }
        {
            // tránh khởi động lại liên tục nếu tiến trình vừa thoát lỗi tức thì (< 2 giây)
            let mut ls = self.last_spawn.lock().unwrap();
            if ls.map(|t| t.elapsed() < Duration::from_secs(2)).unwrap_or(false) {
                return false;
            }
            *ls = Some(Instant::now());
        }
        let Ok(script) = self.write_worker() else { return false };
        let mut c = Command::new(self.python());
        c.arg(script).env("PYTHONUNBUFFERED", "1").env("NGHI_DIR", self.voices_dir()).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
        let Ok(mut child) = c.spawn() else { return false };
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else { return false };
        let ready = Arc::new(Mutex::new(false));
        let tx = self.player();
        let (r2, gen) = (ready.clone(), self.gen.clone());
        let me = Arc::downgrade(self);
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else { continue };
                if v.get("ready").is_some() {
                    *r2.lock().unwrap() = true;
                } else if let Some(f) = v.get("fatal").and_then(|x| x.as_str()) {
                    if let Some(t) = me.upgrade() {
                        t.status.lock().unwrap().error = Some(f.to_string());
                    }
                } else if let (Some(id), Some(f)) = (v.get("id").and_then(|x| x.as_u64()), v.get("file").and_then(|x| x.as_str())) {
                    if id == gen.load(Ordering::SeqCst) && !f.is_empty() {
                        let _ = tx.send((id, f.to_string()));
                    }
                }
            }
        });
        *g = Some(Worker { child, stdin, ready });
        false
    }

    /// Luồng phát âm thanh: phát lần lượt từng câu, bỏ qua câu của lượt đọc cũ.
    fn player(&self) -> mpsc::Sender<(u64, String)> {
        let mut g = self.play_tx.lock().unwrap();
        if let Some(tx) = g.as_ref() {
            return tx.clone();
        }
        let (tx, rx) = mpsc::channel::<(u64, String)>();
        let (gen, pid) = (self.gen.clone(), self.play_pid.clone());
        std::thread::spawn(move || {
            for (id, f) in rx {
                if id != gen.load(Ordering::SeqCst) {
                    let _ = std::fs::remove_file(&f);
                    continue;
                }
                if let Ok(mut c) = play_cmd(&f).spawn() {
                    *pid.lock().unwrap() = Some(c.id());
                    let _ = c.wait();
                    *pid.lock().unwrap() = None;
                }
                let _ = std::fs::remove_file(&f);
            }
        });
        *g = Some(tx.clone());
        tx
    }

    /// Đọc to. Gửi thẳng vào hàng đợi của tiến trình nền (kể cả khi nó vừa khởi động — câu sẽ đọc ngay khi nạp xong).
    pub fn speak(self: &Arc<Self>, text: &str, voice: &str) -> bool {
        if !self.installed() {
            return false;
        }
        let _ = self.ensure_worker();
        let id = self.gen.fetch_add(1, Ordering::SeqCst) + 1;
        let out = std::env::temp_dir().join("mini-asi-voice");
        let msg = serde_json::json!({ "id": id, "text": text, "voice": self.usable_voice(voice), "dir": out });
        let mut g = self.worker.lock().unwrap();
        match g.as_mut() {
            Some(w) => writeln!(w.stdin, "{msg}").and_then(|_| w.stdin.flush()).is_ok(),
            None => false,
        }
    }

    pub fn stop(&self) {
        let id = self.gen.fetch_add(1, Ordering::SeqCst);
        if let Some(w) = self.worker.lock().unwrap().as_mut() {
            let _ = writeln!(w.stdin, "{{\"cancel\":{id}}}");
            let _ = w.stdin.flush();
        }
        if let Some(p) = self.play_pid.lock().unwrap().take() {
            kill_pid(p);
        }
    }
}

/// Bỏ phần Python không dùng tới (bộ test, IDLE, Tk…) cho gọn ≈ 40 MB
fn prune_python(root: &Path) {
    const DROP: &[&str] = &["test", "idlelib", "tkinter", "turtledemo", "ensurepip", "lib2to3", "pydoc_data"];
    let dirs: Vec<PathBuf> = walkdir::WalkDir::new(root)
        .max_depth(5)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_dir())
        .filter(|e| {
            let name = e.file_name().to_string_lossy();
            let parent = e.path().parent().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            (parent.starts_with("python3.") && DROP.contains(&name.as_ref())) || (parent == "lib" && (name.starts_with("tcl8") || name.starts_with("tk8")))
        })
        .map(|e| e.into_path())
        .collect();
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

fn play_cmd(f: &str) -> Command {
    #[cfg(target_os = "macos")]
    {
        let mut c = Command::new("afplay");
        c.arg(f);
        c
    }
    #[cfg(target_os = "windows")]
    {
        let mut c = Command::new("powershell");
        c.args(["-NoProfile", "-c", &format!("(New-Object Media.SoundPlayer '{}').PlaySync()", f.replace('\'', "''"))]);
        c
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let mut c = Command::new("sh");
        c.args(["-c", "paplay \"$0\" 2>/dev/null || aplay -q \"$0\"", f]);
        c
    }
}

pub fn kill_pid(pid: u32) {
    #[cfg(unix)]
    unsafe {
        libc::kill(pid as i32, libc::SIGTERM);
    }
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill").args(["/PID", &pid.to_string(), "/F"]).output();
    }
}

/// Chạy lệnh, chuyển từng dòng stdout/stderr (kể cả thanh tiến độ dùng `\r`) cho `on_line`.
fn run(mut c: Command, what: &str, on_line: impl Fn(&str)) -> Result<Vec<String>, String> {
    c.stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null());
    let mut child = c.spawn().map_err(|e| format!("Không {what} được: {e}"))?;
    let (tx, rx) = mpsc::channel::<String>();
    for r in [child.stdout.take().map(|s| Box::new(s) as Box<dyn Read + Send>), child.stderr.take().map(|s| Box::new(s) as Box<dyn Read + Send>)].into_iter().flatten() {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            for b in std::io::BufReader::new(r).bytes().map_while(Result::ok) {
                if b == b'\n' || b == b'\r' {
                    if !buf.is_empty() {
                        let _ = tx.send(String::from_utf8_lossy(&buf).trim().to_string());
                        buf.clear();
                    }
                } else {
                    buf.push(b);
                }
            }
            if !buf.is_empty() {
                let _ = tx.send(String::from_utf8_lossy(&buf).trim().to_string());
            }
        });
    }
    drop(tx);
    let mut lines = vec![];
    let mut last = Instant::now() - Duration::from_secs(1);
    for l in rx {
        if l.is_empty() {
            continue;
        }
        if last.elapsed() > Duration::from_millis(250) {
            on_line(&short(&l));
            last = Instant::now();
        }
        lines.push(l);
        if lines.len() > 400 {
            lines.drain(..200);
        }
    }
    let st = child.wait().map_err(|e| e.to_string())?;
    if !st.success() {
        let tail = lines.iter().rev().take(4).rev().cloned().collect::<Vec<_>>().join(" · ");
        return Err(format!("Lỗi khi {what}: {}", short(&tail)));
    }
    Ok(lines)
}

fn short(s: &str) -> String {
    let s: String = s.chars().filter(|c| !c.is_control()).collect();
    if s.chars().count() > 140 { s.chars().take(140).collect::<String>() + "…" } else { s }
}

fn dir_size(p: &Path) -> u64 {
    walkdir::WalkDir::new(p).into_iter().filter_map(|e| e.ok()).filter_map(|e| e.metadata().ok()).filter(|m| m.is_file()).map(|m| m.len()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prune_keeps_needed_stdlib() {
        let d = tempfile::tempdir().unwrap();
        let lib = d.path().join("cpython-3.12/lib");
        for p in ["python3.12/test/x", "python3.12/json", "python3.12/tkinter", "python3.12/site-packages/test", "tcl8.6"] {
            std::fs::create_dir_all(lib.join(p)).unwrap();
        }
        prune_python(d.path());
        assert!(!lib.join("python3.12/test").exists() && !lib.join("python3.12/tkinter").exists() && !lib.join("tcl8.6").exists());
        assert!(lib.join("python3.12/json").exists());
        assert!(lib.join("python3.12/site-packages/test").exists(), "không đụng vào gói đã cài");
    }

    #[test]
    fn voices_and_worker() {
        assert!(WORKER_PY.contains("split_sentences") && !WORKER_PY.contains("edge_tts") && !WORKER_PY.contains("vieneu"));
        assert!(has_voice(DEFAULT_VOICE));
        assert_eq!(voice_or_default("Hoài My"), DEFAULT_VOICE, "giọng cũ (edge-tts) → giọng mặc định");
        // tên file trong máy phải khớp tên file worker tải về (_slug trong tts_worker.py)
        assert_eq!(slug("Mai Phương"), "mai-phuong");
        assert_eq!(slug("Minh Quang"), "minh-quang");
    }

    #[test]
    fn legacy_dir_removed() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("vieneu/venv")).unwrap();
        remove_legacy(d.path());
        assert!(!d.path().join("vieneu").exists());
    }

    /// Tiến trình nền nạp `piper` (giả) và đọc ra file WAV thật
    #[cfg(unix)]
    #[test]
    fn worker_with_fake_piper() {
        use std::os::unix::fs::PermissionsExt;
        let d = tempfile::tempdir().unwrap();
        let t = Tts::new(d.path());
        let fake = d.path().join("fake/piper");
        std::fs::create_dir_all(&fake).unwrap();
        std::fs::write(
            fake.join("__init__.py"),
            "class PiperVoice:\n    @staticmethod\n    def load(m, config_path=None): return PiperVoice()\n    def synthesize_wav(self, text, wf):\n        wf.setnchannels(1); wf.setsampwidth(2); wf.setframerate(22050); wf.writeframes(b'\\0\\0' * 100)\n",
        )
        .unwrap();
        let bin = t.dir.join("venv/bin");
        std::fs::create_dir_all(&bin).unwrap();
        let py = bin.join("python");
        std::fs::write(&py, format!("#!/bin/sh\nPYTHONPATH={} exec python3 \"$@\"\n", d.path().join("fake").display())).unwrap();
        std::fs::set_permissions(&py, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(t.marker(), "{}").unwrap();
        assert!(!t.installed(), "chưa có giọng nào thì chưa tính là đã cài");
        std::fs::create_dir_all(t.voices_dir()).unwrap();
        std::fs::write(t.voices_dir().join("minh-quang.onnx"), "x").unwrap();
        assert!(t.installed());
        assert_eq!(t.downloaded(), vec!["Minh Quang".to_string()]);
        assert_eq!(t.usable_voice("Mai Phương"), "Minh Quang", "giọng chưa tải → dùng giọng đã có");
        assert!(!t.ensure_worker(), "lần đầu còn đang nạp");
        let mut ok = false;
        for _ in 0..50 {
            std::thread::sleep(Duration::from_millis(100));
            if t.ensure_worker() {
                ok = true;
                break;
            }
        }
        assert!(ok, "tiến trình nền phải báo sẵn sàng");
        assert!(t.status().warm);
        assert!(t.speak("Chào bạn. Lúc 15:30 nhớ uống nước nhé!", "Minh Quang"));
        t.stop();
        t.stop_worker();
    }

    #[cfg(unix)]
    #[test]
    fn run_collects_lines_and_errors() {
        let mut c = Command::new("sh");
        c.args(["-c", "printf 'a\\rb\\nc\\n'; echo oops >&2; exit 3"]);
        let e = run(c, "thử", |_| {}).unwrap_err();
        assert!(e.contains("oops"), "{e}");
        let mut c = Command::new("sh");
        c.args(["-c", "printf 'x\\ry\\n'"]);
        assert_eq!(run(c, "thử", |_| {}).unwrap(), vec!["x", "y"]);
    }

    #[test]
    fn not_installed_by_default() {
        let d = tempfile::tempdir().unwrap();
        let t = Tts::new(d.path());
        assert!(!t.installed());
        assert_eq!(t.status().phase, "none");
        assert!(!t.speak("xin chào", DEFAULT_VOICE));
    }
}
