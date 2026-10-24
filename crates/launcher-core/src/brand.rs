//! Brand constants. The brand name lives only in this file on the Rust side.

/// Lowercase application name, used for config paths and identifiers.
pub const APP_NAME: &str = "flick";

/// Display name shown to the user in window titles and UI text.
pub const APP_DISPLAY_NAME: &str = "Flick";

/// GitHub `owner/name` of the repository releases are published from.
pub const REPO: &str = "akshit-bansal11/flick";

/// Reverse-DNS bundle identifier for the application.
pub const BUNDLE_ID: &str = "dev.akshitbansal.flick";

/// Id of the built-in dark default theme. Persisted in config, so it never changes.
pub const DEFAULT_THEME_ID: &str = "flick-default";

/// Id of the built-in light default theme. Persisted in config, so it never changes.
pub const DEFAULT_THEME_LIGHT_ID: &str = "flick-default-light";
