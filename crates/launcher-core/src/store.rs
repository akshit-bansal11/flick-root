//! SQLite store (BSD section 6.2): frecency, prefix learning, NL cache and rate cache.
//!
//! One dedicated std thread owns the write connection and receives [`WriteMsg`] over an mpsc
//! channel; reads use a separate query-only connection behind a mutex. All methods are
//! synchronous: async callers must wrap them in `tokio::task::spawn_blocking`.
//! Writes are batched for up to 2 s (write-behind); reads see only committed data, so they can
//! be up to 2 s stale. Features therefore keep their own in-memory frecency and update it on
//! launch. Call [`Store::shutdown`] on app exit so the last batch is committed.
//! Write failures on the store thread never panic; see [`Store::error_count`] and
//! [`Store::last_error`].

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, ErrorCode, OpenFlags, params};

use crate::error::CoreError;

const SCHEMA_VERSION: i64 = 1;
const DAY_MS: f64 = 86_400_000.0;
const WRITE_BEHIND: Duration = Duration::from_secs(2);
const NL_CACHE_MAX: i64 = 500;
const PRUNE_SCORE: f64 = 0.05;
const PRUNE_AGE_DAYS: i64 = 90;
const DEFAULT_HALF_LIFE_DAYS: u32 = 7;
static MEM_ID: AtomicU64 = AtomicU64::new(0);

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS frecency (
  item_key TEXT PRIMARY KEY, kind TEXT NOT NULL, score REAL NOT NULL,
  uses INTEGER NOT NULL DEFAULT 1, last_used INTEGER NOT NULL);
CREATE INDEX IF NOT EXISTS idx_frecency_last_used ON frecency(last_used);
CREATE TABLE IF NOT EXISTS prefix_learning (
  prefix TEXT NOT NULL, item_key TEXT NOT NULL, count INTEGER NOT NULL DEFAULT 1,
  last_used INTEGER NOT NULL, PRIMARY KEY (prefix, item_key)) WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS idx_prefix_item ON prefix_learning(item_key);
CREATE TABLE IF NOT EXISTS nl_cache (
  cache_key TEXT PRIMARY KEY, phrase TEXT NOT NULL, chosen_path TEXT NOT NULL,
  engine TEXT NOT NULL, p REAL, hits INTEGER NOT NULL DEFAULT 1,
  created_at INTEGER NOT NULL, last_hit INTEGER NOT NULL);
CREATE INDEX IF NOT EXISTS idx_nl_last_hit ON nl_cache(last_hit);
CREATE TABLE IF NOT EXISTS rate_cache (
  currency TEXT PRIMARY KEY, per_eur REAL NOT NULL, rate_date TEXT NOT NULL,
  fetched_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS secret_meta (
  name TEXT PRIMARY KEY, host TEXT NOT NULL, added_at INTEGER NOT NULL, last_used INTEGER);
";

/// Result of [`Store::open`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenReport {
    /// True when a corrupt or unreadable file was moved aside and a fresh one created.
    pub reset: bool,
}

/// Which data [`Store::clear`] removes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearScope {
    /// Frecency and prefix learning.
    Frecency,
    /// The natural-language cache.
    NlCache,
    /// Frecency, prefix learning, NL cache and rate cache (not secret metadata).
    All,
}

/// One frecency row. `score` is as of `last_used_ms`.
#[derive(Debug, Clone, PartialEq)]
pub struct FrecencyRow {
    /// Item key such as `app:<parsing name>`.
    pub item_key: String,
    /// Item kind.
    pub kind: String,
    /// Score as of `last_used_ms`.
    pub score: f64,
    /// Number of launches.
    pub uses: i64,
    /// Last launch, Unix ms.
    pub last_used_ms: i64,
}

/// One NL cache entry.
#[derive(Debug, Clone, PartialEq)]
pub struct NlCacheEntry {
    /// Hash of normalized phrase and roots.
    pub cache_key: String,
    /// The phrase.
    pub phrase: String,
    /// Chosen path.
    pub chosen_path: String,
    /// Engine that chose it.
    pub engine: String,
    /// Confidence, if any.
    pub p: Option<f64>,
}

/// One cached exchange rate.
#[derive(Debug, Clone, PartialEq)]
pub struct RateRow {
    /// Currency code.
    pub currency: String,
    /// Units per euro.
    pub per_eur: f64,
    /// ECB reference date.
    pub rate_date: String,
    /// Fetch time, Unix ms.
    pub fetched_at: i64,
}

