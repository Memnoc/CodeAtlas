/// A module aliased by a `use`. This file reaches `src/util.rs` through this
/// statement and no other, so the import edge is itself evidence the alias
/// resolved — and the call below is evidence the alias *bound* the module.
use crate::util as u;

pub fn tip() -> u8 {
    3
}

pub fn via_alias() -> i32 {
    u::helper()
}
