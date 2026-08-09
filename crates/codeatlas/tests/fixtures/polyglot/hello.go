package main

type Greeter struct {
	prefix string
}

func Greet(name string) string {
	return decorate(name)
}

func decorate(name string) string {
	return "* " + name
}
