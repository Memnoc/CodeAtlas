pub mod deep;
pub mod util;

// The last segment is a module, not an item. Python needs a rule for this
// shape (`from pkg import util`); Rust already had one, and this is what
// keeps that true.
use crate::deep::leaf;

// A bare local-module path in a `use`. `mod util;` (the `pub mod` above)
// already makes the file edge, so the binding must land too — this is the
// import-convention half of ticket 21.
use util::helper;

// A module aliased by a `use`: calls through `u::` are calls through `util`.
use crate::util as u;

pub fn tip() -> u8 {
    leaf::tip()
}

/// Fully qualified from the crate root.
pub fn from_crate_root() -> i32 {
    crate::util::helper()
}

/// A bare local-module path, written inline at the call site.
pub fn from_bare_module() -> i32 {
    util::helper()
}

/// Through the module alias.
pub fn through_alias() -> i32 {
    u::helper()
}

/// Bound by `use util::helper;`, so the call is unqualified.
pub fn from_bound_name() -> i32 {
    helper()
}

/// `self::` — the module this file is, spelled out.
pub fn from_self() -> i32 {
    self::util::helper()
}

/// The receiver is a crate outside the scanned tree. Resolving more must not
/// start inventing edges: this one stays unresolved.
pub fn external() -> Option<i32> {
    serde_json::from_str("7").ok()
}
