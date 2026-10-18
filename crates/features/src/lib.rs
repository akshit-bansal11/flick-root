//! Feature trait, feature registry, settings schema and start/stop lifecycle. Every feature is a module behind this trait with a toggle; a stopped feature holds no tasks or clients.

pub mod ctx;
pub mod error;
pub mod feature;
pub mod registry;
pub mod schema;
pub mod search;

pub use ctx::{EventSink, FeatureCtx, TaskScope};
pub use error::FeatureError;
pub use feature::Feature;
pub use registry::{FeatureState, FeatureView, Registry};
pub use schema::{Column, Field, FieldKind, SettingsSchema, validate};
pub use search::{command_search, find_search};

use std::sync::Arc;

/// All built-in features. Empty until the feature crates land.
pub fn builtin_features() -> Vec<Arc<dyn Feature>> {
    Vec::new()
}
