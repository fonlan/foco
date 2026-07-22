package greeting

import (
	fmt "fmt"
	"example.com/project/text"
)

type Greeter struct{}

func (Greeter) Format(value string) string {
	return helper(value)
}

func helper(value string) string {
	return value
}

func caller(value string) {
	_ = helper(value)
	fmt.Println(value)
}
