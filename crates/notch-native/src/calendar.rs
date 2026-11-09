//! Lịch của máy (macOS: EventKit — đọc đúng cả sự kiện lặp lại, tạo sự kiện mới).

#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub id: String,
    pub title: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub all_day: bool,
    pub location: String,
    pub calendar: String,
}

/// Một lịch trên máy (iCloud, Google, Exchange… đã thêm trong Cài đặt hệ thống › Tài khoản Internet)
#[derive(Debug, Clone, PartialEq)]
pub struct CalendarInfo {
    pub id: String,
    pub title: String,
    /// Tên tài khoản: "iCloud", "Google" / email, "Trên máy"…
    pub source: String,
    pub default: bool,
}

/// Sự kiện mới cần tạo
#[derive(Debug, Clone, Default)]
pub struct NewEvent {
    pub title: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub all_day: bool,
    pub location: String,
    pub notes: String,
    /// "" = lịch mặc định
    pub calendar_id: String,
}

#[cfg(target_os = "macos")]
mod imp {
    #![allow(unused_unsafe)]
    use super::Event;
    use block2::RcBlock;
    use objc2::runtime::Bool;
    use objc2::sel;
    use objc2_event_kit::{EKAuthorizationStatus, EKEntityType, EKEvent, EKEventStore, EKSpan};
    use objc2_foundation::{NSDate, NSError, NSObjectProtocol, NSString};
    use std::sync::mpsc;
    use std::time::Duration;

    fn date(ms: i64) -> objc2::rc::Retained<NSDate> {
        unsafe { NSDate::dateWithTimeIntervalSince1970(ms as f64 / 1000.0) }
    }
    fn ms(d: &NSDate) -> i64 {
        unsafe { (d.timeIntervalSince1970() * 1000.0) as i64 }
    }

    /// Xin quyền đọc/ghi Lịch (hiện hộp thoại của macOS ở lần đầu).
    pub fn ensure_access() -> Result<(), String> {
        let status = unsafe { EKEventStore::authorizationStatusForEntityType(EKEntityType::Event) };
        // FullAccess (= Authorized cũ) = 3, WriteOnly = 4
        if status.0 == 3 || status.0 == 4 {
            return Ok(());
        }
        if status == EKAuthorizationStatus::Denied || status == EKAuthorizationStatus::Restricted {
            return Err("Chưa cấp quyền Lịch — vào Cài đặt hệ thống › Quyền riêng tư › Lịch".into());
        }
        let store = unsafe { EKEventStore::new() };
        let (tx, rx) = mpsc::channel();
        let block = RcBlock::new(move |granted: Bool, _err: *mut NSError| {
            let _ = tx.send(granted.as_bool());
        });
        unsafe {
            if store.respondsToSelector(sel!(requestFullAccessToEventsWithCompletion:)) {
                store.requestFullAccessToEventsWithCompletion(RcBlock::as_ptr(&block));
            } else {
                #[allow(deprecated)]
                store.requestAccessToEntityType_completion(EKEntityType::Event, RcBlock::as_ptr(&block));
            }
        }
        match rx.recv_timeout(Duration::from_secs(120)) {
            Ok(true) => Ok(()),
            _ => Err("Bạn chưa cho phép Mini-Asi xem Lịch".into()),
        }
    }

    pub fn events(start_ms: i64, end_ms: i64) -> Result<Vec<Event>, String> {
        ensure_access()?;
        unsafe {
            let store = EKEventStore::new();
            let pred = store.predicateForEventsWithStartDate_endDate_calendars(&date(start_ms), &date(end_ms), None);
            let list = store.eventsMatchingPredicate(&pred);
            let mut out = Vec::new();
            for ev in list.iter() {
                out.push(Event {
                    id: ev.eventIdentifier().map(|s| s.to_string()).unwrap_or_default(),
                    title: ev.title().to_string(),
                    start_ms: ms(&ev.startDate()),
                    end_ms: ms(&ev.endDate()),
                    all_day: ev.isAllDay(),
                    location: ev.location().map(|s| s.to_string()).unwrap_or_default(),
                    calendar: ev.calendar().map(|c| c.title().to_string()).unwrap_or_default(),
                });
            }
            out.sort_by_key(|e| e.start_ms);
            Ok(out)
        }
    }

