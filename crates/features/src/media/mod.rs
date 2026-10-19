//! F08 Media controls: GSMTC play/pause/next/previous/now-playing on any player, plus
//! "play <q> on spotify|youtube" search links (ADR-010 base layer).
//!
//! `query()` only parses text. The media session is read in `execute()`, so no task runs
//! while the feature is on and the "Now playing" row reads the track when chosen.

mod gsmtc;
pub mod links;

use crate::ctx::FeatureCtx;
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::schema::SettingsSchema;
use async_trait::async_trait;
use gsmtc::{Backend, Gsmtc};
use launcher_core::{Action, ActionKind, Input, Item, ItemId, ItemKind, Mode, Outcome};
use std::sync::Arc;
use win::media::{MediaCmd, PlaybackState};

/// What the typed text asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ask {
    /// Send a command to the current session.
    Cmd(MediaCmd),
    /// Show the current track.
    NowPlaying,
    /// `play <q> on spotify`.
    Spotify(String),
    /// `play <q> on youtube`.
    YouTube(String),
}

/// Parses the query text. Pure; no I/O.
pub fn parse(text: &str) -> Option<Ask> {
    let t = text.trim();
    match t.to_lowercase().as_str() {
        "play" => return Some(Ask::Cmd(MediaCmd::Play)),
        "pause" => return Some(Ask::Cmd(MediaCmd::Pause)),
        "next" => return Some(Ask::Cmd(MediaCmd::Next)),
        "previous" | "prev" => return Some(Ask::Cmd(MediaCmd::Previous)),
        "stop" => return Some(Ask::Cmd(MediaCmd::Stop)),
        "now playing" | "np" => return Some(Ask::NowPlaying),
        _ => {}
    }
    let head = t.get(..5)?;
    if !head.eq_ignore_ascii_case("play ") {
        return None;
    }
    let body = t[5..].trim();
    let lower = body.to_lowercase();
    for (suffix, spotify) in [(" on spotify", true), (" on youtube", false)] {
        if lower.ends_with(suffix) {
            // ponytail: assumes lowercasing keeps byte length (true for ASCII suffix matches).
            let q = body.get(..body.len() - suffix.len())?.trim();
            if q.is_empty() {
                return None;
            }
            let q = q.to_owned();
            return Some(if spotify {
                Ask::Spotify(q)
            } else {
                Ask::YouTube(q)
            });
        }
    }
    None
}

fn cmd_name(c: MediaCmd) -> &'static str {
    match c {
        MediaCmd::PlayPause => "playpause",
        MediaCmd::Play => "play",
        MediaCmd::Pause => "pause",
        MediaCmd::Next => "next",
        MediaCmd::Previous => "previous",
        MediaCmd::Stop => "stop",
    }
}

fn cmd_from(s: &str) -> Option<MediaCmd> {
    Some(match s {
        "playpause" => MediaCmd::PlayPause,
        "play" => MediaCmd::Play,
        "pause" => MediaCmd::Pause,
        "next" => MediaCmd::Next,
        "previous" => MediaCmd::Previous,
        "stop" => MediaCmd::Stop,
        _ => return None,
    })
}

fn row(id: String, title: String, subtitle: &str, payload: serde_json::Value) -> Item {
    Item {
        id: ItemId(id),
        kind: ItemKind::Media,
        title,
        subtitle: subtitle.to_owned(),
        icon: None,
        score: 1.0,
        source: "media".to_owned(),
        actions: vec![ActionKind::Primary],
        payload,
    }
}

