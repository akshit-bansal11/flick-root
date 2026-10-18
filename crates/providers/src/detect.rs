//! Local model server detection. Read-only: never changes config, never enables anything.

use crate::http::plain_client;
use crate::http::read_capped;
use crate::presets::{Group, PRESETS};
use futures::future::join_all;
use serde_json::Value;
use std::time::Duration;

/// A local server that answered `GET /models`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detected {
    /// Preset id.
    pub preset_id: String,
    /// Base URL that answered.
    pub base_url: String,
    /// Model ids reported.
    pub models: Vec<String>,
}

/// Default probe timeout per server.
pub const PROBE_TIMEOUT: Duration = Duration::from_millis(200);

/// Probe the three local presets concurrently; unreachable ones are omitted.
pub async fn detect_local(timeout: Duration) -> Vec<Detected> {
    let targets: Vec<(String, String)> = PRESETS
        .iter()
        .filter(|p| p.group == Group::Local)
        .map(|p| (p.id.to_owned(), p.default_base_url.to_owned()))
        .collect();
    detect_targets(&targets, timeout).await
}

/// Probe explicit `(preset_id, base_url)` targets; injectable for tests.
pub async fn detect_targets(targets: &[(String, String)], timeout: Duration) -> Vec<Detected> {
    let Ok(client) = plain_client() else {
        return vec![];
    };
    join_all(targets.iter().map(|(id, base)| {
        let client = client.clone();
        async move {
            let url = format!("{}/models", base.trim_end_matches('/'));
            let probe = async {
                let resp = client.get(url).timeout(timeout).send().await.ok()?;
                if !resp.status().is_success() {
                    return None;
                }
                let body = read_capped(resp, 1 << 20).await.ok()?;
                serde_json::from_slice::<Value>(&body).ok()
            };
            // The reqwest timeout covers the body too; this guards the whole probe.
            let v = tokio::time::timeout(timeout, probe).await.ok()??;
            let models = v["data"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|m| m["id"].as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default();
            Some(Detected {
                preset_id: id.clone(),
                base_url: base.clone(),
                models,
            })
        }
    }))
    .await
    .into_iter()
    .flatten()
    .collect()
}
