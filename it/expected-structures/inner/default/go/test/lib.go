package test

type Entry struct {
  A *A `json:"a,omitempty"`

  B *A_B `json:"b,omitempty"`
}

type A struct {
  B A_B `json:"b"`
}

type A_B struct {
  Field string `json:"field"`
}
