// Inside the vendored tree, `log` means the vendored log — the near one, not
// the workspace's. Path order alone would pick crates/log, so this is the
// case that makes locality load-bearing rather than decorative.
use log::note;

fn main() {
    println!("{}", note());
}
