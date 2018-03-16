package test

import "encoding/json"
import "errors"

type Entry struct {
  Explicit *EnumExplicit `json:"explicit,omitempty"`

  Implicit *EnumImplicit `json:"implicit,omitempty"`
}

// Explicitly assigned strings
type EnumExplicit int

const (
  EnumExplicit_A EnumExplicit = iota
  EnumExplicit_B
)

func (this *EnumExplicit) UnmarshalJSON(b []byte) error {
  var s string

  if err := json.Unmarshal(b, &s); err != nil {
    return err
  }

  switch s {
  case "foo":
    *this = EnumExplicit_A
  case "bar":
    *this = EnumExplicit_B
  default:
    return errors.New("bad value")
  }

  return nil
}

func (this EnumExplicit) MarshalJSON() ([]byte, error) {
  var s string

  switch this {
  case EnumExplicit_A:
    s = "foo"
  case EnumExplicit_B:
    s = "bar"
  default:
    return nil, errors.New("bad value")
  }

  return json.Marshal(s)
}

// Implicit naming depending on the variant
type EnumImplicit int

const (
  EnumImplicit_A EnumImplicit = iota
  EnumImplicit_B
)

func (this *EnumImplicit) UnmarshalJSON(b []byte) error {
  var s string

  if err := json.Unmarshal(b, &s); err != nil {
    return err
  }

  switch s {
  case "A":
    *this = EnumImplicit_A
  case "B":
    *this = EnumImplicit_B
  default:
    return errors.New("bad value")
  }

  return nil
}

func (this EnumImplicit) MarshalJSON() ([]byte, error) {
  var s string

  switch this {
  case EnumImplicit_A:
    s = "A"
  case EnumImplicit_B:
    s = "B"
  default:
    return nil, errors.New("bad value")
  }

  return json.Marshal(s)
}

// Variants with long names.
type EnumLongNames int

const (
  EnumLongNames_FooBar EnumLongNames = iota
  EnumLongNames_Baz
)

func (this *EnumLongNames) UnmarshalJSON(b []byte) error {
  var s string

  if err := json.Unmarshal(b, &s); err != nil {
    return err
  }

  switch s {
  case "FooBar":
    *this = EnumLongNames_FooBar
  case "Baz":
    *this = EnumLongNames_Baz
  default:
    return errors.New("bad value")
  }

  return nil
}

func (this EnumLongNames) MarshalJSON() ([]byte, error) {
  var s string

  switch this {
  case EnumLongNames_FooBar:
    s = "FooBar"
  case EnumLongNames_Baz:
    s = "Baz"
  default:
    return nil, errors.New("bad value")
  }

  return json.Marshal(s)
}
