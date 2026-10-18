//! Incremental Server-Sent Events parser.

use bytes::Bytes;
use futures::{Stream, StreamExt};

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
    pub fn push(&mut self, chunk: &[u8]) -> Vec<SseEvent> {
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
                    self.end_line(&line, &mut out);
                }
                _ => self.line.push(b),
            }
        }
        out
    }

    fn end_line(&mut self, raw: &[u8], out: &mut Vec<SseEvent>) {
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
            return;
        }
        if raw[0] == b':' {
            return;
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
                    d.push('\n');
                    d.push_str(value);
                }
                None => self.data = Some(value.to_owned()),
            },
            _ => {}
        }
    }
}

/// Adapt a byte stream into an event stream.
pub fn events<S, E>(bytes: S) -> impl Stream<Item = Result<SseEvent, E>>
where
    S: Stream<Item = Result<Bytes, E>>,
{
    let mut p = SseParser::default();
    bytes.flat_map(move |r| {
        futures::stream::iter(match r {
            Ok(b) => p.push(&b).into_iter().map(Ok).collect(),
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
        assert_eq!(SseParser::default().push(SAMPLE), expect());
    }

    #[test]
    fn every_split_point() {
        for i in 0..=SAMPLE.len() {
            let mut p = SseParser::default();
            let mut got = p.push(&SAMPLE[..i]);
            got.extend(p.push(&SAMPLE[i..]));
            assert_eq!(got, expect(), "split at {i}");
        }
    }

    #[test]
    fn byte_at_a_time() {
        let mut p = SseParser::default();
        let got: Vec<_> = SAMPLE.iter().flat_map(|b| p.push(&[*b])).collect();
        assert_eq!(got, expect());
    }
}
