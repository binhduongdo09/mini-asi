//! Thu âm từ micro (cpal — chạy được trên macOS / Windows / Linux) rồi ghi ra WAV để nhận dạng.
//! Tự dừng khi người dùng im lặng ~2 giây sau khi đã nói, hoặc sau 2 phút.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub struct Recorder {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<Result<(Vec<f32>, u32), String>>>,
}

const MAX_SECS: u64 = 120;
const SILENCE_STOP: Duration = Duration::from_millis(2200);
const SPEECH_RMS: f32 = 0.018;

/// `on_level(rms 0..1, giây đã thu)` được gọi ~10 lần/giây; `on_end` khi tự dừng.
pub fn start(on_level: impl Fn(f32, f32) + Send + 'static, on_end: impl Fn() + Send + 'static) -> Result<Recorder, String> {
    let stop = Arc::new(AtomicBool::new(false));
    let stop2 = stop.clone();
    let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
    let handle = std::thread::spawn(move || {
        let fail = |e: String, tx: &mpsc::Sender<Result<(), String>>| {
            let _ = tx.send(Err(e.clone()));
            Err(e)
        };
        let host = cpal::default_host();
        let Some(dev) = host.default_input_device() else { return fail("Không tìm thấy micro".into(), &ready_tx) };
        let cfg = match dev.default_input_config() {
            Ok(c) => c,
            Err(e) => return fail(format!("Không mở được micro: {e}"), &ready_tx),
        };
        let channels = cfg.channels() as usize;
        let rate = cfg.sample_rate();
        let buf: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::with_capacity(rate as usize * 30)));
        let err_fn = |e| eprintln!("Lỗi micro: {e}");
        let sc: cpal::StreamConfig = cfg.into();
        macro_rules! build {
            ($t:ty, $conv:expr) => {{
                let b = buf.clone();
                dev.build_input_stream(
                    sc.clone(),
                    move |data: &[$t], _: &cpal::InputCallbackInfo| {
                        let mut v = b.lock().unwrap();
                        for frame in data.chunks(channels.max(1)) {
                            let s: f32 = frame.iter().map(|x| $conv(*x)).sum::<f32>() / frame.len() as f32;
                            v.push(s);
                        }
                    },
                    err_fn,
                    None,
                )
            }};
        }
        let stream = match cfg.sample_format() {
            cpal::SampleFormat::F32 => build!(f32, |x: f32| x),
            cpal::SampleFormat::I16 => build!(i16, |x: i16| x as f32 / 32768.0),
            cpal::SampleFormat::U16 => build!(u16, |x: u16| (x as f32 - 32768.0) / 32768.0),
            cpal::SampleFormat::I32 => build!(i32, |x: i32| x as f32 / 2147483648.0),
            f => return fail(format!("Định dạng micro chưa hỗ trợ: {f:?}"), &ready_tx),
        };
        let stream = match stream {
            Ok(s) => s,
            Err(e) => return fail(format!("Không thu âm được (đã cấp quyền Micro chưa?): {e}"), &ready_tx),
        };
        if let Err(e) = stream.play() {
            return fail(format!("Không bật được micro: {e}"), &ready_tx);
        }
        let _ = ready_tx.send(Ok(()));
        let t0 = Instant::now();
        let mut read = 0usize;
        let mut spoke = false;
        let mut last_voice = Instant::now();
        let mut auto = false;
        while !stop2.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(100));
            let (rms, total) = {
                let v = buf.lock().unwrap();
                let chunk = &v[read.min(v.len())..];
                let rms = if chunk.is_empty() { 0.0 } else { (chunk.iter().map(|x| x * x).sum::<f32>() / chunk.len() as f32).sqrt() };
                (rms, v.len())
            };
            read = total;
            on_level((rms * 6.0).min(1.0), t0.elapsed().as_secs_f32());
            if rms > SPEECH_RMS {
                spoke = true;
                last_voice = Instant::now();
            }
            if (spoke && last_voice.elapsed() > SILENCE_STOP) || t0.elapsed().as_secs() >= MAX_SECS {
                auto = true;
                break;
            }
        }
        drop(stream);
        if auto {
            on_end();
        }
        let samples = std::mem::take(&mut *buf.lock().unwrap());
        Ok((samples, rate))
    });
    match ready_rx.recv_timeout(Duration::from_secs(20)) {
        Ok(Ok(())) => Ok(Recorder { stop, handle: Some(handle) }),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("Micro không phản hồi".into()),
    }
}

impl Recorder {
    pub fn finish(mut self) -> Result<(Vec<f32>, u32), String> {
        self.stop.store(true, Ordering::Relaxed);
        self.handle.take().ok_or("Đã dừng")?.join().map_err(|_| "Luồng thu âm bị lỗi".to_string())?
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

pub fn write_wav(path: &Path, samples: &[f32], rate: u32) -> Result<(), String> {
    let spec = hound::WavSpec { channels: 1, sample_rate: rate, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut w = hound::WavWriter::create(path, spec).map_err(|e| e.to_string())?;
    for s in samples {
        w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16).map_err(|e| e.to_string())?;
    }
    w.finalize().map_err(|e| e.to_string())
}

/// Có tiếng nói không (tránh gửi file im lặng đi nhận dạng)
pub fn has_speech(samples: &[f32], rate: u32) -> bool {
    let win = (rate as usize / 10).max(1);
    samples.chunks(win).any(|c| (c.iter().map(|x| x * x).sum::<f32>() / c.len() as f32).sqrt() > SPEECH_RMS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_and_speech_detection() {
        let rate = 16_000;
        let silence = vec![0.0f32; rate as usize];
        assert!(!has_speech(&silence, rate));
        let tone: Vec<f32> = (0..rate).map(|i| (i as f32 * 0.05).sin() * 0.3).collect();
        assert!(has_speech(&tone, rate));
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("a.wav");
        write_wav(&p, &tone, rate).unwrap();
        let r = hound::WavReader::open(&p).unwrap();
        assert_eq!(r.spec().sample_rate, rate);
        assert_eq!(r.len(), rate);
    }
}
