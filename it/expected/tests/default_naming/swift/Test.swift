public struct Test_Entry: Codable {
  let lower_camel: LowerCamel_Value?
  let lower_snake: LowerSnake_Value?
  let upper_camel: UpperCamel_Value?
  let upper_snake: UpperSnake_Value?

  enum CodingKeys: String, CodingKey {
    case lower_camel = "lower_camel"
    case lower_snake = "lower_snake"
    case upper_camel = "upper_camel"
    case upper_snake = "upper_snake"
  }
}
