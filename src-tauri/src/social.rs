//! Tab "Social": mở các mạng xã hội thành cửa sổ dạng điện thoại ngay dưới tai thỏ (giống cửa sổ Threads).
//! Mỗi mạng một cửa sổ riêng (giữ nguyên trang đang xem khi ẩn/hiện), đăng nhập một lần là nhớ.
//! Các cửa sổ này KHÔNG được quyền gọi lệnh của Mini-Asi; link ra ngoài mạng đó → mở bằng trình duyệt của máy.

pub struct Network {
    pub id: &'static str,
    pub name: &'static str,
    /// Trang mở đầu
    pub url: &'static str,
    /// Tên miền được phép đi tới ngay trong cửa sổ (kể cả tên miền con)
    pub hosts: &'static [&'static str],
    /// true = giả làm Safari trên iPhone (giao diện mobile); false = giao diện web thường, cửa sổ hẹp
    /// (Messenger bản mobile web chỉ hiện trang mời tải app)
    pub mobile: bool,
    pub width: f64,
}

const META: &[&str] = &["facebook.com", "facebook.net", "fbcdn.net", "fbsbx.com", "fb.com", "fb.me", "meta.com", "messenger.com", "instagram.com", "cdninstagram.com", "threads.com", "threads.net"];

pub const NETWORKS: &[Network] = &[
    Network { id: "facebook", name: "Facebook", url: "https://m.facebook.com/", hosts: META, mobile: true, width: 440.0 },
    Network { id: "messenger", name: "Messenger", url: "https://www.messenger.com/", hosts: META, mobile: false, width: 480.0 },
    Network { id: "instagram", name: "Instagram", url: "https://www.instagram.com/", hosts: META, mobile: true, width: 440.0 },
    Network { id: "threads", name: "Threads", url: "https://www.threads.com/", hosts: META, mobile: true, width: 440.0 },
    Network { id: "x", name: "X", url: "https://x.com/home", hosts: &["x.com", "twitter.com", "twimg.com", "t.co", "pscp.tv", "periscope.tv"], mobile: true, width: 440.0 },
    Network {
        id: "tiktok",
        name: "TikTok",
        url: "https://www.tiktok.com/foryou",
        hosts: &["tiktok.com", "tiktokv.com", "tiktokcdn.com", "tiktokcdn-us.com", "tiktokw.us", "byteoversea.com", "ibytedtos.com", "ttwstatic.com", "byteimg.com"],
        mobile: true,
        width: 440.0,
    },
    Network {
        id: "youtube",
        name: "YouTube",
        url: "https://m.youtube.com/",
        hosts: &["youtube.com", "youtu.be", "ytimg.com", "googlevideo.com", "ggpht.com", "youtube-nocookie.com", "googleusercontent.com"],
        mobile: true,
        width: 440.0,
    },
];

/// Đăng nhập bằng Google / Apple ngay trong cửa sổ (nếu mở ra trình duyệt ngoài thì đăng nhập không quay lại được)
const SIGN_IN: &[&str] = &["accounts.google.com", "accounts.youtube.com", "appleid.apple.com", "gstatic.com", "recaptcha.net", "google.com"];

pub fn find(id: &str) -> Option<&'static Network> {
    NETWORKS.iter().find(|n| n.id == id)
}

fn host_in(host: &str, list: &[&str]) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    list.iter().any(|d| host == *d || host.ends_with(&format!(".{d}")))
}

/// Trang có được mở ngay trong cửa sổ của mạng này không
pub fn host_ok(n: &Network, host: &str) -> bool {
    host_in(host, n.hosts) || host_in(host, SIGN_IN)
}

/// Nhãn cửa sổ — Threads giữ nhãn cũ để dùng chung cửa sổ với nút "Mở bảng tin"
pub fn window_label(n: &Network) -> String {
    if n.id == "threads" { "threads-web".into() } else { format!("social-{}", n.id) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn networks_and_hosts() {
        assert_eq!(NETWORKS.len(), 7);
        let fb = find("facebook").unwrap();
        assert!(host_ok(fb, "m.facebook.com"));
        assert!(host_ok(fb, "scontent.xx.fbcdn.net"));
        assert!(host_ok(fb, "accounts.google.com"), "đăng nhập Google ngay trong cửa sổ");
        assert!(!host_ok(fb, "vnexpress.net"));
        assert!(!host_ok(fb, "evilfacebook.com"), "không nhận tên miền giả");
        let x = find("x").unwrap();
        assert!(host_ok(x, "x.com") && host_ok(x, "pbs.twimg.com") && !host_ok(x, "facebook.com"));
        let yt = find("youtube").unwrap();
        assert!(host_ok(yt, "m.youtube.com") && host_ok(yt, "i.ytimg.com") && !host_ok(yt, "tiktok.com"));
        assert!(!find("messenger").unwrap().mobile);
        assert_eq!(window_label(find("threads").unwrap()), "threads-web");
        assert_eq!(window_label(find("tiktok").unwrap()), "social-tiktok");
        assert!(find("myspace").is_none());
        for n in NETWORKS {
            assert!(n.url.starts_with("https://"));
            let h = n.url.split('/').nth(2).unwrap();
            assert!(host_ok(n, h), "{} mở được trang đầu", n.id);
        }
    }
}
