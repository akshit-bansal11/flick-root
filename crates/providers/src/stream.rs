//! The one SSE driver and the `/models` probe shared by both wire clients.

use crate::http::{check_status, net_err, read_capped};
use crate::sse::{SseEvent, SseParser};
use crate::{Delta, ProbeResult, ProviderError};
use futures::stream::{BoxStream, StreamExt};
use serde_json::Value;
use std::collections::VecDeque;
use std::future::Future;
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

/// Cap for the `/models` probe body.
const MAX_PROBE_BYTES: usize = 1 << 20;

/// Per-wire event handling state.
pub(crate) trait WireState: Send + 'static {
    /// Error text when the body ends before the wire's terminal event.
    const ENDED: &'static str;
    /// Turn one event into zero or more deltas. `Delta::Done` and `Err` end the stream.
    fn handle(&mut self, ev: SseEvent) -> Vec<Result<Delta, ProviderError>>;
}

/// Send `rb` (cancellable) and require a 2xx status.
pub(crate) async fn open(
    rb: reqwest::RequestBuilder,
    cancel: &CancellationToken,
) -> Result<reqwest::Response, ProviderError> {
    let resp = tokio::select! {
        () = cancel.cancelled() => return Err(ProviderError::Cancelled),
        r = rb.send() => r.map_err(net_err)?,
    };
    check_status(&resp)?;
    Ok(resp)
}

struct Driver<W> {
    bytes: BoxStream<'static, Result<bytes::Bytes, reqwest::Error>>,
    parser: SseParser,
    wire: W,
    out: VecDeque<Result<Delta, ProviderError>>,
    cancel: CancellationToken,
    done: bool,
}

impl<W: WireState> Driver<W> {
    async fn next(&mut self) -> Option<Result<Delta, ProviderError>> {
        loop {
            if self.cancel.is_cancelled() {
                if self.done {
                    return None;
                }
                self.done = true;
                return Some(Err(ProviderError::Cancelled));
            }
            if let Some(item) = self.out.pop_front() {
                if matches!(item, Ok(Delta::Done(_)) | Err(_)) {
                    self.done = true;
                    self.out.clear();
                }
                return Some(item);
            }
            if self.done {
                return None;
            }
            let chunk = tokio::select! {
                () = self.cancel.cancelled() => continue,
                c = self.bytes.next() => c,
            };
            match chunk {
                Some(Ok(b)) => match self.parser.push(&b) {
                    Ok(evs) => {
                        for ev in evs {
                            self.out.extend(self.wire.handle(ev));
                        }
                    }
                    Err(e) => self.out.push_back(Err(e)),
                },
                Some(Err(e)) => self.out.push_back(Err(net_err(e))),
                None => self.out.push_back(Err(ProviderError::Decode(W::ENDED))),
            }
        }
    }
}

/// Decode an SSE response into deltas with cancellation and the SSE caps.
pub(crate) fn drive<W: WireState>(
    resp: reqwest::Response,
    wire: W,
    cancel: CancellationToken,
) -> BoxStream<'static, Result<Delta, ProviderError>> {
    let d = Driver {
        bytes: resp.bytes_stream().boxed(),
        parser: SseParser::default(),
        wire,
        out: VecDeque::new(),
        cancel,
        done: false,
    };
    futures::stream::unfold(d, |mut d| async move {
        let item = d.next().await?;
        Some((item, d))
    })
    .boxed()
}

/// Run `req` as a stream: open the connection, then drive it with `wire`.
pub(crate) fn start<W: WireState>(
    rb: Result<reqwest::RequestBuilder, ProviderError>,
    wire: W,
    cancel: CancellationToken,
) -> BoxStream<'static, Result<Delta, ProviderError>> {
    futures::stream::once(async move {
        let resp = open(rb?, &cancel).await?;
        Ok(drive(resp, wire, cancel))
    })
    .flat_map(|r| match r {
        Err(e) => futures::stream::once(async move { Err(e) }).boxed(),
        Ok(s) => s,
    })
    .boxed()
}

/// GET a models list (`{"data":[{"id":..}]}`) with a capped body.
pub(crate) async fn list_models(rb: reqwest::RequestBuilder) -> Result<Vec<String>, ProviderError> {
    let resp = rb.send().await.map_err(net_err)?;
    check_status(&resp)?;
    let body = read_capped(resp, MAX_PROBE_BYTES).await?;
    let v: Value =
        serde_json::from_slice(&body).map_err(|_| ProviderError::Decode("models json"))?;
    Ok(v["data"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|m| m["id"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default())
}

/// Wrap a models-list call into a [`ProbeResult`] with a deadline.
pub(crate) async fn probe(
    id: &str,
    timeout: Duration,
    models: impl Future<Output = Result<Vec<String>, ProviderError>>,
) -> ProbeResult {
    let start = Instant::now();
    let (reachable, models, error) = match tokio::time::timeout(timeout, models).await {
        Ok(Ok(m)) => (true, m, None),
        Ok(Err(e)) => (false, vec![], Some(e.to_string())),
        Err(_) => (false, vec![], Some("timed out".to_owned())),
    };
    ProbeResult {
        id: id.to_owned(),
        reachable,
        models,
        ms: u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX),
        error,
    }
}
