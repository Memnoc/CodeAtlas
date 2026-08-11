pub fn helper() -> i32 {
    7
}

/// The decoy. `serde_json::from_str` in `lib.rs` writes this same name, so a
/// resolver that matched callees by name alone — rather than through the
/// module the receiver actually resolves to — would wire the two together.
pub fn from_str(_text: &str) -> Option<i32> {
    None
}
