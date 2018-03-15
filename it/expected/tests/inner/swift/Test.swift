public struct Test_Entry: Codable {
  let a: Test_A?
  let b: Test_A_B?

  enum CodingKeys: String, CodingKey {
    case a = "a"
    case b = "b"
  }
}

public struct Test_A: Codable {
  let b: Test_A_B

  enum CodingKeys: String, CodingKey {
    case b = "b"
  }
}

public struct Test_A_B: Codable {
  let field: String

  enum CodingKeys: String, CodingKey {
    case field = "field"
  }
}
