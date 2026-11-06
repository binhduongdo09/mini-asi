//! Học từ mới tiếng Anh: mỗi lần mở tai thỏ Asi đưa 2–3 từ, kèm phiên âm, nghĩa, ví dụ và giọng đọc chuẩn.
//!
//! - Danh sách từ có sẵn trong app (`resources/words_en.tsv`: từ · loại từ · nghĩa tiếng Việt).
//! - Phiên âm IPA, file phát âm US/UK, định nghĩa và ví dụ lấy online từ Free Dictionary API
//!   (dictionaryapi.dev, miễn phí, không cần khoá), lưu cache để lần sau dùng offline.
//! - Kiểm tra phát âm: người dùng đọc vào micro → nhận dạng giọng nói tiếng Anh → so với từ cần đọc
//!   (`score()` thuần, kiểm thử được).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const WORDS_TSV: &str = include_str!("../resources/words_en.tsv");
const DICT_API: &str = "https://api.dictionaryapi.dev/api/v2/entries/en/";

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Word {
    pub word: String,
    pub pos: String,
    pub vi: String,
}

/// Thông tin lấy online (có thể thiếu nếu mất mạng)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Details {
    pub ipa_us: Option<String>,
    pub ipa_uk: Option<String>,
    pub audio_us: Option<String>,
    pub audio_uk: Option<String>,
    pub definition: Option<String>,
    pub example: Option<String>,
    pub fetched: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Progress {
    pub shown: u32,
    pub tries: u32,
    pub correct: u32,
    pub best: u32,
    pub learned: bool,
    pub last_seen: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct VocabState {
    pub progress: HashMap<String, Progress>,
    pub cache: HashMap<String, Details>,
    /// bộ từ đang hiện
    pub current: Vec<String>,
    /// vị trí trong vòng xoay danh sách
    pub cursor: usize,
    /// hạt giống xáo thứ tự từ (mỗi máy một thứ tự, không đi theo bảng chữ cái)
    pub seed: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Card {
    #[serde(flatten)]
    pub word: Word,
    pub details: Details,
    pub progress: Progress,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Check {
    /// 0–100
    pub score: u32,
    /// great | close | retry
    pub verdict: String,
    pub heard: String,
    pub tip: String,
}

pub fn word_list() -> Vec<Word> {
    let mut seen = std::collections::HashSet::new();
    WORDS_TSV
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .filter_map(|l| {
            let mut it = l.split('\t');
            let (w, pos, vi) = (it.next()?.trim(), it.next()?.trim(), it.next()?.trim());
            seen.insert(w.to_lowercase()).then(|| Word { word: w.into(), pos: pos.into(), vi: vi.into() })
        })
        .collect()
}

pub struct Vocab {
    file: PathBuf,
    pub state: VocabState,
    words: Vec<Word>,
}

impl Vocab {
    pub fn open(dir: &Path) -> Self {
        let file = dir.join("vocab.json");
        let mut state: VocabState = std::fs::read_to_string(&file).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        if state.seed == 0 {
            state.seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1) | 1;
        }
        let mut words = word_list();
        let seed = state.seed;
        words.sort_by_key(|w| mix(seed, &w.word));
        Self { file, state, words }
    }

    pub fn save(&self) {
        let tmp = self.file.with_extension("json.tmp");
        if std::fs::write(&tmp, serde_json::to_string(&self.state).unwrap_or_default()).is_ok() {
            let _ = std::fs::rename(&tmp, &self.file);
        }
    }

    fn find(&self, w: &str) -> Option<&Word> {
        self.words.iter().find(|x| x.word.eq_ignore_ascii_case(w))
    }

    /// Chọn `n` từ mới tiếp theo (bỏ qua từ đã thuộc), xoay vòng hết danh sách rồi quay lại từ đầu.
    pub fn next_batch(&mut self, n: usize, now: i64) -> Vec<String> {
        let total = self.words.len();
        let mut out = vec![];
        let mut steps = 0;
        while out.len() < n.min(total) && steps < total * 2 {
            let w = &self.words[self.state.cursor % total];
            self.state.cursor = (self.state.cursor + 1) % total;
            steps += 1;
            let learned = self.state.progress.get(&w.word).is_some_and(|p| p.learned);
            if (!learned || steps > total) && !out.contains(&w.word) {
                out.push(w.word.clone());
            }
        }
        for w in &out {
            let p = self.state.progress.entry(w.clone()).or_default();
            p.shown += 1;
            p.last_seen = now;
        }
        self.state.current = out.clone();
        self.save();
        out
    }

    /// Bộ từ đang hiện; `fresh` = lần mở mới → đổi sang từ khác
    pub fn batch(&mut self, n: usize, fresh: bool, now: i64) -> Vec<String> {
        if fresh || self.state.current.len() != n || self.state.current.iter().any(|w| self.find(w).is_none()) {
            self.next_batch(n, now)
        } else {
            self.state.current.clone()
        }
    }

    pub fn card(&self, w: &str) -> Option<Card> {
        let word = self.find(w)?.clone();
        Some(Card {
            details: self.state.cache.get(&word.word).cloned().unwrap_or_default(),
            progress: self.state.progress.get(&word.word).cloned().unwrap_or_default(),
            word,
        })
    }

    pub fn needs_fetch(&self, w: &str) -> bool {
        self.state.cache.get(w).is_none_or(|d| d.ipa_us.is_none() && d.ipa_uk.is_none() && d.definition.is_none())
    }

    pub fn record(&mut self, w: &str, c: &Check) {
        let p = self.state.progress.entry(w.to_string()).or_default();
        p.tries += 1;
        p.best = p.best.max(c.score);
        if c.verdict == "great" {
            p.correct += 1;
            if p.correct >= 2 {
                p.learned = true; // đọc chuẩn 2 lần → coi như đã thuộc
            }
        }
        self.save();
    }

    pub fn set_learned(&mut self, w: &str, learned: bool) {
        self.state.progress.entry(w.to_string()).or_default().learned = learned;
        self.save();
    }

    pub fn stats(&self) -> (usize, usize) {
        (self.state.progress.values().filter(|p| p.learned).count(), self.words.len())
    }
}

/// Băm đơn giản (FNV-1a) để xáo thứ tự từ ổn định theo hạt giống
fn mix(seed: u64, w: &str) -> u64 {
    let mut h = 0xcbf29ce484222325u64 ^ seed;
    for b in w.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Tra từ trên Free Dictionary API (chạy ở luồng nền)
pub fn fetch(word: &str, now: i64) -> Result<Details, String> {
    let url = format!("{DICT_API}{}", word.to_lowercase());
    let v: serde_json::Value = ureq::get(&url).timeout(std::time::Duration::from_secs(8)).call().map_err(|e| format!("Không tra được từ: {e}"))?.into_json().map_err(|e| e.to_string())?;
    Ok(parse_entry(&v, now))
}

/// Đọc JSON của dictionaryapi.dev → phiên âm, file phát âm US/UK, định nghĩa & ví dụ đầu tiên
pub fn parse_entry(v: &serde_json::Value, now: i64) -> Details {
    let mut d = Details { fetched: now, ..Default::default() };
    let entries = v.as_array().cloned().unwrap_or_default();
    for e in &entries {
        for p in e.get("phonetics").and_then(|x| x.as_array()).into_iter().flatten() {
            let text = p.get("text").and_then(|x| x.as_str()).filter(|s| !s.is_empty()).map(String::from);
            let audio = p.get("audio").and_then(|x| x.as_str()).filter(|s| !s.is_empty()).map(String::from);
            let tag = audio.as_deref().unwrap_or("");
            if tag.contains("-us.") || tag.contains("_us_") {
                d.audio_us = d.audio_us.or(audio.clone());
                d.ipa_us = d.ipa_us.or(text.clone());
            } else if tag.contains("-uk.") || tag.contains("-gb.") || tag.contains("_gb_") {
                d.audio_uk = d.audio_uk.or(audio.clone());
                d.ipa_uk = d.ipa_uk.or(text.clone());
            } else if text.is_some() && d.ipa_us.is_none() && d.ipa_uk.is_none() {
                d.ipa_us = text.clone();
            }
        }
        if d.ipa_us.is_none() && d.ipa_uk.is_none() {
            d.ipa_us = e.get("phonetic").and_then(|x| x.as_str()).map(String::from);
        }
        for m in e.get("meanings").and_then(|x| x.as_array()).into_iter().flatten() {
            for df in m.get("definitions").and_then(|x| x.as_array()).into_iter().flatten() {
                if d.definition.is_none() {
                    d.definition = df.get("definition").and_then(|x| x.as_str()).map(String::from);
                }
                if d.example.is_none() {
                    d.example = df.get("example").and_then(|x| x.as_str()).map(String::from);
                }
            }
        }
    }
    d
}

fn edit_distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + usize::from(a[i - 1] != b[j - 1]));
        }
        prev = cur;
    }
    prev[b.len()]
}

