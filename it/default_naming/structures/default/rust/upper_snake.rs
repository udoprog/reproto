#[derive(Serialize, Deserialize, Debug)]
pub struct Value {
  #[serde(rename = "FOO_BAR")]
  foo_bar: String,
}
