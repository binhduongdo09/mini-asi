//! Prelude nội bộ: các import hay dùng + toàn bộ lệnh/tiện ích của `commands::*`.
#![allow(unused_imports)]

pub use chrono::{Datelike, Local, Timelike};
pub use serde::Serialize;
pub use std::collections::{HashMap, HashSet};
pub use std::path::PathBuf;
pub use std::sync::{mpsc, Arc, Mutex, OnceLock};
pub use std::time::Duration;
pub use store::{now_ms, Store, Task};
pub use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, State, WebviewWindow};
pub use crate::{apple, apple_ai, capture, assistant, backup, clipboard, files, health, journal, license, llm, nlp, store, sync, threads, tts, updater, vocab, voice};
pub use crate::commands::state::*;
pub use crate::commands::window::*;
pub use crate::commands::tasks::*;
pub use crate::commands::notes::*;
pub use crate::commands::files::*;
pub use crate::commands::clean::*;
pub use crate::commands::health::*;
pub use crate::commands::settings::*;
pub use crate::commands::clipboard::*;
pub use crate::commands::voice_input::*;
pub use crate::commands::apple_sync::*;
pub use crate::commands::journal::*;
pub use crate::commands::threads::*;
pub use crate::commands::permissions::*;
pub use crate::commands::backup::*;
pub use crate::commands::assistant::*;
pub use crate::commands::speech::*;
pub use crate::commands::vocab::*;
pub use crate::commands::status::*;
pub use crate::commands::updater::*;
pub use crate::commands::capture::*;
pub use crate::commands::live::*;
