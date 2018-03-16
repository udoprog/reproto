package test

type Entry interface{}

type Entry_A struct {
  Shared string `json:"shared"`
}

type Entry_B struct {
  Shared string `json:"shared"`
}

type Entry_Bar struct {
  Shared string `json:"shared"`
}

type Entry_Baz struct {
  Shared string `json:"shared"`
}

func (this *Entry) UnmarshalJSON(b []byte) error {
}

func (this Entry) MarshalJSON() ([]byte, error) {
}
