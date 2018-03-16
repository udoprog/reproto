package test

import "encoding/json"

type Entry struct {
  Tuple1 *Tuple1 `json:"tuple1,omitempty"`

  Tuple2 *Tuple2 `json:"tuple2,omitempty"`
}

// Tuple containing primitive.
type Tuple1 struct {
  A string

  B uint64
}

func (this *Tuple1) UnmarshalJSON(b []byte) error {
  var array []json.RawMessage

  if err := json.Unmarshal(b, &array); err != nil {
    return err
  }

  var A string
  if err := json.Unmarshal(array[0], &A); err != nil {
    return err
  }
  this.A = A

  var B uint64
  if err := json.Unmarshal(array[1], &B); err != nil {
    return err
  }
  this.B = B

  return nil
}

func (this Tuple1) MarshalJSON() ([]byte, error) {
  var array []json.RawMessage

  A, err := json.Marshal(this.A)

  if err != nil {
    return nil, err
  }

  array = append(array, A)

  B, err := json.Marshal(this.B)

  if err != nil {
    return nil, err
  }

  array = append(array, B)

  return json.Marshal(array)
}

// Tuple containing object.
type Tuple2 struct {
  A string

  B Other
}

func (this *Tuple2) UnmarshalJSON(b []byte) error {
  var array []json.RawMessage

  if err := json.Unmarshal(b, &array); err != nil {
    return err
  }

  var A string
  if err := json.Unmarshal(array[0], &A); err != nil {
    return err
  }
  this.A = A

  var B Other
  if err := json.Unmarshal(array[1], &B); err != nil {
    return err
  }
  this.B = B

  return nil
}

func (this Tuple2) MarshalJSON() ([]byte, error) {
  var array []json.RawMessage

  A, err := json.Marshal(this.A)

  if err != nil {
    return nil, err
  }

  array = append(array, A)

  B, err := json.Marshal(this.B)

  if err != nil {
    return nil, err
  }

  array = append(array, B)

  return json.Marshal(array)
}

// Complex object.
type Other struct {
  A string `json:"a"`
}