fn items(ask: Ask) -> Vec<Item> {
    match ask {
        Ask::Cmd(c) => {
            let title = match c {
                MediaCmd::PlayPause => "Play/Pause",
                MediaCmd::Play => "Play",
                MediaCmd::Pause => "Pause",
                MediaCmd::Next => "Next track",
                MediaCmd::Previous => "Previous track",
                MediaCmd::Stop => "Stop",
            };
            vec![row(
                format!("media:{}", cmd_name(c)),
                title.into(),
                "Current media player",
                serde_json::json!({ "cmd": cmd_name(c) }),
            )]
        }
        Ask::NowPlaying => vec![row(
            "media:now".into(),
            "Now playing".into(),
            "Show the current track",
            serde_json::json!({ "now": true }),
        )],
        Ask::Spotify(q) => vec![row(
            format!("media:spotify:{q}"),
            format!("Search Spotify for {q}"),
            "Opens the Spotify app",
            serde_json::json!({ "url": links::spotify(&q) }),
        )],
        Ask::YouTube(q) => vec![row(
            format!("media:youtube:{q}"),
            format!("Search YouTube for {q}"),
            "Opens your browser",
            serde_json::json!({ "url": links::youtube(&q) }),
        )],
    }
}

/// The media feature.
pub struct Media {
    backend: Arc<dyn Backend>,
}

impl Default for Media {
    fn default() -> Self {
        Self {
            backend: Arc::new(Gsmtc),
        }
    }
}

fn join_err(e: tokio::task::JoinError) -> FeatureError {
    FeatureError::Other(format!("media task failed: {e}"))
}

