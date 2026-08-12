package main

import (
	"fmt"

	"example.com/demo/util"
)

func main() {
	fmt.Println(util.Format(run()))
}

// The same qualifier reaching the package's *other* file. A Go import names a
// directory, so a resolver that bound `util` to the package's anchor file
// alone would resolve `Format` above and silently miss this — right on a
// one-file package, quietly wrong on a real one.
func second() string {
	return util.Extra("second")
}
