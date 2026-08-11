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

/// A local type whose method shares a name with `util::helper`.
pub struct Util;

impl Util {
    pub fn helper(&self) -> i32 {
        0
    }
}

/// The decoy for the value-receiver row, and it has to live *here*: this file
/// declares `mod util;`, so a bare `util::` written in it really does resolve
/// to `src/util.rs`. Rust's `::` may be resolved on sight precisely because it
/// can only ever separate path segments. A `.` promises nothing — `util` below
/// is a binding holding a value — and following one here would fabricate an
/// edge the source does not contain.
pub fn call_on_a_value(util: Util) -> i32 {
    util.helper()
}
