pub mod leaf;

/// A relative path in a `use`. `super::` from this child module is the crate
/// root, and this is the only statement in the file that reaches
/// `src/util.rs`, so the edge is evidence the relative path resolved rather
/// than something else in the file happening to reach the same target.
use super::util::helper;

pub fn relative() -> i32 {
    helper()
}

/// `super::` — from the `deep` module that is one hop up, which is the crate
/// root. The receiver is two segments and neither is a name a `use` bound.
pub fn up_and_across() -> i32 {
    super::util::helper()
}
