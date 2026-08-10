// A vendored namesake of crates/log. Nothing should reach it from the
// workspace: the nearer crate of the same name wins.
pub fn note() -> i32 {
    99
}
