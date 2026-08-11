package main

// The decoy for the value-receiver row: `util` here is a parameter holding a
// Logger, while the in-repo package of that name really does export `Format`.
// A resolver that read any dotted receiver as a package name would wire this
// call into `util/util.go`.
type Logger struct{}

func (l Logger) Format(value string) string {
	return value
}

func onValue(util Logger) string {
	return util.Format("value")
}
