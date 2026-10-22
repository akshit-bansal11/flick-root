//! Feature trait, feature registry, settings schema and start/stop lifecycle. Every feature is a module behind this trait with a toggle; a stopped feature holds no tasks or clients.

pub mod capture;
pub mod ctx;
pub mod error;
pub mod explain;
pub mod feature;
pub mod frecency;
pub mod math;
pub mod media;
pub mod nlfind;
pub mod ports;
pub mod projects;
pub mod registry;
pub mod schema;
pub mod search;
pub mod syscmd;
pub mod web;

pub use ctx::{EventSink, FeatureCtx, TaskScope};
pub use error::FeatureError;
pub use explain::Explain;
pub use feature::Feature;
pub use nlfind::{NlFind, NlHost};
pub use registry::{FeatureState, FeatureView, Registry};
pub use schema::{Column, Field, FieldKind, SettingsSchema, validate};
pub use search::{command_search, find_search};

use std::sync::Arc;

/// All built-in features.
pub fn builtin_features() -> Vec<Arc<dyn Feature>> {
    builtin_features_with(media::Media::default(), capture::Capture::default())
}

/// All built-in features, with a media feature that has the optional Spotify/YouTube APIs and
/// a capture feature that can read the secret vault.
pub fn builtin_features_with(
    media: media::Media,
    capture: capture::Capture,
) -> Vec<Arc<dyn Feature>> {
    vec![
        Arc::new(frecency::Frecency::default()),
        Arc::new(math::Math),
        Arc::new(syscmd::SysCmd::default()),
        Arc::new(web::Web),
        Arc::new(projects::Projects::default()),
        Arc::new(ports::Ports::default()),
        Arc::new(explain::Explain),
        Arc::new(media),
        Arc::new(capture),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_are_nine_unique_and_enabled() {
        let f = builtin_features();
        let ids: std::collections::HashSet<_> = f.iter().map(|x| x.id()).collect();
        assert_eq!(f.len(), 9);
        assert_eq!(ids.len(), 9);
        assert!(f.iter().all(|x| x.default_enabled()));
        assert!(
            f.iter()
                .any(|x| x.id() == "ports" && x.version_tag() == "v0.0.1")
        );
    }
}
