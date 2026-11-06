//! Giọng đọc tự nhiên cho Asi — tải về khi người dùng chọn, chạy hoàn toàn trên máy.
//!
//! Hai bộ đọc (mỗi bộ một thư mục riêng trong `<data_dir>/`, gỡ là xoá sạch):
//! - **VieNeu-TTS v3 Turbo** (`vieneu/`, Apache-2.0): 48 kHz, tự nhiên nhất. Bản Gọn (int8) ≈ 210 MB mô hình,
//!   bỏ phần nhân bản giọng (denoiser, codec encoder) vì Asi chỉ dùng giọng dựng sẵn.
//! - **NghiTTS** (`nghitts/`, Piper/VITS qua `piper-tts`): 22 kHz, rất nhẹ & nhanh, ≈ 60 MB mỗi giọng.
//!   Chỉ dùng giọng "thường", KHÔNG dùng các giọng mang tên người nổi tiếng / giọng thương mại.
//!
//! Cài: tải `uv` → tạo Python 3.12 riêng → cài gói tối giản của bộ đọc → chạy thử (tải mô hình).
//! Đọc: tiến trình nền `resources/tts_worker.py` giữ mô hình trong RAM, đọc từng câu ra WAV; câu đầu xong
//! là phát ngay (afplay / paplay / PowerShell). Rảnh 20 phút thì tự tắt. Đang nạp thì Asi dùng giọng hệ thống.

use serde::Serialize;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

pub const WORKER_PY: &str = include_str!("../resources/tts_worker.py");
pub const VIENEU_VERSION: &str = "3.8.3";
pub const PIPER_VERSION: &str = "1.8.0";
// Chỉ những gói cần để ĐỌC bằng giọng dựng sẵn (soxr, kaldi-native-fbank chỉ dùng khi nhân bản giọng → bỏ)
const VIENEU_DEPS: &[&str] = &["sea-g2p>=0.9.1", "onnxruntime>=1.20.0", "numpy", "soundfile", "tokenizers>=0.20", "huggingface_hub"];

/// Bộ đọc giọng tải thêm
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    VieNeu,
    Nghi,
}

impl Engine {
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "vieneu" => Some(Self::VieNeu),
            "nghi" => Some(Self::Nghi),
            _ => None,
        }
    }
    pub fn id(self) -> &'static str {
        match self {
            Self::VieNeu => "vieneu",
            Self::Nghi => "nghi",
        }
    }
    fn dir_name(self) -> &'static str {
        match self {
            Self::VieNeu => "vieneu",
            Self::Nghi => "nghitts",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::VieNeu => "VieNeu-TTS",
            Self::Nghi => "NghiTTS",
        }
    }
    /// (tên giọng, giới tính, mô tả)
    pub fn voices(self) -> &'static [(&'static str, &'static str, &'static str)] {
        match self {
            Self::VieNeu => &[("Trúc Ly", "female", "Nữ · giọng Bắc, trong trẻo, tự nhiên"), ("Hải Đăng", "male", "Nam · giọng Bắc, ấm áp, tự nhiên")],
            Self::Nghi => &[("Mai Phương", "female", "Nữ · giọng Bắc, nhẹ nhàng, ấm áp"), ("Minh Quang", "male", "Nam · giọng Bắc, rõ ràng, chững chạc")],
        }
    }
    pub fn default_voice(self) -> &'static str {
        self.voices()[0].0
    }
    pub fn has_voice(self, v: &str) -> bool {
        self.voices().iter().any(|x| x.0 == v)
    }
    /// Dung lượng mô hình phải tải (MB, ước lượng) theo bản chọn
    pub fn model_mb(self, quality: &str) -> u32 {
        match self {
            Self::VieNeu => QUALITIES.iter().find(|q| q.0 == quality).map(|q| q.2).unwrap_or(210),
            Self::Nghi => 122,
        }
    }
}

/// "compact" = backbone int8 (≈ 210 MB mô hình) · "high" = fp32 (≈ 520 MB, nét hơn chút)
pub const QUALITIES: &[(&str, &str, u32)] = &[("compact", "Gọn", 210), ("high", "Chất lượng cao", 520)];
/// Chip Apple (ARM) chạy int8 tốt; máy Intel không có VNNI dễ bị méo tiếng với int8 → mặc định fp32
pub fn default_quality() -> &'static str {
    if cfg!(target_arch = "aarch64") { "compact" } else { "high" }
}
const IDLE_KILL: Duration = Duration::from_secs(20 * 60);