    /// Các lịch ghi được (để chọn lịch đích: iCloud, Google…)
    pub fn calendars() -> Result<Vec<super::CalendarInfo>, String> {
        ensure_access()?;
        unsafe {
            let store = EKEventStore::new();
            let def = store.defaultCalendarForNewEvents().map(|c| c.calendarIdentifier().to_string()).unwrap_or_default();
            let mut out = vec![];
            for c in store.calendarsForEntityType(EKEntityType::Event).iter() {
                if !c.allowsContentModifications() {
                    continue;
                }
                let id = c.calendarIdentifier().to_string();
                out.push(super::CalendarInfo {
                    default: id == def,
                    id,
                    title: c.title().to_string(),
                    source: c.source().map(|s| s.title().to_string()).unwrap_or_default(),
                });
            }
            out.sort_by(|a, b| b.default.cmp(&a.default).then(a.source.cmp(&b.source)).then(a.title.cmp(&b.title)));
            Ok(out)
        }
    }

    /// Tạo sự kiện đầy đủ (địa điểm, ghi chú, cả ngày, lịch đích). Lịch Google trên máy sẽ tự đồng bộ lên Google Calendar.
    pub fn create_event_full(e: &super::NewEvent) -> Result<String, String> {
        ensure_access()?;
        unsafe {
            let store = EKEventStore::new();
            let cal = if e.calendar_id.is_empty() { None } else { store.calendarWithIdentifier(&NSString::from_str(&e.calendar_id)) };
            let cal = cal.or_else(|| store.defaultCalendarForNewEvents()).ok_or("Chưa có lịch nào trên máy")?;
            let ev = EKEvent::eventWithEventStore(&store);
            ev.setTitle(Some(&NSString::from_str(&e.title)));
            ev.setStartDate(Some(&date(e.start_ms)));
            ev.setEndDate(Some(&date(e.end_ms.max(e.start_ms + 60_000))));
            ev.setAllDay(e.all_day);
            if !e.location.trim().is_empty() {
                ev.setLocation(Some(&NSString::from_str(&e.location)));
            }
            if !e.notes.trim().is_empty() {
                ev.setNotes(Some(&NSString::from_str(&e.notes)));
            }
            ev.setCalendar(Some(&cal));
            store
                .saveEvent_span_commit_error(&ev, EKSpan::ThisEvent, true)
                .map_err(|e| format!("Không tạo được sự kiện: {}", e.localizedDescription()))?;
            Ok(ev.eventIdentifier().map(|s| s.to_string()).unwrap_or_default())
        }
    }

    pub fn create_event(title: &str, start_ms: i64, end_ms: i64) -> Result<String, String> {
        ensure_access()?;
        unsafe {
            let store = EKEventStore::new();
            let cal = store.defaultCalendarForNewEvents().ok_or("Chưa có lịch mặc định trên máy")?;
            let ev = EKEvent::eventWithEventStore(&store);
            ev.setTitle(Some(&NSString::from_str(title)));
            ev.setStartDate(Some(&date(start_ms)));
            ev.setEndDate(Some(&date(end_ms)));
            ev.setCalendar(Some(&cal));
            store
                .saveEvent_span_commit_error(&ev, EKSpan::ThisEvent, true)
                .map_err(|e| format!("Không tạo được sự kiện: {}", e.localizedDescription()))?;
            Ok(ev.eventIdentifier().map(|s| s.to_string()).unwrap_or_default())
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::Event;
    pub fn ensure_access() -> Result<(), String> {
        Err("Đồng bộ Lịch hiện chỉ có trên macOS".into())
    }
    pub fn events(_s: i64, _e: i64) -> Result<Vec<Event>, String> {
        ensure_access().map(|_| vec![])
    }
    pub fn create_event(_t: &str, _s: i64, _e: i64) -> Result<String, String> {
        ensure_access().map(|_| String::new())
    }
    pub fn calendars() -> Result<Vec<super::CalendarInfo>, String> {
        ensure_access().map(|_| vec![])
    }
    pub fn create_event_full(_e: &super::NewEvent) -> Result<String, String> {
        ensure_access().map(|_| String::new())
    }
}

pub use imp::{calendars, create_event, create_event_full, ensure_access, events};
