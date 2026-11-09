# Mini-Asi — tiến trình đọc giọng chạy nền: NghiTTS (Piper/VITS, github.com/nghimestudio/nghitts) qua `piper-tts`.
# Chạy hoàn toàn trên máy. Chỉ dùng giọng "thường" (không mang tên người nổi tiếng / giọng thương mại).
#
# Nạp mô hình 1 lần, nhận yêu cầu đọc qua stdin (JSON mỗi dòng), đọc từng câu ra file WAV rồi báo
# qua stdout để Mini-Asi phát ngay câu đầu trong khi câu sau đang được đọc.
#
#   vào : {"id": 3, "text": "...", "voice": "Mai Phương", "dir": "/tmp/x"} | {"cancel": 3} | {"quit": true}
#   ra  : {"ready": true} | {"id": 3, "i": 0, "file": "...", "last": false} | {"id": 3, "error": "..."}
#         {"fatal": "..."} (không nạp được) | {"warmup": "<file>"} (chạy với --warmup <file>: tải giọng TTS_VOICE rồi đọc thử)
#
# Mọi print() của thư viện đi sang stderr; stdout chỉ dành cho giao thức ở trên.
import hashlib
import json
import os
import queue
import re
import sys
import threading
import urllib.parse
import urllib.request

OUT = os.fdopen(os.dup(sys.stdout.fileno()), "w", buffering=1, encoding="utf-8")
sys.stdout = sys.stderr


def send(obj):
    OUT.write(json.dumps(obj, ensure_ascii=False) + "\n")
    OUT.flush()


def split_sentences(text, min_len=18, max_len=220):
    """Tách câu để đọc dần; gộp câu quá ngắn vào câu kế bên cho tự nhiên."""
    text = re.sub(r"\s+", " ", text).strip()
    if not text:
        return []
    parts = re.split(r"(?<=[.!?…;:])\s+|\n+", text)
    out, buf = [], ""
    for p in parts:
        p = p.strip()
        if not p:
            continue
        buf = (buf + " " + p).strip() if buf else p
        if len(buf) >= min_len:
            out.append(buf)
            buf = ""
    if buf:
        if out and len(buf) < min_len and len(out[-1]) + len(buf) < max_len:
            out[-1] = out[-1] + " " + buf
        else:
            out.append(buf)
    return out


# ───────────────────────────── NghiTTS (Piper) ─────────────────────────────
# Chỉ những giọng "thường" (không mang tên/giọng người nổi tiếng hay giọng thương mại).
# File tải từ máy chủ của NghiTTS, kiểm tra SHA-256 (cùng giá trị dự án wyoming-vietnamese ghim).
NGHI_BASE = "https://nghitts.app/api/model/"
NGHI_CONFIG_SHA = "971f57f8d504223fee5b40d664f503cf769baf7db21f7d2ae0554a75d07de2f8"
NGHI_VOICES = {
    "Mai Phương": "3d8384d05a0569be1a0a4da12d7582e7d0500b1f55139584e70f310dac92b01b",
    "Minh Quang": "0a559df4c4eaab442e0d2007f32888f87567900dfe1a4e9f1632f64070090e17",
}


def _sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def _slug(name):
    import unicodedata

    s = unicodedata.normalize("NFD", name)
    s = "".join(c for c in s if unicodedata.category(c) != "Mn").replace("đ", "d").replace("Đ", "D")
    return re.sub(r"[^A-Za-z0-9]+", "-", s).strip("-").lower()


def _download(url, dest, sha):
    if os.path.exists(dest) and _sha256(dest) == sha:
        return
    tmp = dest + ".part"
    req = urllib.request.Request(url, headers={"User-Agent": "Mini-Asi"})
    with urllib.request.urlopen(req, timeout=60) as r, open(tmp, "wb") as f:
        total, got, last = int(r.headers.get("Content-Length") or 0), 0, -1
        while True:
            b = r.read(1 << 16)
            if not b:
                break
            f.write(b)
            got += len(b)
            pct = got * 100 // total if total else -1
            if pct != last and pct % 5 == 0:
                print(f"{os.path.basename(dest)}: {pct}% ({got // 1_000_000} MB)", flush=True)
                last = pct
    if _sha256(tmp) != sha:
        os.remove(tmp)
        raise RuntimeError(f"File {os.path.basename(dest)} tải về bị sai (SHA-256 không khớp)")
    os.replace(tmp, dest)


def nghi_files(name, root):
    return os.path.join(root, _slug(name) + ".onnx"), os.path.join(root, _slug(name) + ".onnx.json")