/// Giọng mặc định của VieNeu (giữ tên cũ cho phần cài đặt)
pub const DEFAULT_VOICE: &str = "Trúc Ly";

#[derive(Debug, Clone, Serialize, Default)]
pub struct Status {
    /// none | installing | ready | error
    pub phase: String,
    pub step: String,
    pub detail: String,
    pub error: Option<String>,
    /// tiến trình nền đã nạp xong mô hình
    pub warm: bool,
    pub size_mb: u64,
    /// bản mô hình đã tải: compact | high
    pub quality: Option<String>,
}

struct Worker {
    child: Child,
    stdin: ChildStdin,
    ready: Arc<Mutex<bool>>,
}

pub struct Tts {
    pub engine: Engine,
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
    pub fn new(data_dir: &Path, engine: Engine) -> Arc<Self> {
        let t = Arc::new(Self {
            engine,
            dir: data_dir.join(engine.dir_name()),
            status: Mutex::new(Status::default()),
            worker: Mutex::new(None),
            gen: Arc::new(AtomicU64::new(1)),
            last_used: Mutex::new(Instant::now()),
            play_tx: Mutex::new(None),
            play_pid: Arc::new(Mutex::new(None)),
            last_spawn: Mutex::new(None),
        });
        {
            let mut s = t.status.lock().unwrap();
            s.phase = if t.installed() { "ready".into() } else { "none".into() };
        }
        let w = Arc::downgrade(&t);
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(60));
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
    pub fn installed(&self) -> bool {
        self.marker().exists() && self.python().exists()
    }
    /// Bản mô hình đang có trên máy ("compact" | "high")
    pub fn installed_quality(&self) -> Option<String> {
        if !self.installed() {
            return None;
        }
        let v: serde_json::Value = std::fs::read_to_string(self.marker()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        Some(v.get("quality").and_then(|q| q.as_str()).unwrap_or("high").to_string())
    }

    pub fn status(&self) -> Status {
        let mut s = self.status.lock().unwrap().clone();
        if s.phase != "installing" {
            s.phase = if self.installed() { "ready".into() } else if s.error.is_some() { "error".into() } else { "none".into() };
        }
        s.quality = self.installed_quality();
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
        c.env("UV_CACHE_DIR", self.dir.join("cache"))
            .env("UV_PYTHON_INSTALL_DIR", self.dir.join("python"))
            .env("UV_NO_CONFIG", "1")
            .env("HF_HOME", self.dir.join("hf"));
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

    /// Cài toàn bộ (chạy trên luồng riêng). `progress` được gọi mỗi khi có bước mới.
    pub fn install(self: &Arc<Self>, quality: &str, progress: impl Fn(Status) + Send + 'static) {
        let quality = if self.engine == Engine::Nghi { "standard".to_string() } else if QUALITIES.iter().any(|q| q.0 == quality) { quality.to_string() } else { default_quality().to_string() };
        {
            let mut s = self.status.lock().unwrap();
            if s.phase == "installing" {
                return;
            }
            *s = Status { phase: "installing".into(), ..Default::default() };
        }
        let t = self.clone();
        std::thread::spawn(move || {
            t.stop_worker();
            let r = t.install_inner(&quality, &progress);
            let mut s = t.status.lock().unwrap();
            match r {
                Ok(()) => *s = Status { phase: "ready".into(), step: "Xong".into(), ..Default::default() },
                Err(e) => {
                    s.phase = "error".into();
                    s.error = Some(e);
                }
            }
            let snap = s.clone();
            drop(s);
            progress(snap);
        });
    }

    fn install_inner(&self, quality: &str, progress: &dyn Fn(Status)) -> Result<(), String> {
        let mb = self.engine.model_mb(quality);
        let step4 = format!("4/4 · Tải mô hình giọng (≈ {mb} MB, chỉ 1 lần)");
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let tick = |t: &Self, step: &str, detail: &str| {
            t.set(step, detail);
            progress(t.status.lock().unwrap().clone());
        };
        // 1. uv
        tick(self, "1/4 · Chuẩn bị trình cài Python (uv)", "");
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
                run(c, "tải uv", |l| tick(self, "1/4 · Tải uv", l))?;
                self.find_uv().ok_or("Không tải được uv — kiểm tra kết nối mạng")?
            }
        };
        // 2. Python riêng
        if !self.python().exists() {
            tick(self, "2/4 · Tạo môi trường Python 3.12 riêng", "");
            let mut c = Command::new(&uv);
            c.args(["venv", "--python", "3.12"]).arg(self.dir.join("venv"));
            self.uv_env(&mut c);
            run(c, "tạo môi trường Python", |l| tick(self, "2/4 · Tạo môi trường Python 3.12 riêng", l))?;
        }
        // 3. gói tối giản của bộ đọc (ưu tiên cài edge-tts nhẹ + thử cài gói offline nếu có)
        let py = self.python();
        tick(self, "3/4 · Cài bộ đọc giọng tự nhiên Neural TTS", "");
        let mut c = Command::new(&uv);
        c.args(["pip", "install", "--python"]).arg(&py).arg("edge-tts>=6.1.12");
        self.uv_env(&mut c);
        run(c, "cài bộ đọc giọng tự nhiên", |l| tick(self, "3/4 · Cài bộ đọc giọng tự nhiên", l))?;
        match self.engine {
            Engine::VieNeu => {
                let mut c = Command::new(&uv);
                c.args(["pip", "install", "--python"]).arg(&py).args(["--no-deps", &format!("vieneu=={VIENEU_VERSION}")]);
                self.uv_env(&mut c);
                if run(c, "cài vieneu", |l| tick(self, "3/4 · Cài VieNeu-TTS", l)).is_ok() {
                    let mut c2 = Command::new(&uv);
                    c2.args(["pip", "install", "--python"]).arg(&py).args(VIENEU_DEPS);
                    self.uv_env(&mut c2);
                    let _ = run(c2, "cài thư viện", |l| tick(self, "3/4 · Cài thư viện (onnxruntime, sea-g2p…)", l));
                }
            }
            Engine::Nghi => {
                let mut c = Command::new(&uv);
                c.args(["pip", "install", "--python"]).arg(&py).arg("piper-tts>=1.2.0");
                self.uv_env(&mut c);
                let _ = run(c, "cài piper-tts", |l| tick(self, "3/4 · Cài bộ đọc Piper", l));
            }
        }
        prune_python(&self.dir.join("python"));
        // 4. tải mô hình + đọc thử
        tick(self, &step4, "");
        let w = self.write_worker()?;
        let out = self.dir.join("warmup.wav");
        let mut c = Command::new(&py);
        c.arg(&w).arg("--warmup").arg(&out).env("TTS_VOICE", self.engine.default_voice()).env("PYTHONUNBUFFERED", "1");
        self.worker_env(&mut c, quality);
        let lines = run(c, "tải mô hình", |l| tick(self, &step4, l))?;
        if let Some(f) = lines.iter().find(|l| l.contains("\"fatal\"")) {
            return Err(format!("{} không chạy được: {f}", self.engine.name()));
        }
        if !out.exists() {
            return Err("Đọc thử không ra âm thanh".into());
        }
        let _ = std::fs::remove_dir_all(self.dir.join("cache"));
        let marker = match self.engine {
            Engine::VieNeu => serde_json::json!({ "engine": "vieneu", "vieneu": VIENEU_VERSION, "quality": quality }),
            Engine::Nghi => serde_json::json!({ "engine": "nghi", "piper": PIPER_VERSION, "quality": "standard" }),
        };
        std::fs::write(self.marker(), marker.to_string()).map_err(|e| e.to_string())?;
        self.status.lock().unwrap().size_mb = 0; // tính lại dung lượng
        Ok(())
    }

