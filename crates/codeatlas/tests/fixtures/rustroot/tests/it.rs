// The crate sits at the scan root, so no directory in any scanned path is
// named after it: resolving this needs the manifest.
use root_lib::util::helper;

#[test]
fn helps() {
    assert_eq!(helper(), 7);
}
