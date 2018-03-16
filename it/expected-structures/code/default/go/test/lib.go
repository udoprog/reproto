package test

import "encoding/json"
import "errors"

type Entry struct {
}

type Type struct {
}

type Interface interface{}

type Interface_SubType struct {
}

func (this *Interface) UnmarshalJSON(b []byte) error {
}

func (this Interface) MarshalJSON() ([]byte, error) {
}

type Enum int

const (
  Enum_Variant Enum = iota
)

func (this *Enum) UnmarshalJSON(b []byte) error {
  var s string

  if err := json.Unmarshal(b, &s); err != nil {
    return err
  }

  switch s {
  case "Variant":
    *this = Enum_Variant
  default:
    return errors.New("bad value")
  }

  return nil
}

func (this Enum) MarshalJSON() ([]byte, error) {
  var s string

  switch this {
  case Enum_Variant:
    s = "Variant"
  default:
    return nil, errors.New("bad value")
  }

  return json.Marshal(s)
}

type Tuple struct {
}

func (this *Tuple) UnmarshalJSON(b []byte) error {
  var array []json.RawMessage

  if err := json.Unmarshal(b, &array); err != nil {
    return err
  }


  return nil
}

func (this Tuple) MarshalJSON() ([]byte, error) {
  var array []json.RawMessage


  return json.Marshal(array)
}
