package util

// A second file in the package, so that an import edge landing on util.go is
// evidence of the directory-anchor rule (`<dir>/<dir>.go` wins) rather than of
// the package happening to hold exactly one file.
func Extra(value string) string {
	return value
}
