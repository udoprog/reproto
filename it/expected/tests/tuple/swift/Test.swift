public struct Test_Entry: Codable {
  let tuple1: Test_Tuple1?
  let tuple2: Test_Tuple2?

  enum CodingKeys: String, CodingKey {
    case tuple1 = "tuple1"
    case tuple2 = "tuple2"
  }
}

// Tuple containing primitive.
public struct Test_Tuple1 {
  let a: String
  let b: UInt64

  enum CodingKeys: String, CodingKey {
    case a = "a"
    case b = "b"
  }
}
extension Test_Tuple1: Decodable {
  public init(from decoder: Decoder) throws {
    var values = try decoder.unkeyedContainer()

    self.a = try values.decode(String.self)
    self.b = try values.decode(UInt64.self)
  }
}
extension Test_Tuple1: Encodable {
  public func encode(to encoder: Encoder) throws {
    var values = encoder.unkeyedContainer()

    try values.encode(self.a)
    try values.encode(self.b)
  }
}

// Tuple containing object.
public struct Test_Tuple2 {
  let a: String
  let b: Test_Other

  enum CodingKeys: String, CodingKey {
    case a = "a"
    case b = "b"
  }
}
extension Test_Tuple2: Decodable {
  public init(from decoder: Decoder) throws {
    var values = try decoder.unkeyedContainer()

    self.a = try values.decode(String.self)
    self.b = try values.decode(Test_Other.self)
  }
}
extension Test_Tuple2: Encodable {
  public func encode(to encoder: Encoder) throws {
    var values = encoder.unkeyedContainer()

    try values.encode(self.a)
    try values.encode(self.b)
  }
}

// Complex object.
public struct Test_Other: Codable {
  let a: String

  enum CodingKeys: String, CodingKey {
    case a = "a"
  }
}
