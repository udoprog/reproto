pub enum Entry {
  Entry {
    #[serde(rename = "foo")]
    A,
    #[serde(rename = "bar")]
    B,
  }
}

pub enum Entry2 {
  Entry2 {
    A,
    B,
    C,
  }
}
