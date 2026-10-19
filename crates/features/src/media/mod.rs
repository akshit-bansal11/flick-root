//! F08 Media controls: GSMTC play/pause/next/previous/now-playing on any player, plus
//! "play <q> on spotify|youtube" search links (ADR-010 base layer).
//!
//! `query()` only parses text. The media session is read in `execute()`, so no task runs
//! while the feature is on and the "Now playing" row reads the track when chosen.
//!
//! With Spotify connected (optional, [`spotify`]), extra rows appear; the cheap `connected`
//! flag decides them and every API call happens in `execute()`.

mod gsmtc;
pub mod links;
pub mod spotify;

use crate::ctx::FeatureCtx;
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::schema::SettingsSchema;
use async_trait::async_trait;
use gsmtc::{Backend, Gsmtc};
use launcher_core::{Action, ActionKind, Input, Item, ItemId, ItemKind, Mode, Outcome};
use spotify::{Named, Played, Spotify};
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
    /// `queue <q>` (connected only).
    Queue(String),
    /// `like`: save the current track (connected only).
    Like,
    /// `devices` (connected only).
    Devices,
    /// `playlist <q>`: your own playlists (connected only).
    Playlist(String),
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
        "like" => return Some(Ask::Like),
        "devices" => return Some(Ask::Devices),
        _ => {}
    }
    for (prefix, make) in [
        ("queue ", Ask::Queue as fn(String) -> Ask),
        ("playlist ", Ask::Playlist),
    ] {
        if t.get(..prefix.len())
            .is_some_and(|h| h.eq_ignore_ascii_case(prefix))
        {
            let q = t[prefix.len()..].trim();
            return (!q.is_empty()).then(|| make(q.to_owned()));
        }
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

const PREMIUM: &str = "Spotify Premium";

fn sp_row(
    kind: &str,
    key: &str,
    title: String,
    subtitle: &str,
    payload: serde_json::Value,
) -> Item {
    row(format!("media:sp:{kind}:{key}"), title, subtitle, payload)
}

fn items(ask: Ask, connected: bool) -> Vec<Item> {
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
        Ask::Spotify(q) => {
            let mut v = Vec::new();
            if connected {
                v.push(sp_row(
                    "play",
                    &q,
                    format!("Play {q} on Spotify"),
                    "Premium; opens the Spotify app instead without it",
                    serde_json::json!({ "sp": "play", "q": q }),
                ));
            }
            v.push(row(
                format!("media:spotify:{q}"),
                format!("Search Spotify for {q}"),
                "Opens the Spotify app",
                serde_json::json!({ "url": links::spotify(&q) }),
            ));
            v
        }
        Ask::YouTube(q) => vec![row(
            format!("media:youtube:{q}"),
            format!("Search YouTube for {q}"),
            "Opens your browser",
            serde_json::json!({ "url": links::youtube(&q), "yt": q }),
        )],
        _ if !connected => Vec::new(),
        Ask::Queue(q) => vec![sp_row(
            "queue",
            &q,
            format!("Queue {q} on Spotify"),
            PREMIUM,
            serde_json::json!({ "sp": "queue", "q": q }),
        )],
        Ask::Like => vec![sp_row(
            "like",
            "",
            "Like current Spotify track".into(),
            "Saves it to your library",
            serde_json::json!({ "sp": "like" }),
        )],
        Ask::Devices => vec![sp_row(
            "devices",
            "",
            "Spotify devices".into(),
            "Pick one to move playback (Premium)",
            serde_json::json!({ "sp": "devices" }),
        )],
        Ask::Playlist(q) => vec![sp_row(
            "playlist",
            &q,
            format!("Find your playlist {q}"),
            "Plays it on Spotify (Premium)",
            serde_json::json!({ "sp": "playlists", "q": q }),
        )],
    }
}

/// The media feature.
pub struct Media {
    backend: Arc<dyn Backend>,
    api: Option<Arc<Spotify>>,
}

impl Default for Media {
    fn default() -> Self {
        Self {
            backend: Arc::new(Gsmtc),
            api: None,
        }
    }
}

impl Media {
    /// Media with the optional Spotify connection and YouTube key lookups.
    pub fn with_api(api: Arc<Spotify>) -> Self {
        Self {
            api: Some(api),
            ..Self::default()
        }
    }

    fn connected(&self) -> bool {
        self.api.as_ref().is_some_and(|a| a.is_connected())
    }

    async fn open(&self, url: &str) -> Result<(), FeatureError> {
        let (b, url) = (self.backend.clone(), url.to_owned());
        tokio::task::spawn_blocking(move || b.open(&url))
            .await
            .map_err(join_err)?
            .map_err(|e| FeatureError::Other(format!("could not open link: {e}")))
    }

