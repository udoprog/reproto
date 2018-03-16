package test

import "encoding/json"
import "errors"

type Entry struct {
}

type RootType struct {
}

type RootInterface interface{}

type RootInterface_Foo struct {
}

func (this *RootInterface) UnmarshalJSON(b []byte) error {
}

func (this RootInterface) MarshalJSON() ([]byte, error) {
}

type RootEnum int

const (
  RootEnum_Foo RootEnum = iota
)

func (this *RootEnum) UnmarshalJSON(b []byte) error {
  var s string

  if err := json.Unmarshal(b, &s); err != nil {
    return err
  }

  switch s {
  case "Foo":
    *this = RootEnum_Foo
  default:
    return errors.New("bad value")
  }

  return nil
}

func (this RootEnum) MarshalJSON() ([]byte, error) {
  var s string

  switch this {
  case RootEnum_Foo:
    s = "Foo"
  default:
    return nil, errors.New("bad value")
  }

  return json.Marshal(s)
}

type RootTuple struct {
}

func (this *RootTuple) UnmarshalJSON(b []byte) error {
  var array []json.RawMessage

  if err := json.Unmarshal(b, &array); err != nil {
    return err
  }


  return nil
}

func (this RootTuple) MarshalJSON() ([]byte, error) {
  var array []json.RawMessage


  return json.Marshal(array)
}

type RootType_NestedType struct {
}

type RootType_NestedInterface interface{}

type RootType_NestedInterface_Foo struct {
}

func (this *RootType_NestedInterface) UnmarshalJSON(b []byte) error {
}

func (this RootType_NestedInterface) MarshalJSON() ([]byte, error) {
}

type RootType_NestedEnum int

const (
  RootType_NestedEnum_Foo RootType_NestedEnum = iota
)

func (this *RootType_NestedEnum) UnmarshalJSON(b []byte) error {
  var s string

  if err := json.Unmarshal(b, &s); err != nil {
    return err
  }

  switch s {
  case "Foo":
    *this = RootType_NestedEnum_Foo
  default:
    return errors.New("bad value")
  }

  return nil
}

func (this RootType_NestedEnum) MarshalJSON() ([]byte, error) {
  var s string

  switch this {
  case RootType_NestedEnum_Foo:
    s = "Foo"
  default:
    return nil, errors.New("bad value")
  }

  return json.Marshal(s)
}

type RootType_NestedTuple struct {
}

func (this *RootType_NestedTuple) UnmarshalJSON(b []byte) error {
  var array []json.RawMessage

  if err := json.Unmarshal(b, &array); err != nil {
    return err
  }


  return nil
}

func (this RootType_NestedTuple) MarshalJSON() ([]byte, error) {
  var array []json.RawMessage


  return json.Marshal(array)
}

type RootInterface_Foo_NestedType struct {
}

type RootInterface_Foo_NestedInterface interface{}

type RootInterface_Foo_NestedInterface_NestedFoo struct {
}

func (this *RootInterface_Foo_NestedInterface) UnmarshalJSON(b []byte) error {
}

func (this RootInterface_Foo_NestedInterface) MarshalJSON() ([]byte, error) {
}

type RootInterface_Foo_NestedEnum int

const (
  RootInterface_Foo_NestedEnum_Foo RootInterface_Foo_NestedEnum = iota
)

func (this *RootInterface_Foo_NestedEnum) UnmarshalJSON(b []byte) error {
  var s string

  if err := json.Unmarshal(b, &s); err != nil {
    return err
  }

  switch s {
  case "Foo":
    *this = RootInterface_Foo_NestedEnum_Foo
  default:
    return errors.New("bad value")
  }

  return nil
}

func (this RootInterface_Foo_NestedEnum) MarshalJSON() ([]byte, error) {
  var s string

  switch this {
  case RootInterface_Foo_NestedEnum_Foo:
    s = "Foo"
  default:
    return nil, errors.New("bad value")
  }

  return json.Marshal(s)
}

type RootInterface_Foo_NestedTuple struct {
}

