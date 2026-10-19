//! F07 Ports and processes: `port <n>` and `proc <name>` (KICKOFF 1.3, AFD 7.7).
//!
//! `query()` only parses the text and returns one cheap row. The OS is read in `execute()`.
//! Killing needs a confirm token (3 s, single use) issued by a first `Kill` action.

use crate::ctx::FeatureCtx;
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::schema::{Field, FieldKind, SettingsSchema};
use crate::syscmd::Clock;
use arc_swap::ArcSwap;
use async_trait::async_trait;
use launcher_core::{Action, ActionKind, Config, Input, Item, ItemId, ItemKind, Mode, Outcome};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use win::WinError;

/// Window in which the confirming action must arrive.
const CONFIRM_MS: u64 = 3000;
/// Most rows one lookup returns.
const MAX_ROWS: usize = 50;
/// Longest command line shown, in characters.
const CMD_CHARS: usize = 80;

/// What the typed text asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ask {
    /// `port` (all listeners) or `port <n>`.
    Port(Option<u16>),
    /// `proc <name>`.
    Proc(String),
}

/// Parses the query text. Pure; no I/O.
pub fn parse(text: &str) -> Option<Ask> {
    let t = text.trim();
    let (head, rest) = t.split_once(char::is_whitespace).unwrap_or((t, ""));
    let rest = rest.trim();
    match head.to_lowercase().as_str() {
        "port" if rest.is_empty() => Some(Ask::Port(None)),
        "port" => rest.parse::<u16>().ok().map(|n| Ask::Port(Some(n))),
        "proc" if !rest.is_empty() => Some(Ask::Proc(rest.to_lowercase())),
        _ => None,
    }
}

/// One process with what it holds, as read from the OS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcRow {
    /// Process id.
    pub pid: u32,
    /// Executable name.
    pub name: String,
    /// Resident memory in bytes.
    pub mem_bytes: u64,
    /// Seconds since it started.
    pub up_secs: u64,
    /// Command line, joined.
    pub cmd: String,
    /// Port entries such as `TCP 127.0.0.1:3000 LISTEN`.
    pub conns: Vec<String>,
}

fn fmt_mem(b: u64) -> String {
    const MB: u64 = 1024 * 1024;
    if b >= 1024 * MB {
        format!("{:.1} GB", b as f64 / (1024 * MB) as f64)
    } else {
        format!("{} MB", b / MB)
    }
}

fn fmt_up(s: u64) -> String {
    match s {
        0..=59 => format!("{s} s"),
        60..=3599 => format!("{} min", s / 60),
        3600..=86399 => format!("{} h {} min", s / 3600, s % 3600 / 60),
        _ => format!("{} d {} h", s / 86400, s % 86400 / 3600),
    }
}

/// Title and subtitle of a row. Pure, so it is tested from fake data.
pub fn format_row(r: &ProcRow, show_cmd: bool) -> (String, String) {
    let mut parts = Vec::new();
    if !r.conns.is_empty() {
        parts.push(r.conns.join(", "));
    }
    parts.push(fmt_mem(r.mem_bytes));
    parts.push(format!("up {}", fmt_up(r.up_secs)));
    if show_cmd && !r.cmd.is_empty() {
        let cut: String = r.cmd.chars().take(CMD_CHARS).collect();
        let dots = if r.cmd.chars().count() > CMD_CHARS {
            "…"
        } else {
            ""
        };
        parts.push(format!("{cut}{dots}"));
    }
    (format!("{} · PID {}", r.name, r.pid), parts.join(" · "))
}

fn process_item(r: &ProcRow, show_cmd: bool, score: f32) -> Item {
    let (title, subtitle) = format_row(r, show_cmd);
    Item {
        id: ItemId(format!("ports:pid:{}", r.pid)),
        kind: ItemKind::Process,
        title,
        subtitle,
        icon: None,
        score,
        source: "ports".to_owned(),
        actions: vec![ActionKind::Kill],
        payload: serde_json::json!({ "pid": r.pid, "name": r.name }),
    }
}

