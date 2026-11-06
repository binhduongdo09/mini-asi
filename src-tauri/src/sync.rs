//! Đồng bộ 2 chiều với app Apple (Lời nhắc ↔ Việc, Ghi chú ↔ Notes).
//! `plan()` là logic thuần: so sánh 2 phía và quyết định việc cần làm → kiểm thử được.

#[derive(Debug, Clone)]
pub struct Local {
    pub id: u64,
    pub ext_id: Option<String>,
    pub updated: i64,
    /// Đã từng thấy mục này bên Apple ở một lần kéo về trước đó (mới chắc chắn là "bên Apple đã xoá")
    pub seen: bool,
    /// Tiêu đề — để nối lại khi id bên Apple bị lệch
    pub title: String,
}

#[derive(Debug, Clone)]
pub struct Remote {
    pub id: String,
    pub modified: i64,
    /// Lời nhắc đã hoàn thành lâu rồi thì không kéo về
    pub stale_done: bool,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Plan {
    /// Mục mới ở Mini-Asi → tạo bên Apple
    PushNew(u64),
    /// Mini-Asi sửa sau → ghi đè bên Apple
    PushUpdate(u64, String),
    /// Mục mới bên Apple → tạo ở Mini-Asi
    PullNew(String),
    /// Bên Apple sửa sau → cập nhật Mini-Asi
    PullUpdate(u64, String),
    /// Bên Apple đã xoá (và Mini-Asi không sửa gì) → xoá ở Mini-Asi
    DeleteLocal(u64),
    /// Mini-Asi đã xoá → xoá bên Apple
    DeleteRemote(String),
    /// id bên Apple bị lệch nhưng tìm được mục cùng tiêu đề → nối lại, không xoá, không tạo trùng
    Relink(u64, String),
}

fn norm(t: &str) -> String {
    t.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

pub fn plan(locals: &[Local], remotes: &[Remote], last_sync: i64, tombstones: &[String]) -> Vec<Plan> {
    use std::collections::{HashMap, HashSet};
    let rmap: HashMap<&str, &Remote> = remotes.iter().map(|r| (r.id.as_str(), r)).collect();
    let tomb: HashSet<&str> = tombstones.iter().map(|s| s.as_str()).collect();
    let mut mapped: HashSet<&str> = HashSet::new();
    let mut out = vec![];

    for t in tombstones {
        if rmap.contains_key(t.as_str()) {
            out.push(Plan::DeleteRemote(t.clone()));
        }
    }
    // Kéo về hỏng (Lời nhắc trả về rỗng) trong khi Mini-Asi có nhiều mục đã nối → không xoá gì cả
    let linked = locals.iter().filter(|l| l.ext_id.is_some()).count();
    let suspicious_empty = remotes.is_empty() && linked >= 3;
    for l in locals {
        if let Some(ext) = &l.ext_id {
            if rmap.contains_key(ext.as_str()) {
                mapped.insert(ext.as_str());
            }
        }
    }
    for l in locals {
        match &l.ext_id {
            None => out.push(Plan::PushNew(l.id)),
            Some(ext) => match rmap.get(ext.as_str()) {
                Some(r) => {
                    let r_changed = r.modified > last_sync;
                    let l_changed = l.updated > last_sync;
                    if r_changed && (!l_changed || r.modified >= l.updated) {
                        out.push(Plan::PullUpdate(l.id, ext.clone()));
                    } else if l_changed {
                        out.push(Plan::PushUpdate(l.id, ext.clone()));
                    }
                }
                None => {
                    if suspicious_empty {
                        continue;
                    }
                    let twin = (!l.title.trim().is_empty())
                        .then(|| remotes.iter().find(|r| !mapped.contains(r.id.as_str()) && !tomb.contains(r.id.as_str()) && norm(&r.title) == norm(&l.title)))
                        .flatten();
                    if let Some(r) = twin {
                        mapped.insert(r.id.as_str());
                        out.push(Plan::Relink(l.id, r.id.clone()));
                    } else if l.seen && l.updated <= last_sync {
                        out.push(Plan::DeleteLocal(l.id));
                    } else {
                        out.push(Plan::PushNew(l.id)); // chưa từng thấy bên Apple / vừa sửa → giữ lại, tạo lại bên Apple
                    }
                }
            },
        }
    }
    for r in remotes {
        if !mapped.contains(r.id.as_str()) && !tomb.contains(r.id.as_str()) && !r.stale_done {
            out.push(Plan::PullNew(r.id.clone()));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l(id: u64, ext: Option<&str>, upd: i64) -> Local {
        Local { id, ext_id: ext.map(String::from), updated: upd, seen: true, title: String::new() }
    }
    fn r(id: &str, m: i64) -> Remote {
        Remote { id: id.into(), modified: m, stale_done: false, title: String::new() }
    }

    #[test]
    fn never_deletes_task_apple_has_not_confirmed() {
        let last = 100;
        // việc vừa thêm bằng giọng nói: đã có id bên Apple nhưng lần kéo về chưa thấy → KHÔNG xoá
        let new = Local { id: 7, ext_id: Some("X".into()), updated: 50, seen: false, title: "Gọi khách".into() };
        assert_eq!(plan(&[new.clone()], &[r("A", 10)], last, &[]), vec![Plan::PushNew(7), Plan::PullNew("A".into())]);
        // id lệch nhưng có lời nhắc cùng tên → nối lại, không tạo trùng
        let twin = Remote { id: "B".into(), modified: 10, stale_done: false, title: "gọi  khách".into() };
        assert_eq!(plan(&[new], &[twin], last, &[]), vec![Plan::Relink(7, "B".into())]);
        // Lời nhắc trả về rỗng bất thường → không xoá hàng loạt
        let many: Vec<Local> = (1..=4).map(|i| l(i, Some(&format!("E{i}")), 10)).collect();
        assert!(plan(&many, &[], last, &[]).is_empty());
    }

    #[test]
    fn first_sync_pushes_and_pulls_everything() {
        let p = plan(&[l(1, None, 5)], &[r("A", 3)], 0, &[]);
        assert_eq!(p, vec![Plan::PushNew(1), Plan::PullNew("A".into())]);
    }

    #[test]
    fn newer_side_wins() {
        let last = 100;
        // chỉ Apple sửa
        assert_eq!(plan(&[l(1, Some("A"), 50)], &[r("A", 150)], last, &[]), vec![Plan::PullUpdate(1, "A".into())]);
        // chỉ Mini-Asi sửa
        assert_eq!(plan(&[l(1, Some("A"), 150)], &[r("A", 50)], last, &[]), vec![Plan::PushUpdate(1, "A".into())]);
        // cả 2 sửa → bên nào mới hơn thắng
        assert_eq!(plan(&[l(1, Some("A"), 160)], &[r("A", 150)], last, &[]), vec![Plan::PushUpdate(1, "A".into())]);
        assert_eq!(plan(&[l(1, Some("A"), 140)], &[r("A", 150)], last, &[]), vec![Plan::PullUpdate(1, "A".into())]);
        // không ai sửa
        assert!(plan(&[l(1, Some("A"), 50)], &[r("A", 60)], last, &[]).is_empty());
    }

    #[test]
    fn deletions_both_ways() {
        let last = 100;
        // Apple xoá, Mini-Asi không sửa → xoá local
        assert_eq!(plan(&[l(1, Some("A"), 50)], &[], last, &[]), vec![Plan::DeleteLocal(1)]);
        // Apple xoá nhưng Mini-Asi vừa sửa → tạo lại
        assert_eq!(plan(&[l(1, Some("A"), 150)], &[], last, &[]), vec![Plan::PushNew(1)]);
        // Mini-Asi xoá → xoá bên Apple, không kéo về lại
        assert_eq!(plan(&[], &[r("A", 50)], last, &["A".into()]), vec![Plan::DeleteRemote("A".into())]);
        // lời nhắc xong từ lâu → không kéo về
        assert!(plan(&[], &[Remote { id: "B".into(), modified: 1, stale_done: true, title: String::new() }], last, &[]).is_empty());
    }
}
