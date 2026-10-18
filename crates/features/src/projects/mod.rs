//! F05 Project launcher (KICKOFF 1.3, ADR-009; FR-F05-01). Scan is off the keystroke path.
//! Keywords `code <name>` / `term <name>` and VS Code recents belong to T4.7; items carry
//! the folder in `payload.path` so those can reuse them.

mod markers;
mod scan;

use crate::ctx::{FeatureCtx, TaskScope};
use crate::error::FeatureError;
use crate::feature::Feature;
use crate::schema::{Field, FieldKind, SettingsSchema};
use arc_swap::ArcSwap;
use async_trait::async_trait;
use launcher_core::matcher::{FuzzyMatcher, MatchIndex};
use launcher_core::{Action, ActionKind, Input, Item, ItemId, ItemKind, Mode, Outcome};
use parking_lot::Mutex;
use scan::{Project, scan};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const MAX_RESULTS: usize = 8;
const DEFAULT_DEPTH: usize = 3;

/// Scanned projects with their fuzzy index.
struct Index {
    projects: Vec<Project>,
    names: MatchIndex,
}

type Slot = Arc<ArcSwap<Option<Index>>>;

/// Running state; dropped by `stop`.
struct Live {
    index: Slot,
    tasks: TaskScope,
}

/// Project launcher.
#[derive(Default)]
pub struct Projects {
    live: Mutex<Option<Live>>,
    matcher: Mutex<FuzzyMatcher>,
}

/// Default roots, then the user's `roots` option; only those that exist.
fn roots_from(opts: &toml::Table) -> Vec<PathBuf> {
    let mut roots = vec![PathBuf::from("D:\\dev")];
    if let Some(home) = std::env::var_os("USERPROFILE") {
        roots.push(PathBuf::from(home).join("source").join("repos"));
    }
    if let Some(list) = opts.get("roots").and_then(|v| v.as_array()) {
        roots.extend(list.iter().filter_map(|v| v.as_str()).map(PathBuf::from));
    }
    roots.retain(|r| r.is_dir());
    roots
}

fn depth_from(opts: &toml::Table) -> usize {
    opts.get("depth")
        .and_then(toml::Value::as_integer)
        .and_then(|d| usize::try_from(d).ok())
        .unwrap_or(DEFAULT_DEPTH)
}

fn build(projects: Vec<Project>) -> Index {
    let names: Vec<&str> = projects.iter().map(|p| p.name.as_str()).collect();
    Index {
        names: MatchIndex::new(&names),
        projects,
    }
}

impl Projects {
    /// Test hook: project count once the scan has finished.
    #[cfg(test)]
    fn count(&self) -> Option<usize> {
        let slot = self.live.lock().as_ref()?.index.clone();
        let guard = slot.load();
        guard.as_ref().as_ref().map(|i| i.projects.len())
    }
}

