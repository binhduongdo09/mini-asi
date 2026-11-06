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
        // FullAccess (= Authorized cũ) = 3
        if status.0 == 3 {
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
}

pub use imp::{create_event, ensure_access, events};
