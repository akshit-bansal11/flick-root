//! Thin seam over `win::media` so execute() can be tested without a live session.

use std::path::Path;
use win::media::{MediaCmd, NowPlaying};

/// Blocking media backend; callers use `spawn_blocking`.
pub trait Backend: Send + Sync {
    /// Sends a command; `Ok(false)` when there is no session or it refused.
    fn control(&self, cmd: MediaCmd) -> Result<bool, String>;
    /// Current track, if any.
    fn now_playing(&self) -> Result<Option<NowPlaying>, String>;
    /// Opens a URL or URI with its default handler.
    fn open(&self, url: &str) -> Result<(), String>;
}

/// Real Windows backend.
pub struct Gsmtc;

impl Backend for Gsmtc {
    fn control(&self, cmd: MediaCmd) -> Result<bool, String> {
        win::media::control(cmd).map_err(|e| e.to_string())
    }
    fn now_playing(&self) -> Result<Option<NowPlaying>, String> {
        win::media::now_playing().map_err(|e| e.to_string())
    }
    fn open(&self, url: &str) -> Result<(), String> {
        let _com = index_apps::com::ComGuard::new().map_err(|e| e.to_string())?;
        index_apps::launch::open_path(Path::new(url)).map_err(|e| e.to_string())
    }
}