#[async_trait]
impl Feature for Media {
    fn id(&self) -> &'static str {
        "media"
    }
    fn title(&self) -> &'static str {
        "Media controls"
    }
    fn version_tag(&self) -> &'static str {
        "v0.1.0"
    }
    fn description(&self) -> &'static str {
        "play, pause, next, previous, now playing; \"play <song> on spotify|youtube\"."
    }
    fn default_enabled(&self) -> bool {
        true
    }
    fn modes(&self) -> &'static [Mode] {
        &[Mode::Command]
    }
    fn settings_schema(&self) -> SettingsSchema {
        SettingsSchema::default()
    }
    fn start(&self, _ctx: FeatureCtx) -> Result<(), FeatureError> {
        Ok(())
    }
    fn stop(&self) {}
    fn query(&self, input: &Input) -> Vec<Item> {
        parse(&input.text).map(items).unwrap_or_default()
    }
    async fn execute(&self, item: &Item, action: Action) -> Result<Outcome, FeatureError> {
        if action.kind != ActionKind::Primary {
            return Err(FeatureError::Other("unsupported action".into()));
        }
        let b = self.backend.clone();
        if let Some(url) = item.payload.get("url").and_then(|u| u.as_str()) {
            let url = url.to_owned();
            tokio::task::spawn_blocking(move || b.open(&url))
                .await
                .map_err(join_err)?
                .map_err(|e| FeatureError::Other(format!("could not open link: {e}")))?;
            return Ok(Outcome::HideLauncher);
        }
        if let Some(cmd) = item
            .payload
            .get("cmd")
            .and_then(|c| c.as_str())
            .and_then(cmd_from)
        {
            let ok = tokio::task::spawn_blocking(move || b.control(cmd))
                .await
                .map_err(join_err)?
                .map_err(|e| FeatureError::Other(format!("media control failed: {e}")))?;
            return if ok {
                Ok(Outcome::HideLauncher)
            } else {
                Err(FeatureError::Other("No media player is active".into()))
            };
        }
        let np = tokio::task::spawn_blocking(move || b.now_playing())
            .await
            .map_err(join_err)?
            .map_err(|e| FeatureError::Other(format!("media read failed: {e}")))?;
        let Some(np) = np else {
            return Err(FeatureError::Other("No media player is active".into()));
        };
        let state = match np.state {
            PlaybackState::Playing => "Playing",
            PlaybackState::Paused => "Paused",
            PlaybackState::Stopped => "Stopped",
        };
        let sub = [np.artist.as_str(), state]
            .iter()
            .filter(|s| !s.is_empty())
            .copied()
            .collect::<Vec<_>>()
            .join(" · ");
        let mut r = row(
            "media:track".into(),
            np.title,
            &sub,
            serde_json::Value::Null,
        );
        r.actions = vec![];
        Ok(Outcome::ShowItems { items: vec![r] })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parking_lot::Mutex;
    use win::media::NowPlaying;

    #[derive(Default)]
    struct Fake {
        session: bool,
        sent: Mutex<Vec<MediaCmd>>,
        opened: Mutex<Vec<String>>,
    }
    impl Backend for Fake {
        fn control(&self, cmd: MediaCmd) -> Result<bool, String> {
            self.sent.lock().push(cmd);
            Ok(self.session)
        }
        fn now_playing(&self) -> Result<Option<NowPlaying>, String> {
            Ok(self.session.then(|| NowPlaying {
                title: "Song".into(),
                artist: "Band".into(),
                album: String::new(),
                source_app_id: String::new(),
                state: PlaybackState::Playing,
                can_play_pause: true,
                can_next: true,
                can_previous: true,
            }))
        }
        fn open(&self, url: &str) -> Result<(), String> {
            self.opened.lock().push(url.to_owned());
            Ok(())
        }
    }

    fn feat(session: bool) -> (Media, Arc<Fake>) {
        let f = Arc::new(Fake {
            session,
            ..Fake::default()
        });
        (Media { backend: f.clone() }, f)
    }

    fn prim() -> Action {
        Action {
            kind: ActionKind::Primary,
            confirm_token: None,
        }
    }

    #[test]
    fn parse_table() {
        use MediaCmd::*;
        let cases = [
            ("play", Some(Ask::Cmd(Play))),
            (" PAUSE ", Some(Ask::Cmd(Pause))),
            ("next", Some(Ask::Cmd(Next))),
            ("prev", Some(Ask::Cmd(Previous))),
            ("Previous", Some(Ask::Cmd(Previous))),
            ("stop", Some(Ask::Cmd(Stop))),
            ("np", Some(Ask::NowPlaying)),
            ("now playing", Some(Ask::NowPlaying)),
            (
                "play Daft Punk on Spotify",
                Some(Ask::Spotify("Daft Punk".into())),
            ),
            ("PLAY lofi on youtube", Some(Ask::YouTube("lofi".into()))),
            ("play on spotify", None),
            ("play lofi", None),
            ("player", None),
            ("pla", None),
            ("", None),
        ];
        for (t, want) in cases {
            assert_eq!(parse(t), want, "{t}");
        }
    }

    #[test]
    fn query_builds_link_rows() {
        let (m, _) = feat(true);
        let i = Input {
            raw: "play a&b on spotify".into(),
            text: "play a&b on spotify".into(),
            mode: Mode::Command,
            seq: 1,
        };
        let r = m.query(&i);
        assert_eq!(r[0].title, "Search Spotify for a&b");
        assert_eq!(r[0].payload["url"], "spotify:search:a%26b");
    }

    #[tokio::test]
    async fn execute_control_and_no_session() {
        let (m, f) = feat(true);
        let item = items(Ask::Cmd(MediaCmd::Next)).remove(0);
        assert_eq!(
            m.execute(&item, prim()).await.unwrap(),
            Outcome::HideLauncher
        );
        assert_eq!(*f.sent.lock(), vec![MediaCmd::Next]);
        let (m, _) = feat(false);
        let e = m.execute(&item, prim()).await.unwrap_err();
        assert!(e.to_string().contains("No media player"));
    }

    #[tokio::test]
    async fn execute_link_and_now_playing() {
        let (m, f) = feat(true);
        let item = items(Ask::YouTube("x y".into())).remove(0);
        m.execute(&item, prim()).await.unwrap();
        assert_eq!(
            *f.opened.lock(),
            vec!["https://www.youtube.com/results?search_query=x%20y"]
        );
        let np = items(Ask::NowPlaying).remove(0);
        let Outcome::ShowItems { items } = m.execute(&np, prim()).await.unwrap() else {
            panic!("expected items");
        };
        assert_eq!(items[0].title, "Song");
        assert_eq!(items[0].subtitle, "Band · Playing");
    }
}
