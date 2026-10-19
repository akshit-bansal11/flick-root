//! Search links for players. T8.4 adds the optional API paths next to these.

use crate::web::QUERY_SET;
use percent_encoding::utf8_percent_encode;

fn enc(q: &str) -> String {
    utf8_percent_encode(q, QUERY_SET).to_string()
}

/// Spotify desktop search URI.
pub fn spotify(q: &str) -> String {
    format!("spotify:search:{}", enc(q))
}

/// YouTube results page for the default browser.
pub fn youtube(q: &str) -> String {
    format!("https://www.youtube.com/results?search_query={}", enc(q))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_spaces_amp_unicode() {
        assert_eq!(spotify("a b"), "spotify:search:a%20b");
        assert_eq!(spotify("a&b=c"), "spotify:search:a%26b%3Dc");
        assert_eq!(
            youtube("café"),
            "https://www.youtube.com/results?search_query=caf%C3%A9"
        );
    }
}
