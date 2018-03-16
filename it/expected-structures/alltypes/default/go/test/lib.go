package test

type Entry struct {
  BooleanType *bool `json:"boolean_type,omitempty"`

  StringType *string `json:"string_type,omitempty"`

  DatetimeType *string `json:"datetime_type,omitempty"`

  Unsigned32 *uint32 `json:"unsigned_32,omitempty"`

  Unsigned64 *uint64 `json:"unsigned_64,omitempty"`

  Signed32 *int32 `json:"signed_32,omitempty"`

  Signed64 *int64 `json:"signed_64,omitempty"`

  FloatType *float32 `json:"float_type,omitempty"`

  DoubleType *float64 `json:"double_type,omitempty"`

  BytesType *string `json:"bytes_type,omitempty"`

  AnyType *interface{} `json:"any_type,omitempty"`

  ArrayType *[]Entry `json:"array_type,omitempty"`

  MapType *map[string]Entry `json:"map_type,omitempty"`
}
