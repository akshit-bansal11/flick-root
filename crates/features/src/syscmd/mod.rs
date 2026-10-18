//! F03 System commands with the double-Enter guard (KICKOFF 1.3, BSD sections 2 and 5).

use crate::ctx::{EventSink, FeatureCtx, TaskScope};
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::schema::SettingsSchema;
use async_trait::async_trait;
use launcher_core::{Action, ActionKind, Input, Item, ItemId, ItemKind, Mode, Outcome};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use win::WinError;
use win::radios::RadioKind;

/// Window in which the second Enter must arrive.
const CONFIRM_MS: u64 = 3000;

/// One command: id, title, lowercase aliases, whether it needs the double Enter.
struct Cmd {
    id: &'static str,
    title: &'static str,
    aliases: &'static [&'static str],
    guarded: bool,
}

const fn cmd(
    id: &'static str,
    title: &'static str,
    aliases: &'static [&'static str],
    guarded: bool,
) -> Cmd {
    Cmd {
        id,
        title,
        aliases,
        guarded,
    }
}

static COMMANDS: &[Cmd] = &[
    cmd("lock", "Lock", &["lock"], false),
    cmd("sleep", "Sleep", &["sleep"], false),
    cmd("hibernate", "Hibernate", &["hibernate"], false),
    cmd("restart", "Restart", &["restart", "reboot"], true),
    cmd(
        "shutdown",
        "Shut down",
        &["shut down", "shutdown", "power off"],
        true,
    ),
    cmd(
        "sign_out",
        "Sign out",
        &["sign out", "log off", "logout"],
        true,
    ),
    cmd(
        "empty_bin",
        "Empty Recycle Bin",
        &["empty recycle bin", "empty bin"],
        true,
    ),
    cmd(
        "restart_explorer",
        "Restart Explorer",
        &["restart explorer"],
        false,
    ),
    cmd("wifi_on", "Wi-Fi on", &["wifi on", "wi-fi on"], false),
    cmd("wifi_off", "Wi-Fi off", &["wifi off", "wi-fi off"], false),
    cmd(
        "bluetooth_on",
        "Bluetooth on",
        &["bluetooth on", "bt on"],
        false,
    ),
    cmd(
        "bluetooth_off",
        "Bluetooth off",
        &["bluetooth off", "bt off"],
        false,
    ),
];

/// Runs a system command. The real impl calls `crates/win`; tests inject a fake.
#[async_trait]
pub trait SysExec: Send + Sync {
    /// Runs the command with this id.
    async fn run(&self, id: &str) -> Result<(), WinError>;
    /// True when hibernate is supported.
    fn hibernate_available(&self) -> bool;
}

/// Real executor.
pub struct WinExec;

#[async_trait]
impl SysExec for WinExec {
    async fn run(&self, id: &str) -> Result<(), WinError> {
        let id = id.to_owned();
        tokio::task::spawn_blocking(move || match id.as_str() {
            "lock" => win::power::lock(),
            "sleep" => win::power::sleep(),
            "hibernate" => win::power::hibernate(),
            "restart" => win::power::restart(),
            "shutdown" => win::power::shutdown(),
            "sign_out" => win::power::sign_out(),
            "empty_bin" => win::recycle::empty_recycle_bin(),
            "restart_explorer" => win::explorer::restart_explorer(),
            "wifi_on" => win::radios::set_radio(RadioKind::Wifi, true),
            "wifi_off" => win::radios::set_radio(RadioKind::Wifi, false),
            "bluetooth_on" => win::radios::set_radio(RadioKind::Bluetooth, true),
            "bluetooth_off" => win::radios::set_radio(RadioKind::Bluetooth, false),
            _ => Err(WinError::Unavailable(id)),
        })
        .await
        .unwrap_or(Err(WinError::Cancelled))
    }
    fn hibernate_available(&self) -> bool {
        win::power::hibernate_available()
    }
}

/// Monotonic millisecond clock, injectable for tests.
pub trait Clock: Send + Sync {
    /// Milliseconds since an arbitrary fixed point.
    fn now_ms(&self) -> u64;
}

/// Real clock.
pub struct SysClock(Instant);

