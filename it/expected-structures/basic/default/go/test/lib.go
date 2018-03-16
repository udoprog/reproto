package test

type Entry struct {
  // The foo field.
  Foo *Foo `json:"foo,omitempty"`
}

type Foo struct {
  // The field.
  Field string `json:"field"`
}

type Bar struct {
  // The inner field.
  Field Bar_Inner `json:"field"`
}

type Bar_Inner struct {
  // The field.
  Field string `json:"field"`
}
