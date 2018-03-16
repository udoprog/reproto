package foo_4_0_0

import "../bar_1_0_0"
import "../bar_2_0_0"

type Thing struct {
  Name *string `json:"name,omitempty"`

  Other *bar_1_0_0.Other `json:"other,omitempty"`

  Other2 *bar_2_0_0.Other `json:"other2,omitempty"`
}
