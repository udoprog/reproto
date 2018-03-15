public struct Test_Entry: Codable {
}

public struct Test_Type: Codable {
}

public enum Test_Interface {
  case SubType(Test_Interface_SubType)
  enum CodingKeys: String, CodingKey {
    case tag = "type"
  }
}

extension Test_Interface: Decodable {
  public init(from decoder: Decoder) throws {
    let values = try decoder.container(keyedBy: CodingKeys.self)

    switch try values.decode(String.self, forKey: .tag) {
    case "SubType":
      self = try .SubType(Test_Interface_SubType(from: decoder))
    default:
      let context = DecodingError.Context(codingPath: [], debugDescription: "type")
      throw DecodingError.dataCorrupted(context)
    }
  }
}

extension Test_Interface: Encodable {
  public func encode(to encoder: Encoder) throws {
    var values = encoder.container(keyedBy: CodingKeys.self)

    switch self {
    case .SubType(let d):
      try values.encode("SubType", forKey: .tag)
      try d.encode(to: encoder)
    }
  }
}

public struct Test_Interface_SubType: Codable {
}

public enum Test_Enum {
  case Variant
}

extension Test_Enum: Decodable {
  public init(from decoder: Decoder) throws {
    let value = try decoder.singleValueContainer()

    switch try value.decode(String.self) {
    case "Variant":
      self = .Variant
    default:
      let context = DecodingError.Context(codingPath: [], debugDescription: "enum variant")
      throw DecodingError.dataCorrupted(context)
    }
  }
}

extension Test_Enum: Encodable {
  public func encode(to encoder: Encoder) throws {
    var value = encoder.singleValueContainer()

    switch self {
    case .Variant:
      try value.encode("Variant")
    }
  }
}

public struct Test_Tuple {
}
extension Test_Tuple: Decodable {
  public init(from decoder: Decoder) throws {
    var values = try decoder.unkeyedContainer()

  }
}
extension Test_Tuple: Encodable {
  public func encode(to encoder: Encoder) throws {
    var values = encoder.unkeyedContainer()

  }
}
