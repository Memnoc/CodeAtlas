//! Helpers shared by the integration tests that enforce ADR-0006's egress
//! guarantee.
//!
//! The allowlist below is policy, not convenience, so it lives in one place:
//! two copies of a security rule drifting apart is how a check quietly stops
//! agreeing with the one beside it.

// Each integration-test binary compiles its own copy of this module, so a
// helper only one of them needs looks dead to the others.
#![allow(dead_code)]

/// Every `http(s)` URL in `text`, delimited the way a URL ends in source or
/// markup.
pub fn urls_in(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for scheme in ["http://", "https://"] {
        for (pos, _) in text.match_indices(scheme) {
            found.push(
                text[pos..]
                    .chars()
                    .take_while(|c| !c.is_whitespace() && !"\"'`\\)<>".contains(*c))
                    .collect(),
            );
        }
    }
    found
}

/// URLs that are string literals by construction, never requests — the same
/// allowlist the dashboard's zero-egress test documents: XML namespace
/// identifiers, React's minified-error text, and React Flow's doc links
/// including its attribution `<a href>` (kept deliberately: a plain anchor
/// performs no request until the reader chooses to click it).
pub fn is_inert(url: &str) -> bool {
    url.starts_with("http://www.w3.org/")
        || url.starts_with("https://www.w3.org/")
        || url.starts_with("https://react.dev/errors/")
        || url.starts_with("https://reactflow.dev")
        || (url.starts_with("https://${") && url.contains("flow.dev"))
}

/// The URLs in `text` that would be real egress — everything [`urls_in`]
/// finds that [`is_inert`] does not excuse.
pub fn external_urls(text: &str) -> Vec<String> {
    urls_in(text)
        .into_iter()
        .filter(|url| !is_inert(url))
        .collect()
}