    /// Biến môi trường cho tiến trình đọc (chọn bộ đọc, nơi để mô hình)
    fn worker_env(&self, c: &mut Command, quality: &str) {
        self.uv_env(c);
        c.env("MINI_ASI_ENGINE", self.engine.id());
        match self.engine {
            Engine::VieNeu => {
                c.env("VIENEU_PRECISION", precision(quality));
            }
            Engine::Nghi => {
                c.env("NGHI_DIR", self.dir.join("voices"));
            }
        }
    }

    fn write_worker(&self) -> Result<PathBuf, String> {
        let p = self.dir.join("tts_worker.py");
        let _ = std::fs::remove_file(self.dir.join("vieneu_worker.py")); // tên cũ (≤ 0.9)
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
            // tiến trình vừa chết (vd. lỗi nạp mô hình) → đừng khởi động lại liên tục
            let mut ls = self.last_spawn.lock().unwrap();
            if ls.map(|t| t.elapsed() < Duration::from_secs(60)).unwrap_or(false) {
                return false;
            }
            *ls = Some(Instant::now());
        }
        let Ok(script) = self.write_worker() else { return false };
        let mut c = Command::new(self.python());
        let q = self.installed_quality().unwrap_or_else(|| "high".into());
        c.arg(script).env("HF_HUB_OFFLINE", "1").env("PYTHONUNBUFFERED", "1").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
        self.worker_env(&mut c, &q);
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