/// Reads processes and ends them. The real impl uses `crates/win` and sysinfo; tests inject a fake.
#[async_trait]
pub trait Os: Send + Sync {
    /// Processes holding `port`, or every listener when `None`.
    async fn by_port(&self, port: Option<u16>) -> Result<Vec<ProcRow>, String>;
    /// Processes whose name contains `needle` (lowercase).
    async fn by_name(&self, needle: &str) -> Result<Vec<ProcRow>, String>;
    /// Ends a process.
    async fn kill(&self, pid: u32) -> Result<(), WinError>;
}

/// Real OS access.
pub struct WinOs;

fn rows_from(
    pids: &HashMap<u32, Vec<String>>,
    name_filter: Option<&str>,
) -> Result<Vec<ProcRow>, String> {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    let kind = ProcessRefreshKind::nothing()
        .with_memory()
        .with_cmd(UpdateKind::OnlyIfNotSet);
    let mut sys = System::new();
    if name_filter.is_some() {
        sys.refresh_processes_specifics(ProcessesToUpdate::All, true, kind);
    } else {
        let ids: Vec<Pid> = pids.keys().map(|p| Pid::from_u32(*p)).collect();
        sys.refresh_processes_specifics(ProcessesToUpdate::Some(&ids), true, kind);
    }
    let mut out = Vec::new();
    for (pid, p) in sys.processes() {
        let name = p.name().to_string_lossy().into_owned();
        if let Some(n) = name_filter
            && !name.to_lowercase().contains(n)
        {
            continue;
        }
        let pid = pid.as_u32();
        if name_filter.is_none() && !pids.contains_key(&pid) {
            continue;
        }
        out.push(ProcRow {
            pid,
            name,
            mem_bytes: p.memory(),
            up_secs: p.run_time(),
            cmd: p
                .cmd()
                .iter()
                .map(|s| s.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" "),
            conns: pids.get(&pid).cloned().unwrap_or_default(),
        });
    }
    Ok(out)
}

fn conn_text(o: &win::ports::PortOwner) -> String {
    use win::ports::Proto;
    match o.proto {
        Proto::Tcp => format!(
            "TCP {}:{} {}",
            o.local_addr,
            o.local_port,
            o.state
                .map(|s| format!("{s:?}").to_uppercase())
                .unwrap_or_default()
        ),
        Proto::Udp => format!("UDP {}:{}", o.local_addr, o.local_port),
    }
}

#[async_trait]
impl Os for WinOs {
    async fn by_port(&self, port: Option<u16>) -> Result<Vec<ProcRow>, String> {
        tokio::task::spawn_blocking(move || {
            use win::ports::{Proto, TcpState};
            let owners = match port {
                Some(n) => win::ports::port_owners(n),
                None => win::ports::all_ports().map(|v| {
                    v.into_iter()
                        .filter(|o| o.proto == Proto::Udp || o.state == Some(TcpState::Listen))
                        .collect()
                }),
            }
            .map_err(|e| e.to_string())?;
            let mut pids: HashMap<u32, Vec<String>> = HashMap::new();
            for o in &owners {
                pids.entry(o.pid).or_default().push(conn_text(o));
            }
            rows_from(&pids, None)
        })
        .await
        .map_err(|e| e.to_string())?
    }
    async fn by_name(&self, needle: &str) -> Result<Vec<ProcRow>, String> {
        let n = needle.to_owned();
        tokio::task::spawn_blocking(move || rows_from(&HashMap::new(), Some(&n)))
            .await
            .map_err(|e| e.to_string())?
    }
    async fn kill(&self, pid: u32) -> Result<(), WinError> {
        tokio::task::spawn_blocking(move || win::ports::kill_process(pid))
            .await
            .unwrap_or(Err(WinError::Cancelled))
    }
}

/// Monotonic clock for the confirm window.
struct Mono(Instant);

