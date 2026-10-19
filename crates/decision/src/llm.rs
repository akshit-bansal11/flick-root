//! LLM picker: asks the user's Query provider to choose a candidate (ADR-005).
//! Model output is untrusted; only a validated index and probability leave this module.

use crate::systemone::DecisionEngine;
use crate::{ChoiceAnswer, ChoiceQuestion, EngineError, ProbeResult};
use async_trait::async_trait;
use futures::StreamExt;
use providers::{Delta, Effort, Message, Provider, Request, Role};
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Pick {
    index: u64,
    p: f64,
}

/// Strictly parse the model's reply: optional ```json fence, then exactly `{"index","p"}`.
fn parse_reply(raw: &str, n: usize) -> Result<(usize, f32), EngineError> {
    let bad = |m: &str| EngineError::BadResponse(m.to_owned());
    let mut s = raw.trim();
    if let Some(r) = s.strip_prefix("```json").or_else(|| s.strip_prefix("```")) {
        s = r
            .strip_suffix("```")
            .ok_or_else(|| bad("unclosed fence"))?
            .trim();
    }
    let pick: Pick = serde_json::from_str(s).map_err(|_| bad("not strict json"))?;
    let index = usize::try_from(pick.index).map_err(|_| bad("index out of range"))?;
    if index >= n {
        return Err(bad("index out of range"));
    }
    if !(0.0..=1.0).contains(&pick.p) {
        return Err(bad("p out of range"));
    }
    Ok((index, pick.p as f32))
}

fn prompt(q: &ChoiceQuestion) -> String {
    let mut s = String::from(
        "Choose the file that best matches the phrase. Reply with strict JSON only: {\"index\": <int>, \"p\": <0..1>}.\n",
    );
    s.push_str(&format!("Phrase: {}\nCandidates:\n", q.phrase));
    for (i, c) in q.candidates.iter().enumerate() {
        s.push_str(&format!("{i}. {} (in {})\n", c.name, c.parent));
    }
    s
}

/// Decision engine backed by a chat provider.
pub struct LlmPicker {
    provider: Arc<dyn Provider>,
    timeout: Duration,
}

impl LlmPicker {
    /// Wrap the user's Query provider (its configured model is used).
    pub fn new(provider: Arc<dyn Provider>, timeout: Duration) -> Self {
        Self { provider, timeout }
    }

    async fn collect(&self, q: &ChoiceQuestion) -> Result<String, EngineError> {
        let req = Request::new(
            vec![Message {
                role: Role::User,
                content: prompt(q),
            }],
            Effort::Fast,
        );
        let mut stream = self.provider.stream(req, CancellationToken::new());
        let mut out = String::new();
        while let Some(d) = stream.next().await {
            match d.map_err(|e| EngineError::Unreachable(e.to_string()))? {
                Delta::Text(t) if out.len() + t.len() <= 4096 => out.push_str(&t),
                Delta::Text(_) => return Err(EngineError::BadResponse("reply too long".into())),
                Delta::Thinking => {}
                Delta::Done(_) => break,
            }
        }
        Ok(out)
    }
}

#[async_trait]
impl DecisionEngine for LlmPicker {
    fn id(&self) -> &'static str {
        "llm"
    }

    async fn choose(&self, q: &ChoiceQuestion) -> Result<ChoiceAnswer, EngineError> {
        let text = tokio::time::timeout(self.timeout, self.collect(q))
            .await
            .map_err(|_| EngineError::Timeout)??;
        let (index, p) = parse_reply(&text, q.candidates.len())?;
        Ok(ChoiceAnswer {
            index,
            p,
            engine: "llm",
        })
    }

    async fn probe(&self) -> ProbeResult {
        let r = self.provider.probe().await;
        ProbeResult {
            id: "llm".into(),
            reachable: r.reachable,
            detail: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Candidate;
    use futures::stream::{self, BoxStream};
    use providers::{ProviderConfig, ProviderError, Wire};

    #[test]
    fn strict_parser_table() {
        let ok = |s| parse_reply(s, 3).is_ok();
        assert!(ok(r#"{"index": 1, "p": 0.5}"#));
        assert!(ok("```json\n{\"index\":2,\"p\":1}\n```"));
        assert!(!ok("Sure! {\"index\":1,\"p\":0.5}"));
        assert!(!ok(r#"{"index":1,"p":0.5,"why":"x"}"#));
        assert!(!ok(r#"{"index":3,"p":0.5}"#));
        assert!(!ok(r#"{"index":-1,"p":0.5}"#));
        assert!(!ok(r#"{"index":1.5,"p":0.5}"#));
        assert!(!ok(r#"{"index":1,"p":1.5}"#));
        assert!(!ok(""));
    }

    struct Fake(ProviderConfig, &'static str);
    #[async_trait]
    impl Provider for Fake {
        fn config(&self) -> &ProviderConfig {
            &self.0
        }
        fn stream(
            &self,
            _: Request,
            _: CancellationToken,
        ) -> BoxStream<'static, Result<Delta, ProviderError>> {
            let t = self.1.to_owned();
            Box::pin(stream::iter(vec![
                Ok(Delta::Text(t)),
                Ok(Delta::Done(Default::default())),
            ]))
        }
        async fn probe(&self) -> providers::ProbeResult {
            unreachable!()
        }
    }

    #[tokio::test]
    async fn end_to_end_fake_provider() {
        let cfg = ProviderConfig {
            id: "f".into(),
            wire: Wire::OpenAi,
            base_url: "https://x.test/v1".parse().unwrap(),
            model: "m".into(),
            key_host: None,
            supports_effort: false,
        };
        let c = |n: &str| Candidate {
            name: n.into(),
            parent: "D:\\".into(),
        };
        let q = ChoiceQuestion {
            phrase: "cv".into(),
            candidates: vec![c("a"), c("b")],
        };
        let good = LlmPicker::new(
            Arc::new(Fake(cfg.clone(), r#"{"index":1,"p":0.7}"#)),
            Duration::from_secs(1),
        );
        let a = good.choose(&q).await.unwrap();
        assert_eq!((a.index, a.engine), (1, "llm"));
        let bad = LlmPicker::new(Arc::new(Fake(cfg, "pick b")), Duration::from_secs(1));
        assert!(matches!(
            bad.choose(&q).await,
            Err(EngineError::BadResponse(_))
        ));
    }
}
