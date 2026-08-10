pub mod deep;
pub mod util;

// The last segment is a module, not an item. Python needs a rule for this
// shape (`from pkg import util`); Rust already had one, and this is what
// keeps that true.
use crate::deep::leaf;

pub fn tip() -> u8 {
    leaf::tip()
}