impl Clock for Mono {
    fn now_ms(&self) -> u64 {
        u64::try_from(self.0.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}

/// Ports and processes.
pub struct Ports {
    os: Arc<dyn Os>,
    clock: Arc<dyn Clock>,
    config: Mutex<Option<Arc<ArcSwap<Config>>>>,
    /// token -> (pid, expiry ms)
    pending: Mutex<HashMap<String, (u32, u64)>>,
    counter: AtomicU64,
}

impl Default for Ports {
    fn default() -> Self {
        Self::with(Arc::new(WinOs), Arc::new(Mono(Instant::now())))
    }
}

impl Ports {
    /// Builds the feature with an injected OS and clock.
    pub fn with(os: Arc<dyn Os>, clock: Arc<dyn Clock>) -> Self {
        Self {
            os,
            clock,
            config: Mutex::new(None),
            pending: Mutex::new(HashMap::new()),
            counter: AtomicU64::new(0),
        }
    }

    fn show_cmd(&self) -> bool {
        self.config
            .lock()
            .as_ref()
            .and_then(|c| {
                c.load()
                    .features
                    .get("ports")?
                    .get("show_command_line")?
                    .as_bool()
            })
            .unwrap_or(true)
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

    fn needs_confirm(&self, pid: u32) -> Outcome {
        let token = self.new_token();
        let now = self.clock.now_ms();
        let mut p = self.pending.lock();
        p.retain(|_, (_, exp)| *exp > now);
        p.insert(token.clone(), (pid, now + CONFIRM_MS));
        Outcome::NeedsConfirm {
            token,
            expires_ms: CONFIRM_MS,
        }
    }

    async fn kill(&self, item: &Item, action: Action) -> Result<Outcome, FeatureError> {
        let pid = item
            .payload
            .get("pid")
            .and_then(serde_json::Value::as_u64)
            .and_then(|p| u32::try_from(p).ok())
            .ok_or_else(|| FeatureError::Other("missing pid".into()))?;
        let ok = match action.confirm_token {
            Some(t) => {
                let now = self.clock.now_ms();
                // Single use: removed whether or not it matches.
                matches!(self.pending.lock().remove(&t), Some((p, exp)) if p == pid && exp > now)
            }
            None => false,
        };
        if !ok {
            return Ok(self.needs_confirm(pid));
        }
        let name = item
            .payload
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("the process");
        self.os.kill(pid).await.map_err(|e| {
            FeatureError::Other(match e {
                WinError::AccessDenied => {
                    format!("Could not end {name}. It may need admin rights.")
                }
                other => format!("Could not end {name}: {other}"),
            })
        })?;
        Ok(Outcome::Done)
    }
}

#[async_trait]
impl Feature for Ports {
    fn id(&self) -> &'static str {
        "ports"
    }
    fn title(&self) -> &'static str {
        "Ports and processes"
    }
    fn version_tag(&self) -> &'static str {
        "v0.1.0"
    }
    fn description(&self) -> &'static str {
        "port 3000 shows what holds a port; proc <name> searches processes; Backspace ends one after a confirm."
    }
    fn default_enabled(&self) -> bool {
        true
    }
    fn modes(&self) -> &'static [Mode] {
        &[Mode::Command]
    }
    fn settings_schema(&self) -> SettingsSchema {
        SettingsSchema {
            fields: vec![Field {
                key: "show_command_line".into(),
                label: "Show command line".into(),
                help: "Add each process's command line to its row.".into(),
                kind: FieldKind::Bool,
                default: serde_json::json!(true),
                restart: false,
            }],
        }
    }
    fn start(&self, ctx: FeatureCtx) -> Result<(), FeatureError> {
        *self.config.lock() = Some(ctx.config);
        Ok(())
    }
    fn stop(&self) {
        *self.config.lock() = None;
        self.pending.lock().clear();
    }
    fn query(&self, input: &Input) -> Vec<Item> {
        let Some(ask) = parse(&input.text) else {
            return Vec::new();
        };
        let (id, title, payload) = match &ask {
            Ask::Port(Some(n)) => (
                format!("ports:port:{n}"),
                format!("Show what's on port {n}"),
                serde_json::json!({ "port": n }),
            ),
            Ask::Port(None) => (
                "ports:port".to_owned(),
                "Show listening ports".to_owned(),
                serde_json::json!({ "port": null }),
            ),
            Ask::Proc(q) => (
                format!("ports:proc:{q}"),
                format!("Search processes: {q}"),
                serde_json::json!({ "proc": q }),
            ),
        };
        vec![Item {
            id: ItemId(id),
            kind: ItemKind::Command,
            title,
            subtitle: "Ports and processes".to_owned(),
            icon: None,
            score: 1.0,
            source: "ports".to_owned(),
            actions: vec![ActionKind::Primary],
            payload,
        }]
    }
    async fn execute(&self, item: &Item, action: Action) -> Result<Outcome, FeatureError> {
        if item.kind == ItemKind::Process {
            return match action.kind {
                ActionKind::Kill => self.kill(item, action).await,
                _ => Ok(Outcome::Done),
            };
        }
        let by_name = item.payload.get("proc").and_then(|v| v.as_str());
        let mut rows = match by_name {
            Some(q) => self.os.by_name(q).await,
            None => {
                let port = item
                    .payload
                    .get("port")
                    .and_then(serde_json::Value::as_u64)
                    .and_then(|p| u16::try_from(p).ok());
                self.os.by_port(port).await
            }
        }
        .map_err(|e| FeatureError::Other(format!("Could not read processes: {e}")))?;
        if by_name.is_some() {
            rows.sort_by(|a, b| b.mem_bytes.cmp(&a.mem_bytes).then(a.pid.cmp(&b.pid)));
        } else {
            rows.sort_by_key(|r| r.pid);
        }
        rows.truncate(MAX_ROWS);
        let show_cmd = self.show_cmd();
        let items = rows
            .iter()
            .enumerate()
            .map(|(i, r)| process_item(r, show_cmd, 1.0 - i as f32 / 1000.0))
            .collect();
        Ok(Outcome::ShowItems { items })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake {
        killed: Mutex<Vec<u32>>,
        deny: bool,
    }
    #[async_trait]
    impl Os for Fake {
        async fn by_port(&self, _: Option<u16>) -> Result<Vec<ProcRow>, String> {
            Ok(vec![row(20, "b.exe"), row(10, "a.exe")])
        }
        async fn by_name(&self, _: &str) -> Result<Vec<ProcRow>, String> {
            Ok(vec![row(1, "chrome.exe")])
        }
        async fn kill(&self, pid: u32) -> Result<(), WinError> {
            if self.deny {
                return Err(WinError::AccessDenied);
            }
            self.killed.lock().push(pid);
            Ok(())
        }
    }
    struct FakeClock(AtomicU64);
    impl Clock for FakeClock {
        fn now_ms(&self) -> u64 {
            self.0.load(Ordering::Relaxed)
        }
    }

    fn row(pid: u32, name: &str) -> ProcRow {
        ProcRow {
            pid,
            name: name.into(),
            mem_bytes: 120 * 1024 * 1024,
            up_secs: 7300,
            cmd: "x".repeat(200),
            conns: vec!["TCP 127.0.0.1:3000 LISTEN".into()],
        }
    }
    fn setup(deny: bool) -> (Ports, Arc<Fake>, Arc<FakeClock>) {
        let f = Arc::new(Fake {
            killed: Mutex::new(vec![]),
            deny,
        });
        let c = Arc::new(FakeClock(AtomicU64::new(1000)));
        (Ports::with(f.clone(), c.clone()), f, c)
    }
    fn input(t: &str) -> Input {
        Input {
            raw: t.into(),
            text: t.into(),
            mode: Mode::Command,
            seq: 1,
        }
    }
    fn act(kind: ActionKind, t: Option<String>) -> Action {
        Action {
            kind,
            confirm_token: t,
        }
    }
    fn token(o: Outcome) -> String {
        match o {
            Outcome::NeedsConfirm { token, .. } => token,
            other => panic!("expected NeedsConfirm, got {other:?}"),
        }
    }

    #[test]
    fn parses() {
        assert_eq!(parse("port 3000"), Some(Ask::Port(Some(3000))));
        assert_eq!(parse(" Port "), Some(Ask::Port(None)));
        assert_eq!(parse("proc Chrome"), Some(Ask::Proc("chrome".into())));
        for bad in ["", "proc", "port abc", "port 99999", "portal", "chrome"] {
            assert_eq!(parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn formats_row() {
        let (t, s) = format_row(&row(1234, "node.exe"), false);
        assert_eq!(t, "node.exe · PID 1234");
        assert_eq!(s, "TCP 127.0.0.1:3000 LISTEN · 120 MB · up 2 h 1 min");
        let (_, s) = format_row(&row(1, "n"), true);
        assert!(s.ends_with('…') && s.contains(&"x".repeat(80)));
    }

    #[test]
    fn query_is_cheap_and_single_row() {
        let (p, _, _) = setup(false);
        let t = Instant::now();
        let r = p.query(&input("port 3000"));
        assert!(t.elapsed().as_micros() < 500);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].title, "Show what's on port 3000");
        assert!(p.query(&input("hello")).is_empty());
    }

    #[tokio::test]
    async fn execute_lists_sorted_processes() {
        let (p, _, _) = setup(false);
        let q = p.query(&input("port 3000")).remove(0);
        let o = p.execute(&q, act(ActionKind::Primary, None)).await.unwrap();
        let Outcome::ShowItems { items } = o else {
            panic!("expected items");
        };
        assert_eq!(items[0].id.0, "ports:pid:10");
        assert_eq!(items[0].kind, ItemKind::Process);
    }

    #[tokio::test]
    async fn kill_needs_confirm() {
        let (p, f, c) = setup(false);
        let it = process_item(&row(10, "a.exe"), false, 1.0);
        let t = token(p.execute(&it, act(ActionKind::Kill, None)).await.unwrap());
        assert!(f.killed.lock().is_empty());
        c.0.store(2000, Ordering::Relaxed);
        let o = p
            .execute(&it, act(ActionKind::Kill, Some(t.clone())))
            .await
            .unwrap();
        assert_eq!(o, Outcome::Done);
        assert_eq!(*f.killed.lock(), [10]);
        // single use
        let again = p
            .execute(&it, act(ActionKind::Kill, Some(t)))
            .await
            .unwrap();
        assert!(matches!(again, Outcome::NeedsConfirm { .. }));
        assert_eq!(f.killed.lock().len(), 1);
    }

    #[tokio::test]
    async fn expired_or_wrong_pid_token_does_not_kill() {
        let (p, f, c) = setup(false);
        let a = process_item(&row(10, "a.exe"), false, 1.0);
        let b = process_item(&row(11, "b.exe"), false, 1.0);
        let t = token(p.execute(&a, act(ActionKind::Kill, None)).await.unwrap());
        let o = p.execute(&b, act(ActionKind::Kill, Some(t))).await.unwrap();
        assert!(matches!(o, Outcome::NeedsConfirm { .. }));
        let t = token(p.execute(&a, act(ActionKind::Kill, None)).await.unwrap());
        c.0.store(1000 + 3100, Ordering::Relaxed);
        let o = p.execute(&a, act(ActionKind::Kill, Some(t))).await.unwrap();
        assert!(matches!(o, Outcome::NeedsConfirm { .. }));
        assert!(f.killed.lock().is_empty());
    }

    #[tokio::test]
    async fn access_denied_says_admin() {
        let (p, _, _) = setup(true);
        let it = process_item(&row(10, "a.exe"), false, 1.0);
        let t = token(p.execute(&it, act(ActionKind::Kill, None)).await.unwrap());
        let e = p
            .execute(&it, act(ActionKind::Kill, Some(t)))
            .await
            .unwrap_err();
        assert!(e.to_string().contains("may need admin rights"));
    }
}
