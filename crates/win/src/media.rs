//! Media session control through Windows Global System Media Transport Controls (GSMTC).
//!
//! Both functions block on WinRT async operations (`.join()`); call them from `spawn_blocking`.

use crate::com::ComGuard;
use crate::error::WinError;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session,
    GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
};

/// Playback state of a media session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    /// Playing.
    Playing,
    /// Paused.
    Paused,
    /// Stopped, closed, opened or changing.
    Stopped,
}

/// A command sent to the current media session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaCmd {
    /// Toggle play and pause.
    PlayPause,
    /// Play.
    Play,
    /// Pause.
    Pause,
    /// Next track.
    Next,
    /// Previous track.
    Previous,
    /// Stop.
    Stop,
}

/// What the current media session is playing and what it accepts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NowPlaying {
    /// Track title.
    pub title: String,
    /// Artist.
    pub artist: String,
    /// Album title.
    pub album: String,
    /// App user model id of the player.
    pub source_app_id: String,
    /// Playback state.
    pub state: PlaybackState,
    /// Whether play/pause toggle is enabled.
    pub can_play_pause: bool,
    /// Whether next track is enabled.
    pub can_next: bool,
    /// Whether previous track is enabled.
    pub can_previous: bool,
}

fn map_status(s: Status) -> PlaybackState {
    if s == Status::Playing {
        PlaybackState::Playing
    } else if s == Status::Paused {
        PlaybackState::Paused
    } else {
        PlaybackState::Stopped
    }
}

fn current_session() -> Result<Option<Session>, WinError> {
    let _com = ComGuard::new_mta()?;
    let mgr = Manager::RequestAsync()
        .and_then(|op| op.join())
        .map_err(|e| WinError::os(e, "SessionManager.RequestAsync"))?;
    // A missing session comes back as a null interface, which the bindings surface as an error.
    Ok(mgr.GetCurrentSession().ok())
}

/// The current media session's track and capabilities, or `None` when nothing is open. Blocking.
pub fn now_playing() -> Result<Option<NowPlaying>, WinError> {
    let Some(s) = current_session()? else {
        return Ok(None);
    };
    let props = s
        .TryGetMediaPropertiesAsync()
        .and_then(|op| op.join())
        .map_err(|e| WinError::os(e, "TryGetMediaPropertiesAsync"))?;
    let info = s
        .GetPlaybackInfo()
        .map_err(|e| WinError::os(e, "GetPlaybackInfo"))?;
    let ctl = info
        .Controls()
        .map_err(|e| WinError::os(e, "PlaybackControls"))?;
    let text = |r: windows::core::Result<windows::core::HSTRING>| {
        r.map(|h| h.to_string()).unwrap_or_default()
    };
    Ok(Some(NowPlaying {
        title: text(props.Title()),
        artist: text(props.Artist()),
        album: text(props.AlbumTitle()),
        source_app_id: text(s.SourceAppUserModelId()),
        state: info
            .PlaybackStatus()
            .map(map_status)
            .unwrap_or(PlaybackState::Stopped),
        can_play_pause: ctl.IsPlayPauseToggleEnabled().unwrap_or(false),
        can_next: ctl.IsNextEnabled().unwrap_or(false),
        can_previous: ctl.IsPreviousEnabled().unwrap_or(false),
    }))
}

/// Send a command to the current session. `Ok(false)` when there is no session or it refused. Blocking.
pub fn control(cmd: MediaCmd) -> Result<bool, WinError> {
    let Some(s) = current_session()? else {
        return Ok(false);
    };
    let op = match cmd {
        MediaCmd::PlayPause => s.TryTogglePlayPauseAsync(),
        MediaCmd::Play => s.TryPlayAsync(),
        MediaCmd::Pause => s.TryPauseAsync(),
        MediaCmd::Next => s.TrySkipNextAsync(),
        MediaCmd::Previous => s.TrySkipPreviousAsync(),
        MediaCmd::Stop => s.TryStopAsync(),
    };
    op.and_then(|o| o.join())
        .map_err(|e| WinError::os(e, "media control"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_mapping() {
        assert_eq!(map_status(Status::Playing), PlaybackState::Playing);
        assert_eq!(map_status(Status::Paused), PlaybackState::Paused);
        assert_eq!(map_status(Status::Stopped), PlaybackState::Stopped);
        assert_eq!(map_status(Status::Closed), PlaybackState::Stopped);
    }

    #[test]
    #[ignore = "needs a live media session"]
    fn live_now_playing() {
        println!("{:?}", now_playing());
    }
}
