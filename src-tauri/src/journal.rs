//! Nhật ký: mỗi tối Mini-Asi hỏi chuyện về ngày hôm nay, ghi lại câu trả lời, tâm trạng
//! và những điều máy tự biết (việc đã xong, cuộc họp, thời gian dùng máy...).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Qa {
    pub q: String,
    pub a: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct DayFacts {
    pub tasks_done: Vec<String>,
    pub tasks_open: usize,
    pub tasks_late: usize,
    pub events: Vec<String>,
    pub screen_minutes: u32,
    pub notes_written: usize,
    pub clips: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Entry {
    /// "YYYY-MM-DD" theo giờ địa phương
    pub date: String,
    /// 1 (tệ) … 5 (tuyệt)
    pub mood: Option<u8>,
    pub answers: Vec<Qa>,
    pub text: String,
    pub facts: DayFacts,
    pub updated: i64,
}

/// 3 câu hỏi ngắn, thiên về cảm xúc. Mỗi câu kèm gợi ý "kể 2–3 câu thôi".
pub fn questions(lang: &str) -> Vec<(&'static str, &'static str)> {
    if lang.starts_with("en") {
        vec![
            ("How did today feel? 🌤", "Just 2–3 sentences — good or tiring, both are okay."),
            ("What made you smile today?", "Even a tiny moment counts."),
            ("Anything still weighing on you?", "Let it out so you can rest — or skip if nothing."),
        ]
    } else {
        vec![
            ("Hôm nay bạn thấy thế nào? 🌤", "Kể Asi nghe 2–3 câu thôi — vui hay mệt đều được."),
            ("Điều gì làm bạn mỉm cười hôm nay?", "Một khoảnh khắc nhỏ thôi cũng được."),
            ("Có điều gì còn làm bạn trăn trở không?", "Nói ra cho nhẹ lòng nhé — không có thì bỏ qua."),
        ]
    }
}

/// Câu kết: báo đã ghi nhật ký; chúc ngủ ngon nếu đã khuya, còn sớm thì chúc buổi tối nhẹ nhàng
pub fn closing(lang: &str, hour: u32, answered: bool) -> String {
    let en = lang.starts_with("en");
    if !answered {
        return if en { "No worries — come back and write whenever you feel like it.".into() } else { "Không sao đâu, khi nào muốn kể thì quay lại với Asi nhé.".into() };
    }
    let late = !(4..21).contains(&hour);
    match (en, late) {
        (false, true) => "Mình sẽ ghi lại nhật ký ngày hôm nay của bạn nhé. Chúc bạn ngủ ngon 🌙".into(),
        (false, false) => format!("Mình sẽ ghi lại nhật ký ngày hôm nay của bạn nhé. Chúc bạn {} thật nhẹ nhàng ✨", if hour < 12 { "một ngày" } else if hour < 18 { "buổi chiều" } else { "buổi tối" }),
        (true, true) => "I'll keep today's journal for you. Sleep well 🌙".into(),
        (true, false) => "I'll keep today's journal for you. Have a gentle rest of your day ✨".into(),
    }
}

pub struct Journal {
    file: PathBuf,
    pub entries: Vec<Entry>,
}

impl Journal {
    pub fn open(dir: &Path) -> Self {
        let file = dir.join("journal.json");
        let entries = std::fs::read_to_string(&file).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        Self { file, entries }
    }

    fn save(&self) -> Result<(), String> {
        let tmp = self.file.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(&self.entries).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &self.file).map_err(|e| e.to_string())
    }

    pub fn get(&self, date: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.date == date)
    }

    pub fn upsert(&mut self, mut e: Entry, now: i64) -> Result<Entry, String> {
        e.updated = now;
        match self.entries.iter_mut().find(|x| x.date == e.date) {
            Some(x) => *x = e.clone(),
            None => self.entries.push(e.clone()),
        }
        self.entries.sort_by(|a, b| b.date.cmp(&a.date));
        self.save()?;
        Ok(e)
    }

    pub fn delete(&mut self, date: &str) -> Result<(), String> {
        self.entries.retain(|e| e.date != date);
        self.save()
    }

    pub fn has_written(&self, date: &str) -> bool {
        self.get(date).is_some_and(|e| e.answers.iter().any(|a| !a.a.trim().is_empty()) || !e.text.trim().is_empty())
    }

    /// Chuỗi ngày viết nhật ký liên tục tính đến `today` (hoặc hôm qua nếu hôm nay chưa viết)
    pub fn streak(&self, today: chrono::NaiveDate) -> u32 {
        let mut d = today;
        if !self.has_written(&d.to_string()) {
            d = d.pred_opt().unwrap_or(d);
        }
        let mut n = 0;
        while self.has_written(&d.to_string()) {
            n += 1;
            d = match d.pred_opt() {
                Some(x) => x,
                None => break,
            };
        }
        n
    }

    pub fn to_markdown(&self) -> String {
        const MOODS: [&str; 6] = ["", "😢", "😕", "😐", "🙂", "😄"];
        let mut s = String::from("# Nhật ký Mini-Asi\n\n");
        for e in &self.entries {
            s.push_str(&format!("## {} {}\n\n", e.date, e.mood.map(|m| MOODS[(m as usize).min(5)]).unwrap_or("")));
            let f = &e.facts;
            if !f.tasks_done.is_empty() {
                s.push_str(&format!("- ✅ Đã xong {} việc: {}\n", f.tasks_done.len(), f.tasks_done.join(", ")));
            }
            if !f.events.is_empty() {
                s.push_str(&format!("- 📅 {}\n", f.events.join(" · ")));
            }
            if f.screen_minutes > 0 {
                s.push_str(&format!("- ⏱ Dùng máy {} giờ {} phút\n", f.screen_minutes / 60, f.screen_minutes % 60));
            }
            s.push('\n');
            for qa in e.answers.iter().filter(|q| !q.a.trim().is_empty()) {
                s.push_str(&format!("**{}**\n\n{}\n\n", qa.q, qa.a.trim()));
            }
            if !e.text.trim().is_empty() {
                s.push_str(&format!("{}\n\n", e.text.trim()));
            }
        }
        s
    }
}