    async fn run_spotify(&self, item: &Item) -> Result<Outcome, FeatureError> {
        let api = self
            .api
            .as_ref()
            .ok_or_else(|| FeatureError::Other("Spotify is not connected".into()))?;
        let p = &item.payload;
        let arg = |k: &str| p.get(k).and_then(|v| v.as_str()).unwrap_or_default();
        let fe = |e: spotify::SpotifyError| FeatureError::Other(e.to_string());
        match arg("sp") {
            "play" => match api.play_query(arg("q")).await.map_err(fe)? {
                Played::Api => {}
                Played::Fallback(uri) => self.open(&uri).await?,
            },
            "queue" => api.queue(arg("q")).await.map_err(fe)?,
            "like" => api.like().await.map_err(fe)?,
            "transfer" => api.transfer(arg("id")).await.map_err(fe)?,
            "context" => api.play_context(arg("id")).await.map_err(fe)?,
            "devices" => {
                let d = api.devices().await.map_err(fe)?;
                let empty = "No Spotify devices found - open Spotify on a device";
                return list(d, "transfer", "Move playback here (Premium)", empty);
            }
            "playlists" => {
                let d = api.playlists(arg("q")).await.map_err(fe)?;
                let empty = "None of your first 50 playlists match";
                return list(d, "context", "Play on Spotify (Premium)", empty);
            }
            _ => return Err(FeatureError::Other("unsupported action".into())),
        }
        Ok(Outcome::HideLauncher)
    }

    /// The video page for `q` when the YouTube API is on and works; `None` means use the search page.
    async fn youtube_hit(&self, q: &str) -> Option<String> {
        let api = self.api.as_ref()?;
        let key = api.youtube_key().await?;
        links::youtube_first(api.http(), links::YOUTUBE_API, &key, q).await
    }
}

fn list(found: Vec<Named>, kind: &str, sub: &str, empty: &str) -> Result<Outcome, FeatureError> {
    if found.is_empty() {
        return Err(FeatureError::Other(empty.into()));
    }
    let items = found
        .into_iter()
        .map(|n| {
            let title = if n.detail.is_empty() {
                n.name.clone()
            } else {
                format!("{} ({})", n.name, n.detail)
            };
            let payload = serde_json::json!({ "sp": kind, "id": n.id });
            sp_row(kind, &n.id, title, sub, payload)
        })
        .collect();
    Ok(Outcome::ShowItems { items })
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
    fn start(&self, ctx: FeatureCtx) -> Result<(), FeatureError> {
        if let Some(api) = &self.api {
            let api = Arc::clone(api);
            ctx.tasks.spawn(async move { api.init().await });
        }
        Ok(())
    }
    fn stop(&self) {}
    fn query(&self, input: &Input) -> Vec<Item> {
        let c = self.connected();
        parse(&input.text).map(|a| items(a, c)).unwrap_or_default()
    }
    async fn execute(&self, item: &Item, action: Action) -> Result<Outcome, FeatureError> {
        if action.kind != ActionKind::Primary {
            return Err(FeatureError::Other("unsupported action".into()));
        }
        if item.payload.get("sp").is_some() {
            return self.run_spotify(item).await;
        }
        let b = self.backend.clone();
        if let Some(url) = item.payload.get("url").and_then(|u| u.as_str()) {
            let hit = match item.payload.get("yt").and_then(|q| q.as_str()) {
                Some(q) => self.youtube_hit(q).await,
                None => None,
            };
            self.open(hit.as_deref().unwrap_or(url)).await?;
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
        (
            Media {
                backend: f.clone(),
                api: None,
            },
            f,
        )
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
    fn parse_spotify_asks() {
        assert_eq!(parse("like"), Some(Ask::Like));
        assert_eq!(parse("devices"), Some(Ask::Devices));
        assert_eq!(parse("Queue  Song x"), Some(Ask::Queue("Song x".into())));
        assert_eq!(parse("playlist gym"), Some(Ask::Playlist("gym".into())));
        assert_eq!(parse("queue "), None);
        // Disconnected: no rows for connected-only asks, no API play row.
        assert!(items(Ask::Like, false).is_empty());
        assert_eq!(items(Ask::Spotify("x".into()), false).len(), 1);
        let on = items(Ask::Spotify("x".into()), true);
        assert_eq!(on.len(), 2);
        assert!(on[0].subtitle.contains("Premium"));
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
        let item = items(Ask::Cmd(MediaCmd::Next), false).remove(0);
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
        let item = items(Ask::YouTube("x y".into()), false).remove(0);
        m.execute(&item, prim()).await.unwrap();
        assert_eq!(
            *f.opened.lock(),
            vec!["https://www.youtube.com/results?search_query=x%20y"]
        );
        let np = items(Ask::NowPlaying, false).remove(0);
        let Outcome::ShowItems { items } = m.execute(&np, prim()).await.unwrap() else {
            panic!("expected items");
        };
        assert_eq!(items[0].title, "Song");
        assert_eq!(items[0].subtitle, "Band · Playing");
    }
}