def nghi_prepare(root, name):
    """Chỉ tải giọng được chọn (≈ 60 MB) — giọng còn lại tải khi người dùng chọn tới."""
    os.makedirs(root, exist_ok=True)
    name = name if name in NGHI_VOICES else "Mai Phương"
    model, cfg = nghi_files(name, root)
    q = urllib.parse.quote
    _download(NGHI_BASE + q(name + ".onnx.json", safe=""), cfg, NGHI_CONFIG_SHA)
    _download(NGHI_BASE + q(name + ".onnx", safe=""), model, NGHI_VOICES[name])


def vi_normalize(text):
    """Chuẩn hoá nhẹ cho bộ đọc Piper (eSpeak đọc số tiếng Việt được, nhưng không hiểu 15:30, 3h, %...)."""
    t = re.sub(r"[\U0001F300-\U0001FAFF☀-➿]", "", text)
    t = re.sub(r"\b(\d{1,2}):(\d{2})\b", lambda m: f"{int(m[1])} giờ {int(m[2])}" if m[2] != "00" else f"{int(m[1])} giờ", t)
    t = re.sub(r"\b(\d{1,2})h(\d{2})\b", lambda m: f"{int(m[1])} giờ {int(m[2])}", t)
    t = re.sub(r"\b(\d{1,2})h\b", r"\1 giờ", t)
    t = re.sub(r"(\d)\s*%", r"\1 phần trăm", t)
    t = re.sub(r"(\d)\s*GB\b", r"\1 ghi ga bai", t)
    t = re.sub(r"(\d)\s*MB\b", r"\1 mê ga bai", t)
    t = t.replace("&", " và ").replace("“", "").replace("”", "").replace('"', "")
    return re.sub(r"\s+", " ", t).strip()


class NghiBackend:
    default_voice = "Mai Phương"

    def __init__(self, warmup):
        self.root = os.environ.get("NGHI_DIR", "voices")
        if warmup:
            nghi_prepare(self.root, os.environ.get("TTS_VOICE", self.default_voice))
        from piper import PiperVoice  # noqa: F401  (kiểm tra cài đặt ngay khi khởi động)

        self.voices = {}

    def _available(self, name):
        return os.path.exists(nghi_files(name, self.root)[0])

    def _voice(self, name):
        name = name if name in NGHI_VOICES else self.default_voice
        if not self._available(name):  # giọng chưa tải → dùng giọng đã có
            name = next((n for n in NGHI_VOICES if self._available(n)), name)
        if name not in self.voices:
            from piper import PiperVoice

            model, cfg = nghi_files(name, self.root)
            self.voices[name] = PiperVoice.load(model, config_path=cfg)
        return self.voices[name]

    def say(self, text, voice, out):
        import wave

        v = self._voice(voice)
        with wave.open(out, "wb") as wf:
            v.synthesize_wav(vi_normalize(text), wf)


# ───────────────────────────── vòng lặp chung ─────────────────────────────
latest = {"id": -1}
cancelled = set()
jobs = queue.Queue()


def reader():
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            msg = json.loads(line)
        except Exception:
            continue
        if "cancel" in msg:
            cancelled.add(msg["cancel"])
            continue
        if msg.get("quit"):
            os._exit(0)
        latest["id"] = msg.get("id", 0)
        jobs.put(msg)
    os._exit(0)  # Mini-Asi đóng → tự thoát


def load_backend(warmup):
    return NghiBackend(warmup)


def main():
    warmup = "--warmup" in sys.argv
    threading.Thread(target=reader, daemon=True).start()
    try:
        backend = load_backend(warmup)
    except Exception as e:  # thiếu gói / chưa tải mô hình / mất mạng khi tải
        send({"fatal": f"{type(e).__name__}: {e}"})
        return
    if warmup:
        i = sys.argv.index("--warmup")
        out = sys.argv[i + 1] if len(sys.argv) > i + 1 else "warmup.wav"
        try:
            backend.say("Xin chào, mình là Asi.", os.environ.get("TTS_VOICE", ""), out)
            send({"warmup": out})
        except Exception as e:
            send({"fatal": f"{type(e).__name__}: {e}"})
        return
    send({"ready": True})
    while True:
        job = jobs.get()
        jid = job.get("id", 0)
        if jid != latest["id"] or jid in cancelled:
            continue  # đã có yêu cầu mới hơn
        sents = split_sentences(job.get("text", ""))
        os.makedirs(job.get("dir", "."), exist_ok=True)
        for i, s in enumerate(sents):
            if jid != latest["id"] or jid in cancelled:
                break
            try:
                f = os.path.join(job.get("dir", "."), f"asi-{jid}-{i}.wav")
                backend.say(s, job.get("voice") or "", f)
                send({"id": jid, "i": i, "file": f, "last": i == len(sents) - 1})
            except Exception as e:
                send({"id": jid, "error": f"{type(e).__name__}: {e}"})
                break
        if not sents:
            send({"id": jid, "i": 0, "file": "", "last": True})


if __name__ == "__main__":
    main()