/// Tách câu trả lời "Ngày mai muốn làm gì" thành danh sách việc
pub fn split_plans(text: &str) -> Vec<String> {
    text.split(['\n', ';', ','])
        .flat_map(|s| s.split(" và ").map(str::to_string).collect::<Vec<_>>())
        .flat_map(|s| s.split(" and ").map(str::to_string).collect::<Vec<_>>())
        .map(|s| s.trim().trim_start_matches(|c: char| c == '-' || c == '•' || c == '*' || c.is_ascii_digit() || c == '.' || c == ')').trim().to_string())
        .filter(|s| s.chars().count() >= 2)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_emotional_questions_and_closing() {
        let q = questions("vi-VN");
        assert_eq!(q.len(), 3);
        assert!(q.iter().all(|(_, h)| !h.is_empty()));
        assert!(closing("vi-VN", 22, true).contains("ngủ ngon"));
        assert!(closing("vi-VN", 2, true).contains("ngủ ngon"));
        let early = closing("vi-VN", 19, true);
        assert!(early.starts_with("Mình sẽ ghi lại nhật ký ngày hôm nay của bạn nhé") && !early.contains("ngủ ngon"), "{early}");
        assert!(!closing("vi-VN", 22, false).contains("ghi lại"));
    }

    #[test]
    fn journal_roundtrip_streak_markdown() {
        let d = tempfile::tempdir().unwrap();
        let mut j = Journal::open(d.path());
        let day = |s: &str| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap();
        for date in ["2026-11-02", "2026-11-03", "2026-11-04"] {
            j.upsert(Entry { date: date.into(), mood: Some(4), answers: vec![Qa { q: "Hôm nay thế nào?".into(), a: "Ổn".into() }], ..Default::default() }, 1).unwrap();
        }
        assert_eq!(j.streak(day("2026-11-04")), 3);
        assert_eq!(j.streak(day("2026-11-05")), 3, "hôm nay chưa viết vẫn giữ chuỗi");
        assert_eq!(j.streak(day("2026-11-07")), 0);
        let j2 = Journal::open(d.path());
        assert_eq!(j2.entries.len(), 3);
        assert_eq!(j2.entries[0].date, "2026-11-04", "mới nhất lên đầu");
        assert!(j2.to_markdown().contains("## 2026-11-04 🙂"));
    }

    #[test]
    fn plans() {
        assert_eq!(split_plans("- gọi khách 9h\n- nộp báo cáo, đi gym và mua hoa"), vec!["gọi khách 9h", "nộp báo cáo", "đi gym", "mua hoa"]);
        assert_eq!(split_plans("1. call John; 2. gym and read"), vec!["call John", "gym", "read"]);
    }
}
