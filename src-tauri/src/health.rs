//! Theo dõi thời gian dùng máy liên tục → nhắc nghỉ mắt, uống nước, đi ngủ.
//! Logic thuần (không phụ thuộc hệ điều hành) để kiểm thử được.

use serde::Serialize;

/// Rời máy quá lâu này thì coi như đã nghỉ (giây)
pub const IDLE_RESET_SECS: f64 = 5.0 * 60.0;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum Nudge {
    Break { minutes: u32 },
    Water,
}

#[derive(Debug, Clone, Serialize)]
pub struct HealthState {
    /// Thời điểm bắt đầu phiên dùng máy liên tục (unix giây)
    pub session_start: i64,
    pub last_tick: i64,
    pub last_water: i64,
    pub snooze_until: i64,
    pub break_notified: bool,
    /// Tổng phút dùng máy hôm nay
    pub today_minutes: f64,
    pub today: u32,
    /// Bộ hẹn giờ tập trung (Pomodoro): thời điểm kết thúc
    pub focus_until: Option<i64>,
}

impl HealthState {
    pub fn new(now: i64, day: u32) -> Self {
        Self { session_start: now, last_tick: now, last_water: now, snooze_until: 0, break_notified: false, today_minutes: 0.0, today: day, focus_until: None }
    }

    pub fn session_minutes(&self, now: i64) -> u32 {
        ((now - self.session_start).max(0) / 60) as u32
    }

    /// Gọi định kỳ. `idle` = số giây người dùng không chạm máy (None nếu hệ điều hành không cho biết).
    pub fn tick(&mut self, now: i64, day: u32, idle: Option<f64>, break_min: u32, break_on: bool, water_min: u32, water_on: bool) -> Vec<Nudge> {
        let gap = now - self.last_tick;
        self.last_tick = now;
        if day != self.today {
            self.today = day;
            self.today_minutes = 0.0;
        }
        // Máy ngủ / gập máy (khoảng trống lớn giữa 2 lần tick) hoặc rời máy lâu → reset phiên
        let idle = idle.unwrap_or(0.0);
        if gap as f64 > IDLE_RESET_SECS || idle > IDLE_RESET_SECS {
            self.session_start = now;
            self.last_water = now;
            self.break_notified = false;
            return vec![];
        }
        if idle < 60.0 {
            self.today_minutes += gap as f64 / 60.0;
        }
        let mut out = vec![];
        if break_on && now >= self.snooze_until && !self.break_notified && self.session_minutes(now) >= break_min {
            self.break_notified = true;
            out.push(Nudge::Break { minutes: self.session_minutes(now) });
        }
        if water_on && (now - self.last_water) >= water_min as i64 * 60 {
            self.last_water = now;
            out.push(Nudge::Water);
        }
        out
    }

    /// Người dùng bấm "Đã nghỉ"
    pub fn took_break(&mut self, now: i64) {
        self.session_start = now;
        self.break_notified = false;
        self.snooze_until = 0;
    }

    /// Người dùng bấm "Để sau"
    pub fn snooze(&mut self, now: i64, minutes: i64) {
        self.snooze_until = now + minutes * 60;
        self.break_notified = false;
    }
}

pub fn is_sleep_time(hour: u32, sleep_hour: u32) -> bool {
    hour >= sleep_hour || hour < 5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn break_after_continuous_use() {
        let mut h = HealthState::new(0, 1);
        let mut t = 0;
        let mut got = vec![];
        while t < 51 * 60 {
            t += 30;
            got.extend(h.tick(t, 1, Some(2.0), 50, true, 999, false));
        }
        assert_eq!(got, vec![Nudge::Break { minutes: 50 }]);
        // không nhắc lặp lại
        assert!(h.tick(t + 30, 1, Some(1.0), 50, true, 999, false).is_empty());
        h.snooze(t + 30, 10);
        let mut later = vec![];
        let mut x = t + 30;
        while x < t + 30 + 10 * 60 {
            x += 30;
            later.extend(h.tick(x, 1, Some(1.0), 50, true, 999, false).into_iter().map(|n| (x - t - 30, n)));
        }
        // nhắc lại đúng 1 lần, khi hết 10 phút hoãn
        assert_eq!(later.len(), 1);
        assert_eq!(later[0].0, 600);
    }

    #[test]
    fn idle_or_sleep_resets_session() {
        let mut h = HealthState::new(0, 1);
        for i in 1..=80 {
            h.tick(i * 30, 1, Some(1.0), 50, true, 999, false);
        }
        assert!(h.session_minutes(40 * 60) >= 39);
        // rời máy 6 phút
        h.tick(40 * 60 + 30, 1, Some(6.0 * 60.0), 50, true, 999, false);
        assert_eq!(h.session_minutes(40 * 60 + 30), 0);
        // gập máy 2 tiếng (không có tick)
        let mut h2 = HealthState::new(0, 1);
        h2.tick(30 * 60, 1, None, 50, true, 999, false);
        assert_eq!(h2.session_minutes(30 * 60), 0);
    }

    #[test]
    fn water_reminder() {
        let mut h = HealthState::new(0, 1);
        let mut n = 0;
        for i in 1..=240 {
            n += h.tick(i * 30, 1, Some(1.0), 999, false, 60, true).iter().filter(|x| **x == Nudge::Water).count();
        }
        assert_eq!(n, 2); // 120 phút → 2 lần
    }

    #[test]
    fn sleep() {
        assert!(is_sleep_time(23, 23));
        assert!(is_sleep_time(2, 23));
        assert!(!is_sleep_time(9, 23));
    }
}
