package main

// A dot import binds every exported name of the package unqualified, so this
// is the one unqualified cross-package call Go has. It is legal Go, which is
// why the checklist's unqualified-call row is a gap for Go rather than an
// inapplicable convention: `Format` here is `util.Format`, reached with no
// qualifier at all.
import . "example.com/demo/util"

func viaDot() string {
	return Format("dot")
}

// "Every exported name of the *package*", not of the file the import edge
// lands on: `Extra` lives in `util/extra.go`, one directory-mate away from
// `util/util.go`, and a dot import binds it just the same.
func viaDotSecondFile() string {
	return Extra("dot")
}