    /// Đọc bằng bộ đọc này. Trả về false nếu chưa sẵn sàng (để gọi giọng hệ thống thay thế).
    pub fn speak(self: &Arc<Self>, text: &str, voice: &str) -> bool {
        if !self.ensure_worker() {
            return false;
        }
        let id = self.gen.fetch_add(1, Ordering::SeqCst) + 1;
        let out = std::env::temp_dir().join("mini-asi-voice");
        let msg = serde_json::json!({ "id": id, "text": text, "voice": voice, "dir": out });
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

fn precision(quality: &str) -> &'static str {
    if quality == "compact" { "int8" } else { "fp32" }
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
    fn worker_embedded() {
        assert!(WORKER_PY.contains("split_sentences"));
        assert!(Engine::VieNeu.has_voice(DEFAULT_VOICE) && Engine::Nghi.has_voice(Engine::Nghi.default_voice()));
    }

    /// Giả lập môi trường đã cài: python giả + gói `vieneu` giả → kiểm tra tiến trình nền thật
    #[cfg(unix)]
    #[test]
    fn worker_roundtrip_with_fake_model() {
        use std::os::unix::fs::PermissionsExt;
        let d = tempfile::tempdir().unwrap();
        let t = Tts::new(d.path(), Engine::VieNeu);
        let fake = d.path().join("fake/vieneu");
        std::fs::create_dir_all(&fake).unwrap();
        std::fs::write(fake.join("__init__.py"), "class T:\n    def list_preset_voices(self): return [('a','Trúc Ly')]\n    def infer(self, text, voice=None): return text\n    def save(self, a, p): open(p,'w').write(a)\ndef Vieneu(mode='v3turbo', **k): return T()\n").unwrap();
        let bin = t.dir.join("venv/bin");
        std::fs::create_dir_all(&bin).unwrap();
        let py = bin.join("python");
        std::fs::write(&py, format!("#!/bin/sh\nPYTHONPATH={} exec python3 \"$@\"\n", d.path().join("fake").display())).unwrap();
        std::fs::set_permissions(&py, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(t.marker(), "{}").unwrap();
        assert!(t.installed());
        assert!(!t.ensure_worker(), "lần đầu còn đang nạp");
        let mut ok = false;
        for _ in 0..50 {
            std::thread::sleep(Duration::from_millis(100));
            if t.ensure_worker() { ok = true; break; }
        }
        assert!(ok, "tiến trình nền phải báo sẵn sàng");
        assert!(t.status().warm);
        assert!(t.speak("Xin chào. Mình là Asi nè, rất vui được gặp bạn!", DEFAULT_VOICE));
        t.stop();
        t.stop_worker();
    }

    /// NghiTTS: tiến trình nền nạp `piper` (giả) và đọc ra file WAV thật
    #[cfg(unix)]
    #[test]
    fn nghi_worker_with_fake_piper() {
        use std::os::unix::fs::PermissionsExt;
        let d = tempfile::tempdir().unwrap();
        let t = Tts::new(d.path(), Engine::Nghi);
        assert!(t.dir.ends_with("nghitts"));
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
        std::fs::write(t.marker(), "{\"engine\":\"nghi\"}").unwrap();
        assert!(!t.ensure_worker());
        let mut ok = false;
        for _ in 0..50 {
            std::thread::sleep(Duration::from_millis(100));
            if t.ensure_worker() { ok = true; break; }
        }
        assert!(ok, "tiến trình NghiTTS phải báo sẵn sàng");
        assert!(t.speak("Chào bạn. Lúc 15:30 nhớ uống nước nhé!", "Mai Phương"));
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
        let t = Tts::new(d.path(), Engine::VieNeu);
        assert!(!t.installed());
        assert_eq!(t.status().phase, "none");
        assert!(!t.speak("xin chào", DEFAULT_VOICE));
    }
}
