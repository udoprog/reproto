public struct UpperCamel_Value: Codable {
  let foo_bar: String

  enum CodingKeys: String, CodingKey {
    case foo_bar = "FooBar"
  }
}
