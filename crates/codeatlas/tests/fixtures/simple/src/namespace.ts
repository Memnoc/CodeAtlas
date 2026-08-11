// A namespace import: the local name stands for the whole module, so
// `util.greet` is a call into `./util` even though nothing named `greet` is
// bound in this file's scope.
import * as util from "./util";
import * as u from "./util";
// The decoy: a package outside the map whose member shares a name with a
// real export of `./util`, so resolving by name alone would wire them up.
import * as ext from "node:util";

export function viaNamespace(): string {
  return util.greet("ns");
}

// The same module under a different local name — the alias, not the
// specifier, is what the call site writes.
export function viaAlias(): string {
  return u.greet("alias");
}

// The receiver is a package outside the map: no edge may be invented, even
// though `greet` is a name this repository really does export.
export function viaExternal(): string {
  return ext.greet("outside");
}
