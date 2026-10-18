//! Incremental Server-Sent Events parser.

use crate::ProviderError;
use bytes::Bytes;
use futures::{Stream, StreamExt};

/// Longest accepted SSE line.
pub const MAX_LINE_BYTES: usize = 1 << 20;
/// Largest accepted accumulated event `data`.
pub const MAX_EVENT_BYTES: usize = 1 << 20;

/// One dispatched SSE event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    /// `event:` field, if any.
    pub event: Option<String>,
    /// `data:` lines joined with a newline.
    pub data: String,
}

/// Byte-level parser; feed chunks with [`SseParser::push`].
#[derive(Debug, Default)]
pub struct SseParser {
    line: Vec<u8>,
    skip_lf: bool,
    started: bool,
    event: Option<String>,
    data: Option<String>,
}

impl SseParser {
    /// Feed bytes (any split), returning completed events.
    ///
    /// Errors with `Decode` when a line or an event exceeds its cap; the
    /// parser must not be reused after an error.
    pub fn push(&mut self, chunk: &[u8]) -> Result<Vec<SseEvent>, ProviderError> {
        let mut out = Vec::new();
        for &b in chunk {
            let lf = b == b'\n';
            if std::mem::take(&mut self.skip_lf) && lf {
                continue;
            }
            match b {
                b'\r' | b'\n' => {
                    self.skip_lf = b == b'\r';
                    let line = std::mem::take(&mut self.line);
                    self.end_line(&line, &mut out)?;
                }
                _ => {
                    if self.line.len() >= MAX_LINE_BYTES {
                        return Err(ProviderError::Decode("sse line too long"));
                    }
                    self.line.push(b);
                }
            }
        }
        Ok(out)
    }

    fn end_line(&mut self, raw: &[u8], out: &mut Vec<SseEvent>) -> Result<(), ProviderError> {
        let mut raw = raw;
        if !self.started {
            self.started = true;
            raw = raw.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(raw);
        }
        if raw.is_empty() {
            if let Some(data) = self.data.take() {
                out.push(SseEvent {
                    event: self.event.take(),
                    data,
                });
            }
            self.event = None;
            return Ok(());
        }
        if raw[0] == b':' {
            return Ok(());
        }
        let line = String::from_utf8_lossy(raw);
        let (field, value) = match line.split_once(':') {
            Some((f, v)) => (f, v.strip_prefix(' ').unwrap_or(v)),
            None => (&*line, ""),
        };
        match field {
            "event" => self.event = Some(value.to_owned()),
            "data" => match &mut self.data {
                Some(d) => {
                    if d.len() + 1 + value.len() > MAX_EVENT_BYTES {
                        return Err(ProviderError::Decode("sse event too large"));
                    }
                    d.push('\n');
                    d.push_str(value);
                }
                None => self.data = Some(value.to_owned()),
            },
            _ => {}
        }
        Ok(())
    }
}

/// Adapt a byte stream into an event stream.
pub fn events<S, E>(bytes: S) -> impl Stream<Item = Result<SseEvent, E>>
where
    S: Stream<Item = Result<Bytes, E>>,
    E: From<ProviderError>,
{
    let mut p = SseParser::default();
    bytes.flat_map(move |r| {
        futures::stream::iter(match r {
            Ok(b) => match p.push(&b) {
                Ok(evs) => evs.into_iter().map(Ok).collect(),
                Err(e) => vec![Err(e.into())],
            },
            Err(e) => vec![Err(e)],
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &[u8] = b"\xEF\xBB\xBF: hi\r\nevent: x\r\ndata: a\r\ndata:b\r\n\r\ndata: c\n\ndata:d\r\r:c\nevent:y\ndata\n\n";

    fn expect() -> Vec<SseEvent> {
        let e = |event: Option<&str>, data: &str| SseEvent {
            event: event.map(Into::into),
            data: data.into(),
        };
        vec![
            e(Some("x"), "a\nb"),
            e(None, "c"),
            e(None, "d"),
            e(Some("y"), ""),
        ]
    }

    #[test]
    fn whole() {
        assert_eq!(SseParser::default().push(SAMPLE).unwrap(), expect());
    }

    #[test]
    fn every_split_point() {
        for i in 0..=SAMPLE.len() {
            let mut p = SseParser::default();
            let mut got = p.push(&SAMPLE[..i]).unwrap();
            got.extend(p.push(&SAMPLE[i..]).unwrap());
            assert_eq!(got, expect(), "split at {i}");
        }
    }

    #[test]
    fn byte_at_a_time() {
        let mut p = SseParser::default();
        let got: Vec<_> = SAMPLE.iter().flat_map(|b| p.push(&[*b]).unwrap()).collect();
        assert_eq!(got, expect());
    }

    #[test]
    fn line_cap() {
        let mut p = SseParser::default();
        assert!(p.push(&vec![b'a'; MAX_LINE_BYTES]).is_ok());
        assert!(matches!(
            p.push(b"a"),
            Err(ProviderError::Decode("sse line too long"))
        ));
    }

    #[test]
    fn event_cap() {
        let mut p = SseParser::default();
        let line = format!(
            "data: {}
",
            "a".repeat(1000)
        );
        let mut res = Ok(vec![]);
        for _ in 0..2000 {
            res = p.push(line.as_bytes());
            if res.is_err() {
                break;
            }
        }
        assert!(matches!(
            res,
            Err(ProviderError::Decode("sse event too large"))
        ));
    }
}
