// The crate sits at the scan root, so no directory in any scanned path is
// named after it: resolving this needs the manifest.
use root_lib::util::helper;

#[test]
fn helps() {
    // The bare `helper()` is what makes this a *member* import rather than a
    // module one: `use root_lib::util;` produces the identical file edge, and
    // only the member form binds the name so it can be written unqualified.
    // Bound to a local first, because a call written inside `assert_eq!` sits
    // in a macro token tree and is not recorded as a call at all.
    let seven = helper();
    assert_eq!(seven, 7);
}