/// A message for the store's write thread.
#[derive(Debug)]
pub enum WriteMsg {
    /// Record a launch (and the prefix choice when a prefix is given).
    RecordLaunch {
        /// Item key.
        item_key: String,
        /// Item kind.
        kind: String,
        /// Typed prefix (lowercased by the store).
        prefix: Option<String>,
        /// Time, Unix ms.
        ts_ms: i64,
    },
    /// Record that `prefix` led to `item_key`.
    PrefixChoice {
        /// Typed prefix.
        prefix: String,
        /// Chosen item.
        item_key: String,
        /// Time, Unix ms.
        ts_ms: i64,
    },
    /// Delete data.
    Clear(ClearScope),
    /// Insert or replace an NL cache entry.
    NlPut(NlCacheEntry, i64),
    /// Upsert exchange rates.
    RatesUpsert(Vec<RateRow>),
    /// Reply once every earlier message has been processed.
    Flush(Sender<()>),
    /// Stop the thread.
    Shutdown,
}

struct Inner {
    tx: Mutex<Option<Sender<WriteMsg>>>,
    handle: Mutex<Option<JoinHandle<()>>>,
    read: Mutex<Connection>,
    errors: Arc<AtomicU64>,
    last_error: Arc<Mutex<Option<String>>>,
    half_life_days: Arc<AtomicU32>,
}

/// Handle to the store. Cloning is cheap and shares one write thread.
#[derive(Clone)]
pub struct Store(Arc<Inner>);

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(0))
}

fn is_corrupt(e: &rusqlite::Error) -> bool {
    matches!(
        e,
        rusqlite::Error::SqliteFailure(f, _)
            if matches!(f.code, ErrorCode::NotADatabase | ErrorCode::DatabaseCorrupt)
    )
}

fn pragmas(c: &Connection) -> rusqlite::Result<()> {
    c.busy_timeout(std::time::Duration::from_millis(5000))?;
    c.query_row("PRAGMA journal_mode=WAL", [], |_| Ok(()))?;
    c.execute_batch("PRAGMA synchronous=NORMAL;")
}

/// Open the write connection, check integrity and create/verify the schema.
fn init_write(uri_or_path: &str, flags: OpenFlags) -> Result<Connection, CoreError> {
    let c = Connection::open_with_flags(uri_or_path, flags)?;
    pragmas(&c)?;
    let ok: String = c.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
    if ok != "ok" {
        return Err(CoreError::StoreReset);
    }
    let v: i64 = c.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if v != 0 && v != SCHEMA_VERSION {
        return Err(CoreError::StoreReset);
    }
    c.execute_batch(SCHEMA)?;
    c.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION};"))?;
    Ok(c)
}

fn prune(c: &Connection, now: i64, half_life_days: f64) -> rusqlite::Result<()> {
    let cutoff = now - PRUNE_AGE_DAYS * 86_400_000;
    let mut stale = Vec::new();
    {
        let mut st =
            c.prepare("SELECT item_key, score, last_used FROM frecency WHERE last_used < ?1")?;
        let rows = st.query_map([cutoff], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, f64>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })?;
        for row in rows {
            let (k, s, t) = row?;
            if decayed(s, t, now, half_life_days) < PRUNE_SCORE {
                stale.push(k);
            }
        }
    }
    for k in stale {
        c.execute("DELETE FROM frecency WHERE item_key = ?1", [k])?;
    }
    c.execute(
        "DELETE FROM nl_cache WHERE cache_key NOT IN
         (SELECT cache_key FROM nl_cache ORDER BY last_hit DESC LIMIT ?1)",
        [NL_CACHE_MAX],
    )?;
    Ok(())
}

fn decayed(score: f64, last_used: i64, now: i64, half_life_days: f64) -> f64 {
    let dt = (now - last_used).max(0) as f64 / DAY_MS;
    score * 0.5f64.powf(dt / half_life_days.max(0.001))
}

