package main

// The decoy for the value-receiver row, and the import is the half that makes
// it one. This file really does import the `util` package, so the name `util`
// is a module here — and then shadows it with a parameter of the same name
// holding a Logger. Without the import there is nothing for a resolver to bind
// `util` to and the row asserts nothing, however inviting the call site looks.
// With it, a resolver that reads a dotted receiver as a package name without
// asking whether the call site shadowed it wires this call into util/util.go.
import "example.com/demo/util"

type Logger struct{}

func (l Logger) Format(value string) string {
	return value
}

func onValue(util Logger) string {
	return util.Format("value")
}

// The un-shadowed use of the same name, which keeps the import legal Go and
// makes the shadowing above the only difference between the two.
var _ = util.Extra
