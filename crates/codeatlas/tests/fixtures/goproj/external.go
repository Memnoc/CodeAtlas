package main

import "github.com/external/lib/util"

// `util` here is the *external* module, and the in-repo `util` package
// exports a `Format` of its own — so a resolver that matched a qualified
// callee by name, or a receiver by package suffix, would wire the two
// together. The go.mod module line is what says this import is not ours.
func external() string {
	return util.Format("outside")
}