impl Clock for SysClock {
    fn now_ms(&self) -> u64 {
        u64::try_from(self.0.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}

/// Event sink and task scope of the running feature, set by `start`.
type Live = (Arc<dyn EventSink>, TaskScope);

/// System commands.
pub struct SysCmd {
    exec: Arc<dyn SysExec>,
    clock: Arc<dyn Clock>,
    hibernate: AtomicBool,
    /// token -> (command id, expiry ms)
    pending: Arc<Mutex<HashMap<String, (&'static str, u64)>>>,
    live: Mutex<Option<Live>>,
    counter: AtomicU64,
}

impl Default for SysCmd {
    fn default() -> Self {
        Self::with(Arc::new(WinExec), Arc::new(SysClock(Instant::now())))
    }
}

impl SysCmd {
    /// Builds the feature with an injected executor and clock.
    pub fn with(exec: Arc<dyn SysExec>, clock: Arc<dyn Clock>) -> Self {
        let hib = exec.hibernate_available();
        Self {
            exec,
            clock,
            hibernate: AtomicBool::new(hib),
            pending: Arc::default(),
            live: Mutex::new(None),
            counter: AtomicU64::new(0),
        }
    }

    /// Random-keyed token; `RandomState` is seeded by the OS per instance.
    fn new_token(&self) -> String {
        let n = self.counter.fetch_add(1, Ordering::Relaxed);
        let mut out = String::with_capacity(32);
        for salt in 0..2u64 {
            let mut h = RandomState::new().build_hasher();
            h.write_u64(n ^ (salt << 63));
            out.push_str(&format!("{:016x}", h.finish()));
        }
        out
    }

    fn needs_confirm(&self, id: &'static str) -> Outcome {
        let token = self.new_token();
        let now = self.clock.now_ms();
        {
            let mut p = self.pending.lock();
            p.retain(|_, (_, exp)| *exp > now);
            p.insert(token.clone(), (id, now + CONFIRM_MS));
        }
        if let Some((events, tasks)) = self.live.lock().as_ref() {
            let events = events.clone();
            let pending = self.pending.clone();
            let t = token.clone();
            let cancel = tasks.token();
            tasks.spawn(async move {
                tokio::select! {
                    () = cancel.cancelled() => {}
                    () = tokio::time::sleep(Duration::from_millis(CONFIRM_MS)) => {
                        if pending.lock().remove(&t).is_some() {
                            events.emit("confirm://expired", serde_json::json!({ "token": t }));
                        }
                    }
                }
            });
        }
        Outcome::NeedsConfirm {
            token,
            expires_ms: CONFIRM_MS,
        }
    }
}

fn user_message(e: &WinError) -> String {
    match e {
        WinError::AccessDenied => {
            "Windows denied the request. Check the permission or setting in Windows.".into()
        }
        WinError::Unavailable(what) => format!("Not available on this PC: {what}"),
        other => format!("The command failed: {other}"),
    }
}

#[async_trait]
impl Feature for SysCmd {
    fn id(&self) -> &'static str {
        "syscmd"
    }
    fn title(&self) -> &'static str {
        "System commands"
    }
    fn version_tag(&self) -> &'static str {
        "v0.0.1"
    }
    fn description(&self) -> &'static str {
        "Lock, sleep, restart, shut down, Wi-Fi and Bluetooth toggles and more."
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
        self.hibernate
            .store(self.exec.hibernate_available(), Ordering::Relaxed);
        *self.live.lock() = Some((ctx.events, ctx.tasks));
        Ok(())
    }
    fn stop(&self) {
        *self.live.lock() = None;
        self.pending.lock().clear();
    }
    fn query(&self, input: &Input) -> Vec<Item> {
        let q = input.text.trim().to_lowercase();
        if q.is_empty() {
            return Vec::new();
        }
        let hib = self.hibernate.load(Ordering::Relaxed);
        let mut out = Vec::new();
        for c in COMMANDS {
            if c.id == "hibernate" && !hib {
                continue;
            }
            let mut best = 0.0f32;
            for a in c.aliases {
                if a.starts_with(&q) {
                    best = 1.0;
                    break;
                }
                if a.split(' ').skip(1).any(|w| w.starts_with(&q)) {
                    best = best.max(0.6);
                }
            }
            if best > 0.0 {
                out.push(Item {
                    id: ItemId(format!("syscmd:{}", c.id)),
                    kind: ItemKind::Command,
                    title: c.title.to_owned(),
                    subtitle: if c.guarded {
                        "Press Enter twice to confirm".to_owned()
                    } else {
                        "System command".to_owned()
                    },
                    icon: None,
                    score: best,
                    source: "syscmd".to_owned(),
                    actions: vec![ActionKind::Primary],
                    payload: serde_json::json!({ "command_id": c.id }),
                });
            }
        }
        out
    }
    async fn execute(&self, item: &Item, action: Action) -> Result<Outcome, FeatureError> {
        let id = item
            .payload
            .get("command_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| FeatureError::Other("missing command_id".into()))?;
        let c = COMMANDS
            .iter()
            .find(|c| c.id == id)
            .ok_or_else(|| FeatureError::Other(format!("unknown command {id}")))?;
        if c.guarded {
            let ok = match action.confirm_token {
                Some(t) => {
                    let now = self.clock.now_ms();
                    // Single use: the token is removed whether or not it matches.
                    let hit = self.pending.lock().remove(&t);
                    matches!(hit, Some((cid, exp)) if cid == c.id && exp > now)
                }
                None => false,
            };
            if !ok {
                return Ok(self.needs_confirm(c.id));
            }
        }
        self.exec
            .run(c.id)
            .await
            .map_err(|e| FeatureError::Other(user_message(&e)))?;
        Ok(Outcome::HideLauncher)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Fake {
        ran: Mutex<Vec<String>>,
        no_hib: bool,
    }
    #[async_trait]
    impl SysExec for Fake {
        async fn run(&self, id: &str) -> Result<(), WinError> {
            self.ran.lock().push(id.to_owned());
            if id == "wifi_on" {
                return Err(WinError::AccessDenied);
            }
            Ok(())
        }
        fn hibernate_available(&self) -> bool {
            !self.no_hib
        }
    }
    struct FakeClock(AtomicU64);
    impl Clock for FakeClock {
        fn now_ms(&self) -> u64 {
            self.0.load(Ordering::Relaxed)
        }
    }

    fn setup(no_hib: bool) -> (SysCmd, Arc<Fake>, Arc<FakeClock>) {
        let f = Arc::new(Fake {
            no_hib,
            ..Default::default()
        });
        let c = Arc::new(FakeClock(AtomicU64::new(1000)));
        (SysCmd::with(f.clone(), c.clone()), f, c)
    }
    fn input(t: &str) -> Input {
        Input {
            raw: t.into(),
            text: t.into(),
            mode: Mode::Command,
            seq: 1,
        }
    }
    fn item(s: &SysCmd, q: &str) -> Item {
        s.query(&input(q)).remove(0)
    }
    fn act(t: Option<String>) -> Action {
        Action {
            kind: ActionKind::Primary,
            confirm_token: t,
        }
    }
    fn token(o: Outcome) -> String {
        match o {
            Outcome::NeedsConfirm { token, expires_ms } => {
                assert_eq!(expires_ms, 3000);
                token
            }
            other => panic!("expected NeedsConfirm, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn first_enter_never_runs_second_runs_once() {
        let (s, f, c) = setup(false);
        let it = item(&s, "shut");
        let t = token(s.execute(&it, act(None)).await.unwrap());
        assert!(f.ran.lock().is_empty());
        c.0.store(3500, Ordering::Relaxed);
        let o = s.execute(&it, act(Some(t.clone()))).await.unwrap();
        assert_eq!(o, Outcome::HideLauncher);
        assert_eq!(*f.ran.lock(), ["shutdown"]);
        // token is single use
        token(s.execute(&it, act(Some(t))).await.unwrap());
        assert_eq!(f.ran.lock().len(), 1);
    }

    #[tokio::test]
    async fn expired_token_needs_new_confirm() {
        let (s, f, c) = setup(false);
        let it = item(&s, "restart");
        let t = token(s.execute(&it, act(None)).await.unwrap());
        c.0.store(1000 + 3100, Ordering::Relaxed);
        token(s.execute(&it, act(Some(t))).await.unwrap());
        assert!(f.ran.lock().is_empty());
    }

    #[tokio::test]
    async fn token_is_bound_to_its_command() {
        let (s, f, _c) = setup(false);
        let restart = item(&s, "reboot");
        let bin = item(&s, "empty bin");
        let t = token(s.execute(&restart, act(None)).await.unwrap());
        token(s.execute(&bin, act(Some(t))).await.unwrap());
        assert!(f.ran.lock().is_empty());
    }

    #[tokio::test]
    async fn unguarded_runs_once_and_errors_are_readable() {
        let (s, f, _c) = setup(false);
        let it = item(&s, "lock");
        assert_eq!(
            s.execute(&it, act(None)).await.unwrap(),
            Outcome::HideLauncher
        );
        assert_eq!(*f.ran.lock(), ["lock"]);
        let w = item(&s, "wifi on");
        let e = s.execute(&w, act(None)).await.unwrap_err();
        assert!(e.to_string().contains("denied"));
    }

    #[test]
    fn query_matches_aliases_and_hides_hibernate() {
        let (s, ..) = setup(false);
        assert_eq!(item(&s, "shut").title, "Shut down");
        assert_eq!(item(&s, "bt off").title, "Bluetooth off");
        assert_eq!(item(&s, "off").source, "syscmd");
        assert!(s.query(&input("xyz")).is_empty());
        assert_eq!(s.query(&input("hib")).len(), 1);
        let (s, ..) = setup(true);
        assert!(s.query(&input("hib")).is_empty());
    }
}
