// The standard library is not in the scanned tree and must stay unresolved.
use std::collections::HashMap;
// Workspace siblings, reached by crate name.
use atlas_engine::engine::run;
// The package is `atlas-tools`; its directory is `toolbox/`. Only the
// manifest knows, so this edge exists only if the manifest is consulted.
use atlas_tools::helper;
// A crate in this tree that shares its name with one on crates.io, and with
// a vendored copy at vendor/log. The one in the tree wins over crates.io;
// the nearer of the two in the tree wins over the further.
use log::note;
// Not in the scanned tree at all: must resolve to nothing rather than guess.
use serde::Serialize;

fn main() {
    let mut seen: HashMap<&str, i32> = HashMap::new();
    seen.insert("total", run() + helper() + note());
    println!("{seen:?}");
}
