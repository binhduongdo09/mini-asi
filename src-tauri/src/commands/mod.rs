//! Lệnh Tauri (gọi từ ui/*.js qua `invoke`) chia theo tính năng. Mỗi file một mảng.
//! Mọi tiện ích dùng chung được gom ở `crate::prelude` để các file gọi nhau thoải mái như cùng một file.

pub mod state;
pub mod window;
pub mod tasks;
pub mod notes;
pub mod files;
pub mod clean;
pub mod health;
pub mod settings;
pub mod clipboard;
pub mod voice_input;
pub mod apple_sync;
pub mod journal;
pub mod threads;
pub mod permissions;
pub mod backup;
pub mod assistant;
pub mod speech;
pub mod vocab;
pub mod status;
pub mod updater;
pub mod capture;
