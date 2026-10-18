//! Logging with redaction (ADR-011 rule 7): every record and panic message passes
//! through the global redactor before it reaches stderr or the log file.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::panic::{Location, PanicHookInfo};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use launcher_core::redact::global;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::MakeWriter;

/// Log file size at which `app.log` rotates to `app.log.1`.
const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;

/// Buffers one formatted record, redacts it, then writes it to `out` on flush or drop.
pub struct RedactingWriter<W: Write> {
    buf: Vec<u8>,
    out: W,
}

impl<W: Write> RedactingWriter<W> {
    /// Wraps `out`.
    pub fn new(out: W) -> Self {
        Self {
            buf: Vec::new(),
            out,
        }
    }

    fn emit(&mut self) -> io::Result<()> {
        if self.buf.is_empty() {
            return Ok(());
        }
        let text = String::from_utf8_lossy(&self.buf);
        let res = self.out.write_all(global().redact(&text).as_bytes());
        self.buf.clear();
        res
    }
}

impl<W: Write> Write for RedactingWriter<W> {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.buf.extend_from_slice(data);
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.emit()?;
        self.out.flush()
    }
}

impl<W: Write> Drop for RedactingWriter<W> {
    fn drop(&mut self) {
        // A failed log write has nowhere to be reported.
        let _ = self.emit();
    }
}

/// `MakeWriter` producing a [`RedactingWriter`] over a clone of `W` per record.
#[derive(Clone)]
pub struct RedactingMake<W>(pub W);

impl<'a, W: Write + Clone + 'a> MakeWriter<'a> for RedactingMake<W> {
    type Writer = RedactingWriter<W>;

    fn make_writer(&'a self) -> Self::Writer {
        RedactingWriter::new(self.0.clone())
    }
}

/// Size-capped log file.
struct RotatingFile {
    path: PathBuf,
    file: File,
    size: u64,
}

impl RotatingFile {
    fn open(path: PathBuf) -> io::Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let size = file.metadata()?.len();
        Ok(Self { path, file, size })
    }

    fn rotate(&mut self) -> io::Result<()> {
        let mut old = self.path.clone().into_os_string();
        old.push(".1");
        fs::rename(&self.path, PathBuf::from(old))?;
        self.file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        self.size = 0;
        Ok(())
    }
}

/// Shared sink: stderr in debug builds, plus the file when it could be opened.
struct Sink {
    stderr: bool,
    file: Option<RotatingFile>,
}

/// Cloneable handle to the shared sink.
#[derive(Clone)]
struct SinkHandle(Arc<Mutex<Sink>>);

impl Write for SinkHandle {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        let mut sink = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if sink.stderr {
            let _ = io::stderr().write_all(data);
        }
        if let Some(f) = sink.file.as_mut() {
            if f.size > MAX_LOG_BYTES {
                let _ = f.rotate();
            }
            f.file.write_all(data)?;
            f.size += data.len() as u64;
        }
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut sink = self.0.lock().unwrap_or_else(|e| e.into_inner());
        match sink.file.as_mut() {
            Some(f) => f.file.flush(),
            None => Ok(()),
        }
    }
}

/// Keeps the log sink alive; flushes on drop.
pub struct LogGuard(SinkHandle);

impl Drop for LogGuard {
    fn drop(&mut self) {
        let _ = self.0.flush();
    }
}

/// Installs the redacting tracing subscriber (filter from `RUST_LOG`, default `info`).
/// Logs to stderr in debug builds and to `log_dir/app.log` when given and openable.
/// Returns `Ok` without changes if a subscriber is already installed.
pub fn init(log_dir: Option<PathBuf>) -> anyhow::Result<LogGuard> {
    let mut file_err = None;
    let file = log_dir.and_then(|dir| {
        fs::create_dir_all(&dir)
            .and_then(|()| RotatingFile::open(dir.join("app.log")))
            .map_err(|e| file_err = Some(e))
            .ok()
    });
    let sink = SinkHandle(Arc::new(Mutex::new(Sink {
        // Without a file, stderr is the only target.
        stderr: cfg!(debug_assertions) || file.is_none(),
        file,
    })));
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let installed = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(false)
        .with_writer(RedactingMake(sink.clone()))
        .try_init()
        .is_ok();
    if installed && let Some(e) = file_err {
        tracing::warn!(error = %e, "log file unavailable; logging to stderr only");
    }
    Ok(LogGuard(sink))
}

/// Formats a panic message and location, redacted. Never includes the raw payload.
fn panic_text(info: &PanicHookInfo<'_>) -> String {
    let payload = info.payload();
    let msg = payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("non-string panic payload");
    format_panic(msg, info.location())
}

fn format_panic(msg: &str, loc: Option<&Location<'_>>) -> String {
    let raw = match loc {
        Some(l) => format!("panic at {}:{}: {msg}", l.file(), l.line()),
        None => format!("panic: {msg}"),
    };
    global().redact(&raw).into_owned()
}

/// Replaces the panic hook: the message is redacted, then logged and written to stderr.
pub fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let text = panic_text(info);
        tracing::error!("{text}");
        eprintln!("{text}");
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Default)]
    struct Buf(Arc<Mutex<Vec<u8>>>);

    impl Write for Buf {
        fn write(&mut self, d: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap_or_else(|e| e.into_inner()).extend(d);
            Ok(d.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    const SECRET: &str = "hunter2-fake-secret-value";

    fn contents(b: &Buf) -> String {
        String::from_utf8_lossy(&b.0.lock().unwrap_or_else(|e| e.into_inner())).into_owned()
    }

    #[test]
    fn writer_masks_registered_secret_and_key_patterns() {
        global().register(SECRET.as_bytes());
        let buf = Buf::default();
        let make = RedactingMake(buf.clone());
        {
            let mut w = make.make_writer();
            write!(w, "token={SECRET} ").unwrap();
            write!(w, "key=sk-ant-api03-abcdefghijklmnopqrstuvwxyz0123456789").unwrap();
        }
        let out = contents(&buf);
        assert!(out.contains("[REDACTED]"));
        assert!(!out.contains(SECRET));
        assert!(!out.contains("sk-ant-api03-abcdef"));
    }

    #[test]
    fn file_sink_receives_redacted_event() {
        global().register(SECRET.as_bytes());
        let dir = std::env::temp_dir().join(format!("lg-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("app.log");
        let sink = SinkHandle(Arc::new(Mutex::new(Sink {
            stderr: false,
            file: Some(RotatingFile::open(path.clone()).unwrap()),
        })));
        let sub = tracing_subscriber::fmt()
            .with_ansi(false)
            .with_writer(RedactingMake(sink))
            .finish();
        tracing::subscriber::with_default(sub, || {
            tracing::info!("ready {SECRET}");
        });
        let text = fs::read_to_string(&path).unwrap();
        let _ = fs::remove_dir_all(&dir);
        assert!(text.contains("ready"), "got {text:?}");
        assert!(!text.contains(SECRET));
    }

    #[test]
    fn panic_text_is_redacted() {
        global().register(SECRET.as_bytes());
        let out = format_panic(&format!("boom {SECRET}"), None);
        assert!(out.contains("[REDACTED]"));
        assert!(!out.contains(SECRET));
    }
}