/// So từ cần đọc với những gì máy nghe được
pub fn score(target: &str, heard: &str) -> Check {
    let norm = |s: &str| s.to_lowercase().chars().map(|c| if c.is_ascii_alphabetic() || c == '\'' { c } else { ' ' }).collect::<String>();
    let t = norm(target).trim().to_string();
    let h = norm(heard);
    let tokens: Vec<&str> = h.split_whitespace().collect();
    let best = tokens
        .iter()
        .map(|w| {
            let dist = edit_distance(&t, w);
            let len = t.chars().count().max(w.chars().count()).max(1);
            (100 - (dist * 100 / len).min(100)) as u32
        })
        .max()
        .unwrap_or(0);
    let heard_s = heard.trim().to_string();
    let (verdict, tip) = if tokens.iter().any(|w| *w == t) || best >= 92 {
        ("great", "Chuẩn rồi! 🎉".to_string())
    } else if best >= 70 {
        ("close", format!("Gần đúng — Asi nghe thành “{heard_s}”. Nghe mẫu rồi đọc chậm, rõ từng âm tiết nhé."))
    } else if heard_s.is_empty() {
        ("retry", "Asi chưa nghe rõ — đọc to và gần micro hơn nhé.".to_string())
    } else {
        ("retry", format!("Chưa chuẩn — Asi nghe thành “{heard_s}”. Thử nghe mẫu và đọc lại nhé."))
    };
    Check { score: if verdict == "great" { best.max(95) } else { best }, verdict: verdict.into(), heard: heard_s, tip }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_is_clean() {
        let l = word_list();
        assert!(l.len() >= 150, "{}", l.len());
        assert!(l.iter().all(|w| !w.vi.is_empty() && !w.word.contains(' ')));
        let mut names: Vec<String> = l.iter().map(|w| w.word.to_lowercase()).collect();
        names.dedup();
        assert_eq!(names.len(), l.len(), "không trùng từ");
    }

    #[test]
    fn batches_rotate_and_skip_learned() {
        let d = tempfile::tempdir().unwrap();
        let mut v = Vocab::open(d.path());
        let a = v.batch(3, false, 1);
        assert_eq!(a.len(), 3);
        assert_eq!(v.batch(3, false, 2), a, "chưa mở lại → giữ nguyên");
        v.set_learned(&a[0].clone(), true);
        let b = v.batch(3, true, 3);
        assert!(b.iter().all(|w| !a.contains(w)), "lần mở mới → từ khác");
        // mở lại file → nhớ tiến độ
        let v2 = Vocab::open(d.path());
        assert!(v2.state.progress[&a[0]].learned);
        assert_eq!(v2.state.current, b);
    }

    #[test]
    fn pronunciation_scores() {
        assert_eq!(score("schedule", "Schedule.").verdict, "great");
        assert_eq!(score("entrepreneur", "I said entrepreneur").verdict, "great");
        let c = score("thorough", "throw");
        assert_ne!(c.verdict, "great");
        assert_eq!(score("budget", "budge it").verdict, "close");
        assert_eq!(score("vegetable", "").verdict, "retry");
    }

    #[test]
    fn parses_dictionary_api() {
        let v: serde_json::Value = serde_json::from_str(
            r#"[{"word":"schedule","phonetic":"/ˈʃɛdjuːl/","phonetics":[{"text":"/ˈʃɛdjuːl/","audio":"https://api.dictionaryapi.dev/media/pronunciations/en/schedule-uk.mp3"},{"text":"/ˈskɛdʒuːl/","audio":"https://api.dictionaryapi.dev/media/pronunciations/en/schedule-us.mp3"}],
            "meanings":[{"partOfSpeech":"noun","definitions":[{"definition":"A plan of events.","example":"I have a busy schedule."}]}]}]"#,
        )
        .unwrap();
        let d = parse_entry(&v, 5);
        assert_eq!(d.ipa_us.as_deref(), Some("/ˈskɛdʒuːl/"));
        assert_eq!(d.ipa_uk.as_deref(), Some("/ˈʃɛdjuːl/"));
        assert!(d.audio_us.unwrap().ends_with("schedule-us.mp3"));
        assert_eq!(d.example.as_deref(), Some("I have a busy schedule."));
    }
}
