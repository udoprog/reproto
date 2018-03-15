public enum Test_Entry {
  case A(Test_Entry_A)
  case B(Test_Entry_B)
  case Bar(Test_Entry_Bar)
  case Baz(Test_Entry_Baz)
  enum CodingKeys: String, CodingKey {
    case tag = "@type"
  }
}

extension Test_Entry: Decodable {
  public init(from decoder: Decoder) throws {
    let values = try decoder.container(keyedBy: CodingKeys.self)

    switch try values.decode(String.self, forKey: .tag) {
    case "foo":
      self = try .A(Test_Entry_A(from: decoder))
    case "b":
      self = try .B(Test_Entry_B(from: decoder))
    case "Bar":
      self = try .Bar(Test_Entry_Bar(from: decoder))
    case "Baz":
      self = try .Baz(Test_Entry_Baz(from: decoder))
    default:
      let context = DecodingError.Context(codingPath: [], debugDescription: "@type")
      throw DecodingError.dataCorrupted(context)
    }
  }
}

extension Test_Entry: Encodable {
  public func encode(to encoder: Encoder) throws {
    var values = encoder.container(keyedBy: CodingKeys.self)

    switch self {
    case .A(let d):
      try values.encode("foo", forKey: .tag)
      try d.encode(to: encoder)
    case .B(let d):
      try values.encode("b", forKey: .tag)
      try d.encode(to: encoder)
    case .Bar(let d):
      try values.encode("Bar", forKey: .tag)
      try d.encode(to: encoder)
    case .Baz(let d):
      try values.encode("Baz", forKey: .tag)
      try d.encode(to: encoder)
    }
  }
}

public struct Test_Entry_A: Codable {
  let shared: String

  enum CodingKeys: String, CodingKey {
    case shared = "shared"
  }
}

public struct Test_Entry_B: Codable {
  let shared: String

  enum CodingKeys: String, CodingKey {
    case shared = "shared"
  }
}

public struct Test_Entry_Bar: Codable {
  let shared: String

  enum CodingKeys: String, CodingKey {
    case shared = "shared"
  }
}

public struct Test_Entry_Baz: Codable {
  let shared: String

  enum CodingKeys: String, CodingKey {
    case shared = "shared"
  }
}
