public struct UpperSnake_Value: Codable {
  let foo_bar: String

  enum CodingKeys: String, CodingKey {
    case foo_bar = "FOO_BAR"
  }
}
