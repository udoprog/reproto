public struct LowerCamel_Value: Codable {
  let foo_bar: String

  enum CodingKeys: String, CodingKey {
    case foo_bar = "fooBar"
  }
}
