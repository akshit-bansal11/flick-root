//! Feature trait, feature registry, settings schema and start/stop lifecycle. Every feature is a module behind this trait with a toggle; a stopped feature holds no tasks or clients.

pub mod ctx;
pub mod error;
pub mod feature;
pub mod frecency;
pub mod math;
pub mod nlfind;
pub mod projects;
pub mod registry;
pub mod schema;
pub mod search;
pub mod syscmd;
pub mod web;

pub use ctx::{EventSink, FeatureCtx, TaskScope};
pub use error::FeatureError;
pub use feature::Feature;
pub use nlfind::{NlFind, NlHost};
pub use registry::{FeatureState, FeatureView, Registry};
pub use schema::{Column, Field, FieldKind, SettingsSchema, validate};
pub use search::{command_search, find_search};

use std::sync::Arc;

/// All built-in features.
pub fn builtin_features() -> Vec<Arc<dyn Feature>> {
    vec![
        Arc::new(frecency::Frecency::default()),
        Arc::new(math::Math),
        Arc::new(syscmd::SysCmd::default()),
        Arc::new(web::Web),
        Arc::new(projects::Projects::default()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_are_five_unique_enabled_v001() {
        let f = builtin_features();
        let ids: std::collections::HashSet<_> = f.iter().map(|x| x.id()).collect();
        assert_eq!(f.len(), 5);
        assert_eq!(ids.len(), 5);
        assert!(
            f.iter()
                .all(|x| x.default_enabled() && x.version_tag() == "v0.0.1")
        );
    }
}
