package main

// An aliased import: the local name replaces the package's own, and this file
// reaches the package through this statement alone.
import u "example.com/demo/util"

func aliased() string {
	return u.Format("alias")
}
