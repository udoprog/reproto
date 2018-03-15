public struct Test_Entry: Codable {
  let explicit: Test_EnumExplicit?
  let implicit: Test_EnumImplicit?

  enum CodingKeys: String, CodingKey {
    case explicit = "explicit"
    case implicit = "implicit"
  }
}

public enum Test_EnumExplicit {
  case A
  case B
}

extension Test_EnumExplicit: Decodable {
  public init(from decoder: Decoder) throws {
    let value = try decoder.singleValueContainer()

    switch try value.decode(String.self) {
    case "foo":
      self = .A
    case "bar":
      self = .B
    default:
      let context = DecodingError.Context(codingPath: [], debugDescription: "enum variant")
      throw DecodingError.dataCorrupted(context)
    }
  }
}

extension Test_EnumExplicit: Encodable {
  public func encode(to encoder: Encoder) throws {
    var value = encoder.singleValueContainer()

    switch self {
    case .A:
      try value.encode("foo")
    case .B:
      try value.encode("bar")
    }
  }
}

public enum Test_EnumImplicit {
  case A
  case B
}

extension Test_EnumImplicit: Decodable {
  public init(from decoder: Decoder) throws {
    let value = try decoder.singleValueContainer()

    switch try value.decode(String.self) {
    case "A":
      self = .A
    case "B":
      self = .B
    default:
      let context = DecodingError.Context(codingPath: [], debugDescription: "enum variant")
      throw DecodingError.dataCorrupted(context)
    }
  }
}

extension Test_EnumImplicit: Encodable {
  public func encode(to encoder: Encoder) throws {
    var value = encoder.singleValueContainer()

    switch self {
    case .A:
      try value.encode("A")
    case .B:
      try value.encode("B")
    }
  }
}

public enum Test_EnumLongNames {
  case FooBar
  case Baz
}

extension Test_EnumLongNames: Decodable {
  public init(from decoder: Decoder) throws {
    let value = try decoder.singleValueContainer()

    switch try value.decode(String.self) {
    case "FooBar":
      self = .FooBar
    case "Baz":
      self = .Baz
    default:
      let context = DecodingError.Context(codingPath: [], debugDescription: "enum variant")
      throw DecodingError.dataCorrupted(context)
    }
  }
}

extension Test_EnumLongNames: Encodable {
  public func encode(to encoder: Encoder) throws {
    var value = encoder.singleValueContainer()

    switch self {
    case .FooBar:
      try value.encode("FooBar")
    case .Baz:
      try value.encode("Baz")
    }
  }
}
