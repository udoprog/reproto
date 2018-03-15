public struct Test_Entry: Codable {
  // The foo field.
  let foo: Test_Foo?

  enum CodingKeys: String, CodingKey {
    case foo = "foo"
  }
}

public struct Test_Foo: Codable {
  // The field.
  let field: String

  enum CodingKeys: String, CodingKey {
    case field = "field"
  }
}

public struct Test_Bar: Codable {
  // The inner field.
  let field: Test_Bar_Inner

  enum CodingKeys: String, CodingKey {
    case field = "field"
  }
}

public struct Test_Bar_Inner: Codable {
  // The field.
  let field: String

  enum CodingKeys: String, CodingKey {
    case field = "field"
  }
}
