pub mod leaf;

/// `super::` — from the `deep` module that is one hop up, which is the crate
/// root. The receiver is two segments and neither is a name a `use` bound.
pub fn up_and_across() -> i32 {
    super::util::helper()
}
