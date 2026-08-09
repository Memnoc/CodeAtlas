package util

import "strings"

type Formatter struct {
	Prefix string
}

func (f Formatter) Render(value string) string {
	return f.Prefix + value
}

func Format(value string) string {
	return indent(strings.TrimSpace(value))
}

func indent(value string) string {
	return "  " + value
}