fn handle_msg(c: &Connection, m: WriteMsg, half_life_days: f64) -> rusqlite::Result<()> {
    match m {
        WriteMsg::RecordLaunch {
            item_key,
            kind,
            prefix,
            ts_ms,
        } => {
            let old: Option<(f64, i64)> = c
                .query_row(
                    "SELECT score, last_used FROM frecency WHERE item_key = ?1",
                    [&item_key],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .ok();
            let score = old.map_or(0.0, |(s, t)| decayed(s, t, ts_ms, half_life_days)) + 1.0;
            c.execute(
                "INSERT INTO frecency (item_key, kind, score, uses, last_used) VALUES (?1, ?2, ?3, 1, ?4)
                 ON CONFLICT(item_key) DO UPDATE SET kind = ?2, score = ?3, uses = uses + 1, last_used = ?4",
                params![item_key, kind, score, ts_ms],
            )?;
            if let Some(p) = prefix {
                prefix_upsert(c, &p, &item_key, ts_ms)?;
            }
        }
        WriteMsg::PrefixChoice {
            prefix,
            item_key,
            ts_ms,
        } => prefix_upsert(c, &prefix, &item_key, ts_ms)?,
        WriteMsg::Clear(scope) => {
            let sql = match scope {
                ClearScope::Frecency => "DELETE FROM frecency; DELETE FROM prefix_learning;",
                ClearScope::NlCache => "DELETE FROM nl_cache;",
                ClearScope::All => {
                    "DELETE FROM frecency; DELETE FROM prefix_learning; DELETE FROM nl_cache; DELETE FROM rate_cache;"
                }
            };
            c.execute_batch(sql)?;
        }
        WriteMsg::NlPut(e, ts) => {
            c.execute(
                "INSERT INTO nl_cache (cache_key, phrase, chosen_path, engine, p, hits, created_at, last_hit)
                 VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?6)
                 ON CONFLICT(cache_key) DO UPDATE SET phrase = ?2, chosen_path = ?3, engine = ?4, p = ?5,
                   hits = hits + 1, last_hit = ?6",
                params![e.cache_key, e.phrase, e.chosen_path, e.engine, e.p, ts],
            )?;
        }
        WriteMsg::RatesUpsert(rows) => {
            for r in rows {
                c.execute(
                    "INSERT OR REPLACE INTO rate_cache (currency, per_eur, rate_date, fetched_at) VALUES (?1, ?2, ?3, ?4)",
                    params![r.currency, r.per_eur, r.rate_date, r.fetched_at],
                )?;
            }
        }
        WriteMsg::Flush(tx) => {
            // Receiver may have gone away; nothing to do then.
            let _ = tx.send(());
        }
        WriteMsg::Shutdown => {}
    }
    Ok(())
}

fn prefix_upsert(c: &Connection, prefix: &str, item_key: &str, ts: i64) -> rusqlite::Result<()> {
    c.execute(
        "INSERT INTO prefix_learning (prefix, item_key, count, last_used) VALUES (?1, ?2, 1, ?3)
         ON CONFLICT(prefix, item_key) DO UPDATE SET count = count + 1, last_used = ?3",
        params![prefix.to_lowercase(), item_key, ts],
    )?;
    Ok(())
}

impl Store {
    /// Open (or create) the store at `path`. A corrupt or unreadable file is renamed to
    /// `<name>.corrupt-<unix ts>` and replaced; the report then has `reset = true`.
    pub fn open(path: &Path) -> Result<(Store, OpenReport), CoreError> {
        let p = path.to_string_lossy().into_owned();
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let (w, reset) = match init_write(&p, flags) {
            Ok(c) => (c, false),
            Err(CoreError::StoreReset) => (Self::reset_file(path, &p, flags)?, true),
            Err(CoreError::Store(e)) if is_corrupt(&e) => {
                (Self::reset_file(path, &p, flags)?, true)
            }
            Err(e) => return Err(e),
        };
        let r = Connection::open_with_flags(&p, flags)?;
        r.busy_timeout(std::time::Duration::from_millis(5000))?;
        r.execute_batch("PRAGMA query_only=ON;")?;
        Ok((Self::start(w, r), OpenReport { reset }))
    }

    fn reset_file(path: &Path, p: &str, flags: OpenFlags) -> Result<Connection, CoreError> {
        let mut name = path
            .file_name()
            .map(|n| n.to_os_string())
            .unwrap_or_default();
        name.push(format!(".corrupt-{}", now_ms() / 1000));
        let dest: PathBuf = path.with_file_name(name);
        std::fs::rename(path, &dest).map_err(|source| CoreError::ConfigIo {
            path: path.to_path_buf(),
            source,
        })?;
        for ext in ["-wal", "-shm"] {
            let mut s = path.as_os_str().to_os_string();
            s.push(ext);
            // Stale sidecars of the corrupt file must not attach to the new one.
            let _ = std::fs::remove_file(s);
        }
        init_write(p, flags)
    }

