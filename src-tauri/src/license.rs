//! Kiểm tra khoá Pro (offline). Xem crates/license-tool để cấp khoá.

/// KHOÁ CÔNG KHAI dùng để xác thực. Đây là khoá THỬ NGHIỆM —
/// trước khi bán: chạy `cargo run -p license-tool -- keygen`, thay giá trị này,
/// và cất khoá bí mật ở nơi an toàn (KHÔNG commit).
pub const PUBLIC_KEY: &str = "rh01-i2mR0uRcTwRDhjcYf1e475jM-MrX7yMim5HX70";

pub fn check(key: &str) -> Result<masi_license::License, masi_license::LicenseError> {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    masi_license::verify(key, PUBLIC_KEY, now)
}