func (this *RootInterface_Foo_NestedTuple) UnmarshalJSON(b []byte) error {
  var array []json.RawMessage

  if err := json.Unmarshal(b, &array); err != nil {
    return err
  }


  return nil
}

func (this RootInterface_Foo_NestedTuple) MarshalJSON() ([]byte, error) {
  var array []json.RawMessage


  return json.Marshal(array)
}

type RootTuple_NestedType struct {
}

type RootTuple_NestedInterface interface{}

type RootTuple_NestedInterface_Foo struct {
}

func (this *RootTuple_NestedInterface) UnmarshalJSON(b []byte) error {
}

func (this RootTuple_NestedInterface) MarshalJSON() ([]byte, error) {
}

type RootTuple_NestedEnum int

const (
  RootTuple_NestedEnum_Foo RootTuple_NestedEnum = iota
)

func (this *RootTuple_NestedEnum) UnmarshalJSON(b []byte) error {
  var s string

  if err := json.Unmarshal(b, &s); err != nil {
    return err
  }

  switch s {
  case "Foo":
    *this = RootTuple_NestedEnum_Foo
  default:
    return errors.New("bad value")
  }

  return nil
}

func (this RootTuple_NestedEnum) MarshalJSON() ([]byte, error) {
  var s string

  switch this {
  case RootTuple_NestedEnum_Foo:
    s = "Foo"
  default:
    return nil, errors.New("bad value")
  }

  return json.Marshal(s)
}

type RootTuple_NestedTuple struct {
}

func (this *RootTuple_NestedTuple) UnmarshalJSON(b []byte) error {
  var array []json.RawMessage

  if err := json.Unmarshal(b, &array); err != nil {
    return err
  }


  return nil
}

func (this RootTuple_NestedTuple) MarshalJSON() ([]byte, error) {
  var array []json.RawMessage


  return json.Marshal(array)
}

type RootService_NestedType struct {
}

type RootService_NestedInterface interface{}

type RootService_NestedInterface_Foo struct {
}

func (this *RootService_NestedInterface) UnmarshalJSON(b []byte) error {
}

func (this RootService_NestedInterface) MarshalJSON() ([]byte, error) {
}

type RootService_NestedEnum int

const (
  RootService_NestedEnum_Foo RootService_NestedEnum = iota
)

func (this *RootService_NestedEnum) UnmarshalJSON(b []byte) error {
  var s string

  if err := json.Unmarshal(b, &s); err != nil {
    return err
  }

  switch s {
  case "Foo":
    *this = RootService_NestedEnum_Foo
  default:
    return errors.New("bad value")
  }

  return nil
}

func (this RootService_NestedEnum) MarshalJSON() ([]byte, error) {
  var s string

  switch this {
  case RootService_NestedEnum_Foo:
    s = "Foo"
  default:
    return nil, errors.New("bad value")
  }

  return json.Marshal(s)
}

type RootService_NestedTuple struct {
}

func (this *RootService_NestedTuple) UnmarshalJSON(b []byte) error {
  var array []json.RawMessage

  if err := json.Unmarshal(b, &array); err != nil {
    return err
  }


  return nil
}

func (this RootService_NestedTuple) MarshalJSON() ([]byte, error) {
  var array []json.RawMessage


  return json.Marshal(array)
}

type RootType_NestedInterface_Foo_Nested struct {
}

type RootType_NestedTuple_Nested struct {
}

type RootType_NestedService_Nested struct {
}

type RootInterface_Foo_NestedInterface_NestedFoo_Nested struct {
}

type RootInterface_Foo_NestedTuple_Nested struct {
}

type RootInterface_Foo_NestedService_Nested struct {
}

type RootTuple_NestedInterface_Foo_Nested struct {
}

type RootTuple_NestedTuple_Nested struct {
}

type RootTuple_NestedService_Nested struct {
}

type RootService_NestedInterface_Foo_Nested struct {
}

type RootService_NestedTuple_Nested struct {
}

type RootService_NestedService_Nested struct {
}