#[async_trait]
impl Feature for Projects {
    fn id(&self) -> &'static str {
        "projects"
    }
    fn title(&self) -> &'static str {
        "Project launcher"
    }
    fn version_tag(&self) -> &'static str {
        "v0.0.1"
    }
    fn description(&self) -> &'static str {
        "Finds dev folders and opens them in an editor or terminal."
    }
    fn default_enabled(&self) -> bool {
        true
    }
    fn modes(&self) -> &'static [Mode] {
        &[Mode::Command]
    }
    fn settings_schema(&self) -> SettingsSchema {
        SettingsSchema {
            fields: vec![
                Field {
                    key: "roots".into(),
                    label: "Extra dev folders".into(),
                    help: "Added to D:\\dev and %USERPROFILE%\\source\\repos when those exist."
                        .into(),
                    kind: FieldKind::StringList,
                    default: serde_json::json!([]),
                    restart: true,
                },
                Field {
                    key: "depth".into(),
                    label: "Scan depth".into(),
                    help: "How many folder levels below each root to look.".into(),
                    kind: FieldKind::Number {
                        min: 1.0,
                        max: 8.0,
                        step: 1.0,
                    },
                    default: serde_json::json!(DEFAULT_DEPTH),
                    restart: true,
                },
            ],
        }
    }
    fn start(&self, ctx: FeatureCtx) -> Result<(), FeatureError> {
        let opts = ctx.options("projects");
        let (roots, depth) = (roots_from(&opts), depth_from(&opts));
        let slot: Slot = Arc::new(ArcSwap::from_pointee(None));
        let (out, token) = (slot.clone(), ctx.tasks.token());
        ctx.tasks.spawn(async move {
            let t = token.clone();
            let found =
                tokio::task::spawn_blocking(move || scan(&roots, depth, &|| t.is_cancelled()))
                    .await;
            match found {
                Ok(p) if !token.is_cancelled() => out.store(Arc::new(Some(build(p)))),
                Ok(_) => {}
                Err(e) => tracing::warn!(error = %e, "project scan failed"),
            }
        });
        *self.live.lock() = Some(Live {
            index: slot,
            tasks: ctx.tasks,
        });
        Ok(())
    }
    fn stop(&self) {
        if let Some(l) = self.live.lock().take() {
            l.tasks.token().cancel();
            l.index.store(Arc::new(None));
        }
    }
    fn query(&self, input: &Input) -> Vec<Item> {
        let q = input.text.trim();
        let Some(slot) = self.live.lock().as_ref().map(|l| l.index.clone()) else {
            return Vec::new();
        };
        let guard = slot.load();
        let Some(idx) = guard.as_ref().as_ref() else {
            return Vec::new();
        };
        self.matcher
            .lock()
            .top_k(q, &idx.names, MAX_RESULTS)
            .into_iter()
            .map(|s| {
                let p = &idx.projects[s.idx];
                let path = p.path.to_string_lossy().into_owned();
                Item {
                    id: ItemId(format!("projects:{path}")),
                    kind: ItemKind::Project,
                    title: p.name.clone(),
                    subtitle: path.clone(),
                    icon: None,
                    // ponytail: squash nucleo's unbounded score into 0..1, tune if ranking needs it.
                    score: s.score as f32 / (s.score as f32 + 200.0),
                    source: "projects".into(),
                    actions: vec![ActionKind::Primary],
                    payload: serde_json::json!({ "path": path }),
                }
            })
            .collect()
    }
    async fn execute(&self, item: &Item, action: Action) -> Result<Outcome, FeatureError> {
        if action.kind != ActionKind::Primary {
            return Err(FeatureError::Other("unsupported action".into()));
        }
        let path = item
            .payload
            .get("path")
            .and_then(|p| p.as_str())
            .ok_or_else(|| FeatureError::Other("item has no path".into()))?
            .to_owned();
        tokio::task::spawn_blocking(move || index_apps::launch::open_path(Path::new(&path)))
            .await
            .map_err(|e| FeatureError::Other(format!("open task failed: {e}")))?
            .map_err(|e| FeatureError::Other(format!("could not open folder: {e}")))?;
        Ok(Outcome::HideLauncher)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ctx::EventSink;
    use launcher_core::Config;
    use launcher_core::store::Store;
    use providers::hub::ProviderHub;
    use std::fs;
    use std::time::Duration;

    struct Sink;
    impl EventSink for Sink {
        fn emit(&self, _: &str, _: serde_json::Value) {}
    }

    fn touch(root: &Path, rel: &str) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, "").unwrap();
    }
    fn names(p: &[Project]) -> Vec<String> {
        let mut n: Vec<_> = p.iter().map(|p| p.name.clone()).collect();
        n.sort();
        n
    }
    fn input(t: &str) -> Input {
        Input {
            raw: t.into(),
            text: t.into(),
            mode: Mode::Command,
            seq: 1,
        }
    }
    fn ctx(root: &Path) -> FeatureCtx {
        let mut cfg = Config::default();
        let mut t = toml::Table::new();
        t.insert(
            "roots".into(),
            toml::Value::Array(vec![root.to_string_lossy().into_owned().into()]),
        );
        cfg.features.insert("projects".into(), t);
        FeatureCtx {
            config: Arc::new(ArcSwap::from_pointee(cfg)),
            store: Arc::new(Store::open_in_memory().unwrap()),
            providers: Arc::new(ArcSwap::from_pointee(ProviderHub::new(vec![]))),
            http: reqwest::Client::new(),
            events: Arc::new(Sink),
            tasks: TaskScope::new(),
        }
    }
    fn fixture() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        for p in [
            "git-proj/.git/HEAD",
            "node-proj/package.json",
            "rust-proj/Cargo.toml",
            "py-proj/pyproject.toml",
            "go-proj/go.mod",
            "group/deep-proj/package.json",
            "a/b/c/too-deep/package.json",
            "node-proj/inner-proj/package.json",
            "node_modules/skipped/package.json",
            ".hidden/skipped2/package.json",
            "rust-proj/target/skipped3/Cargo.toml",
            "plain-folder/readme.txt",
        ] {
            touch(d.path(), p);
        }
        d
    }

    #[test]
    fn markers_depth_skips_and_nesting() {
        let d = fixture();
        let roots = vec![d.path().to_path_buf(), d.path().join("missing")];
        let found = scan(&roots, 3, &|| false);
        assert_eq!(
            names(&found),
            [
                "deep-proj",
                "git-proj",
                "go-proj",
                "node-proj",
                "py-proj",
                "rust-proj"
            ]
        );
        assert!(!names(&scan(&roots, 1, &|| false)).contains(&"deep-proj".to_string()));
        assert!(
            scan(&roots, 4, &|| false)
                .iter()
                .any(|p| p.name == "too-deep")
        );
        assert!(scan(&[d.path().join("nope")], 3, &|| false).is_empty());
    }

    #[tokio::test]
    async fn start_scans_queries_and_stop_clears() {
        let d = fixture();
        let c = ctx(d.path());
        let f = Projects::default();
        assert!(f.query(&input("rust")).is_empty());
        f.start(c.clone()).unwrap();
        for _ in 0..300 {
            if f.count().is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let items = f.query(&input("rust"));
        assert_eq!(items[0].title, "rust-proj");
        assert_eq!(items[0].kind, ItemKind::Project);
        assert!(
            items[0].payload["path"]
                .as_str()
                .unwrap()
                .ends_with("rust-proj")
        );
        assert!(f.query(&input("")).is_empty());
        f.stop();
        assert!(f.query(&input("rust")).is_empty());
        assert!(f.count().is_none());
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(c.tasks.active(), 0);
        f.stop();
    }
}
