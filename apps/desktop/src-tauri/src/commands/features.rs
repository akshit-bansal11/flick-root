//! Feature, mode and system-command commands (BSD section 4).
use crate::error::CommandError;
use crate::state::{self, AppState};
use features::{FeatureError, FeatureState, FeatureView};
use launcher_core::{Action, ActionKind, Outcome};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

/// Features page payload.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureList {
    /// One view per feature.
    pub features: Vec<FeatureView>,
    /// Features currently running.
    pub on_count: usize,
}

fn map_err(e: FeatureError) -> CommandError {
    let code = match e {
        FeatureError::Unknown(_) => "unknown_feature",
        FeatureError::Options(_) => "invalid_options",
        FeatureError::Start { .. } => "start_failed",
        FeatureError::ConfirmExpired => "confirm_expired",
        FeatureError::SecretBlocked(_) => "secret_blocked",
        FeatureError::Other(_) => "internal",
    };
    CommandError::new(code, e.to_string())
}

/// Converts UI JSON into a TOML table (or any `T`); nulls and non-objects are rejected.
/// Generic so this crate needs no direct `toml` dependency: the table type is inferred.
fn json_to_table<T: DeserializeOwned>(v: serde_json::Value) -> Result<T, CommandError> {
    if !v.is_object() {
        return Err(CommandError::new(
            "invalid_options",
            "options must be an object",
        ));
    }
    T::deserialize(v).map_err(|e| CommandError::new("invalid_options", e.to_string()))
}

async fn view(state: &AppState, id: &str) -> Result<FeatureView, CommandError> {
    let cfg = state.config.load_full();
    state
        .registry
        .views(&cfg)
        .await
        .into_iter()
        .find(|v| v.id == id)
        .ok_or_else(|| CommandError::new("unknown_feature", format!("no feature {id}")))
}

fn changed(app: &AppHandle, v: &FeatureView) {
    let payload = serde_json::json!({ "id": v.id, "enabled": v.enabled, "status": v.status });
    if let Err(err) = app.emit("features://changed", payload) {
        tracing::warn!(%err, "emit failed");
    }
}

/// Writes only `[features.<id>]` so concurrent provider/general edits are not clobbered.
fn persist(state: &AppState, id: &str, cfg: launcher_core::Config) -> Result<(), CommandError> {
    let table = cfg.features.get(id).cloned().unwrap_or_default();
    state.update_config(|c| {
        c.features.insert(id.to_string(), table);
        Ok(())
    })
}

/// Lists every feature with its schema and options.
#[tauri::command]
pub async fn list_features(state: State<'_, Arc<AppState>>) -> Result<FeatureList, CommandError> {
    let cfg = state.config.load_full();
    let features = state.registry.views(&cfg).await;
    let (on_count, _) = state.registry.on_count().await;
    Ok(FeatureList { features, on_count })
}

/// Turns a feature on or off and persists the choice.
#[tauri::command]
pub async fn set_feature_enabled(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
    enabled: bool,
) -> Result<FeatureView, CommandError> {
    let cfg = state
        .registry
        .set_enabled(&id, enabled, &state.ctx)
        .await
        .map_err(map_err)?;
    persist(&state, &id, cfg)?;
    let v = view(&state, &id).await?;
    changed(&app, &v);
    if enabled && v.status == FeatureState::Failed {
        return Err(CommandError::new(
            "start_failed",
            v.error.unwrap_or_else(|| "feature failed to start".into()),
        ));
    }
    Ok(v)
}

/// Validates and saves a feature's options.
#[tauri::command]
pub async fn set_feature_options(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
    options: serde_json::Value,
) -> Result<FeatureView, CommandError> {
    let table = json_to_table(options)?;
    let cfg = state
        .registry
        .set_options(&id, table, &state.ctx)
        .await
        .map_err(map_err)?;
    persist(&state, &id, cfg)?;
    let v = view(&state, &id).await?;
    changed(&app, &v);
    Ok(v)
}

/// Pure rule: which modes may be toggled.
fn check_mode(mode: &str) -> Result<(), CommandError> {
    match mode {
        "command" => Err(CommandError::new(
            "locked",
            "Command mode cannot be turned off",
        )),
        "find" | "query" => Ok(()),
        _ => Err(CommandError::new(
            "invalid_mode",
            format!("unknown mode {mode}"),
        )),
    }
}

/// Turns Find or Query mode on or off. Command mode is locked.
#[tauri::command]
pub async fn set_mode_enabled(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    mode: String,
    enabled: bool,
) -> Result<(), CommandError> {
    check_mode(&mode)?;
    let is_find = mode == "find";
    let was = state.config.load().modes.find.enabled;
    state.update_config(|c| {
        if is_find {
            c.modes.find.enabled = enabled;
        } else {
            c.modes.query.enabled = enabled;
        }
        Ok(())
    })?;
    if is_find && enabled && !was {
        let s = Arc::clone(&state);
        tauri::async_runtime::spawn(async move { state::start_find(&s).await });
    } else if is_find && !enabled {
        state::stop_find(&state);
    }
    if let Err(err) = app.emit(
        "settings://changed",
        serde_json::json!({ "section": "modes" }),
    ) {
        tracing::warn!(%err, "emit failed");
    }
    Ok(())
}

/// Runs a system command result; the feature enforces the confirmation guard.
#[tauri::command]
pub async fn run_system_command(
    state: State<'_, Arc<AppState>>,
    command_id: String,
    confirm_token: Option<String>,
) -> Result<Outcome, CommandError> {
    let it = state
        .last_results
        .lock()
        .get(&command_id)
        .cloned()
        .ok_or_else(|| CommandError::new("not_found", "that result is no longer available"))?;
    let f = state
        .registry
        .get(&it.source)
        .await
        .ok_or_else(|| CommandError::new("unknown_feature", "no feature for that result"))?;
    let action = Action {
        kind: ActionKind::Primary,
        confirm_token,
    };
    f.execute(&it, action).await.map_err(map_err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn json_object_converts() {
        let m: BTreeMap<String, serde_json::Value> =
            json_to_table(serde_json::json!({ "a": 1, "b": "x" })).unwrap();
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn non_object_rejected() {
        let r: Result<BTreeMap<String, serde_json::Value>, _> =
            json_to_table(serde_json::json!([1]));
        assert_eq!(r.unwrap_err().code, "invalid_options");
    }

    #[test]
    fn command_mode_locked() {
        assert_eq!(check_mode("command").unwrap_err().code, "locked");
        assert!(check_mode("find").is_ok());
        assert!(check_mode("query").is_ok());
        assert_eq!(check_mode("zzz").unwrap_err().code, "invalid_mode");
    }
}
