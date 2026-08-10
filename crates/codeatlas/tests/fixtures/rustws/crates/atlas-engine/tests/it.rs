// A crate naming itself. This is the only form an integration test can use —
// `crate::` does not reach the library from tests/ — and the package name is
// hyphenated where the path is not.
use atlas_engine::engine::run;

#[test]
fn runs() {
    assert_eq!(run(), 42);
}
