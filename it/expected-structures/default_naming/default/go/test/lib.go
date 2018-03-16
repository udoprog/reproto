package test

import "../lower_camel"
import "../lower_snake"
import "../upper_camel"
import "../upper_snake"

type Entry struct {
  LowerCamel *lower_camel.Value `json:"lower_camel,omitempty"`

  LowerSnake *lower_snake.Value `json:"lower_snake,omitempty"`

  UpperCamel *upper_camel.Value `json:"upper_camel,omitempty"`

  UpperSnake *upper_snake.Value `json:"upper_snake,omitempty"`
}