    /// Open a private in-memory store (tests).
    pub fn open_in_memory() -> Result<Store, CoreError> {
        let uri = format!(
            "file:store_mem_{}?mode=memory&cache=shared",
            MEM_ID.fetch_add(1, Ordering::Relaxed)
        );
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_URI
            | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let w = Connection::open_with_flags(&uri, flags)?;
        w.execute_batch(SCHEMA)?;
        w.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION};"))?;
        let r = Connection::open_with_flags(&uri, flags)?;
        r.execute_batch("PRAGMA query_only=ON;")?;
        Ok(Self::start(w, r))
    }

    fn start(w: Connection, r: Connection) -> Store {
        let half = Arc::new(AtomicU32::new(DEFAULT_HALF_LIFE_DAYS));
        let errors = Arc::new(AtomicU64::new(0));
        let last_error = Arc::new(Mutex::new(None));
        let hl = f64::from(half.load(Ordering::Relaxed));
        if let Err(e) = prune(&w, now_ms(), hl) {
            errors.fetch_add(1, Ordering::Relaxed);
            *lock(&last_error) = Some(e.to_string());
        }
        let (tx, rx) = mpsc::channel::<WriteMsg>();
        let (h2, e2, l2) = (half.clone(), errors.clone(), last_error.clone());
        let handle = std::thread::spawn(move || {
            // Write-behind: one open transaction, committed at most WRITE_BEHIND after its first write.
            let mut since: Option<Instant> = None;
            let commit = |since: &mut Option<Instant>| {
                if since.take().is_some()
                    && let Err(e) = w.execute_batch("COMMIT")
                {
                    e2.fetch_add(1, Ordering::Relaxed);
                    *lock(&l2) = Some(e.to_string());
                }
            };
            loop {
                let got = match since {
                    Some(t) => rx.recv_timeout(WRITE_BEHIND.saturating_sub(t.elapsed())),
                    None => rx.recv().map_err(|_| mpsc::RecvTimeoutError::Disconnected),
                };
                let msg = match got {
                    Ok(WriteMsg::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        commit(&mut since);
                        continue;
                    }
                    Ok(m) => m,
                };
                let flush = matches!(msg, WriteMsg::Flush(_));
                if !flush && since.is_none() && w.execute_batch("BEGIN").is_ok() {
                    since = Some(Instant::now());
                }
                if flush {
                    commit(&mut since);
                }
                let hl = f64::from(h2.load(Ordering::Relaxed));
                if let Err(e) = handle_msg(&w, msg, hl) {
                    e2.fetch_add(1, Ordering::Relaxed);
                    *lock(&l2) = Some(e.to_string());
                }
            }
            commit(&mut since);
        });
        Store(Arc::new(Inner {
            tx: Mutex::new(Some(tx)),
            handle: Mutex::new(Some(handle)),
            read: Mutex::new(r),
            errors,
            last_error,
            half_life_days: half,
        }))
    }

    /// Set the frecency half-life in days (config `frecency.half_life_days`).
    pub fn set_half_life_days(&self, days: u32) {
        self.0.half_life_days.store(days.max(1), Ordering::Relaxed);
    }

    /// Queue a raw write message. Returns false if the store has been shut down.
    pub fn send(&self, msg: WriteMsg) -> bool {
        lock(&self.0.tx)
            .as_ref()
            .is_some_and(|tx| tx.send(msg).is_ok())
    }

    /// Record a launch, and the prefix choice when `prefix` is given.
    pub fn record_launch(&self, item_key: &str, kind: &str, prefix: Option<&str>, ts_ms: i64) {
        self.send(WriteMsg::RecordLaunch {
            item_key: item_key.to_owned(),
            kind: kind.to_owned(),
            prefix: prefix.map(str::to_owned),
            ts_ms,
        });
    }

    /// Record that `prefix` led to `item_key`.
    pub fn record_prefix_choice(&self, prefix: &str, item_key: &str, ts_ms: i64) {
        self.send(WriteMsg::PrefixChoice {
            prefix: prefix.to_owned(),
            item_key: item_key.to_owned(),
            ts_ms,
        });
    }

    /// Delete data in `scope`. Unlike other writes this commits immediately (it blocks like
    /// [`Store::flush`]) so a privacy clear is never lost to the write-behind window or a crash.
    pub fn clear(&self, scope: ClearScope) {
        if self.send(WriteMsg::Clear(scope)) {
            self.flush();
        }
    }

    /// Insert or refresh an NL cache entry.
    pub fn nl_cache_put(&self, e: NlCacheEntry) {
        self.send(WriteMsg::NlPut(e, now_ms()));
    }

    /// Upsert exchange rates.
    pub fn rates_upsert(&self, rows: Vec<RateRow>) {
        self.send(WriteMsg::RatesUpsert(rows));
    }

    /// Block until every queued write has been applied.
    pub fn flush(&self) {
        let (tx, rx) = mpsc::channel();
        if self.send(WriteMsg::Flush(tx)) {
            // A closed channel means the thread exited; nothing left to wait for.
            let _ = rx.recv();
        }
    }

    /// Number of write failures seen on the store thread.
    pub fn error_count(&self) -> u64 {
        self.0.errors.load(Ordering::Relaxed)
    }

    /// Message of the most recent write failure, if any.
    pub fn last_error(&self) -> Option<String> {
        lock(&self.0.last_error).clone()
    }

    /// Stop the write thread after draining queued messages, and join it. Idempotent.
    pub fn shutdown(&self) {
        if let Some(tx) = lock(&self.0.tx).take() {
            // Thread may already be gone.
            let _ = tx.send(WriteMsg::Shutdown);
        }
        if let Some(h) = lock(&self.0.handle).take() {
            // A panicked thread has nothing more to report.
            let _ = h.join();
        }
    }

    /// Bulk-load all frecency rows for in-memory ranking.
    pub fn load_frecency(&self) -> Result<Vec<FrecencyRow>, CoreError> {
        let c = lock(&self.0.read);
        let mut st = c.prepare("SELECT item_key, kind, score, uses, last_used FROM frecency")?;
        let rows = st.query_map([], |r| {
            Ok(FrecencyRow {
                item_key: r.get(0)?,
                kind: r.get(1)?,
                score: r.get(2)?,
                uses: r.get(3)?,
                last_used_ms: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// The item most often chosen for `prefix` (case-insensitive), if any.
    pub fn prefix_choice(&self, prefix: &str) -> Result<Option<String>, CoreError> {
        let c = lock(&self.0.read);
        let mut st = c.prepare(
            "SELECT item_key FROM prefix_learning WHERE prefix = ?1 ORDER BY count DESC, last_used DESC LIMIT 1",
        )?;
        let mut rows = st.query([prefix.to_lowercase()])?;
        Ok(rows.next()?.map(|r| r.get(0)).transpose()?)
    }

    /// Look up an NL cache entry by key.
    pub fn nl_cache_get(&self, cache_key: &str) -> Result<Option<NlCacheEntry>, CoreError> {
        let c = lock(&self.0.read);
        let mut st = c.prepare(
            "SELECT cache_key, phrase, chosen_path, engine, p FROM nl_cache WHERE cache_key = ?1",
        )?;
        let mut rows = st.query([cache_key])?;
        let e = rows
            .next()?
            .map(|r| {
                Ok::<_, rusqlite::Error>(NlCacheEntry {
                    cache_key: r.get(0)?,
                    phrase: r.get(1)?,
                    chosen_path: r.get(2)?,
                    engine: r.get(3)?,
                    p: r.get(4)?,
                })
            })
            .transpose()?;
        Ok(e)
    }

    /// Load all cached exchange rates.
    pub fn rates_load(&self) -> Result<Vec<RateRow>, CoreError> {
        let c = lock(&self.0.read);
        let mut st =
            c.prepare("SELECT currency, per_eur, rate_date, fetched_at FROM rate_cache")?;
        let rows = st.query_map([], |r| {
            Ok(RateRow {
                currency: r.get(0)?,
                per_eur: r.get(1)?,
                rate_date: r.get(2)?,
                fetched_at: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        if let Some(tx) = lock(&self.tx).take() {
            // Thread may already be gone.
            let _ = tx.send(WriteMsg::Shutdown);
        }
        if let Some(h) = lock(&self.handle).take() {
            // A panicked thread has nothing more to report.
            let _ = h.join();
        }
    }
}
