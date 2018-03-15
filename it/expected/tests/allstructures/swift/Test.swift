public struct Test_Entry: Codable {
}

public struct Test_RootType: Codable {
}

public enum Test_RootInterface {
  case Foo(Test_RootInterface_Foo)
  enum CodingKeys: String, CodingKey {
    case tag = "type"
  }
}

extension Test_RootInterface: Decodable {
  public init(from decoder: Decoder) throws {
    let values = try decoder.container(keyedBy: CodingKeys.self)

    switch try values.decode(String.self, forKey: .tag) {
    case "Foo":
      self = try .Foo(Test_RootInterface_Foo(from: decoder))
    default:
      let context = DecodingError.Context(codingPath: [], debugDescription: "type")
      throw DecodingError.dataCorrupted(context)
    }
  }
}

extension Test_RootInterface: Encodable {
  public func encode(to encoder: Encoder) throws {
    var values = encoder.container(keyedBy: CodingKeys.self)

    switch self {
    case .Foo(let d):
      try values.encode("Foo", forKey: .tag)
      try d.encode(to: encoder)
    }
  }
}

public struct Test_RootInterface_Foo: Codable {
}

public enum Test_RootEnum {
  case Foo
}

extension Test_RootEnum: Decodable {
  public init(from decoder: Decoder) throws {
    let value = try decoder.singleValueContainer()

    switch try value.decode(String.self) {
    case "Foo":
      self = .Foo
    default:
      let context = DecodingError.Context(codingPath: [], debugDescription: "enum variant")
      throw DecodingError.dataCorrupted(context)
    }
  }
}

extension Test_RootEnum: Encodable {
  public func encode(to encoder: Encoder) throws {
    var value = encoder.singleValueContainer()

    switch self {
    case .Foo:
      try value.encode("Foo")
    }
  }
}

public struct Test_RootTuple {
}
extension Test_RootTuple: Decodable {
  public init(from decoder: Decoder) throws {
    var values = try decoder.unkeyedContainer()

  }
}
extension Test_RootTuple: Encodable {
  public func encode(to encoder: Encoder) throws {
    var values = encoder.unkeyedContainer()

  }
}

public struct Test_RootType_NestedType: Codable {
}

public enum Test_RootType_NestedInterface {
  case Foo(Test_RootType_NestedInterface_Foo)
  enum CodingKeys: String, CodingKey {
    case tag = "type"
  }
}

extension Test_RootType_NestedInterface: Decodable {
  public init(from decoder: Decoder) throws {
    let values = try decoder.container(keyedBy: CodingKeys.self)

    switch try values.decode(String.self, forKey: .tag) {
    case "Foo":
      self = try .Foo(Test_RootType_NestedInterface_Foo(from: decoder))
    default:
      let context = DecodingError.Context(codingPath: [], debugDescription: "type")
      throw DecodingError.dataCorrupted(context)
    }
  }
}

extension Test_RootType_NestedInterface: Encodable {
  public func encode(to encoder: Encoder) throws {
    var values = encoder.container(keyedBy: CodingKeys.self)

    switch self {
    case .Foo(let d):
      try values.encode("Foo", forKey: .tag)
      try d.encode(to: encoder)
    }
  }
}

public struct Test_RootType_NestedInterface_Foo: Codable {
}

public enum Test_RootType_NestedEnum {
  case Foo
}

extension Test_RootType_NestedEnum: Decodable {
  public init(from decoder: Decoder) throws {
    let value = try decoder.singleValueContainer()

    switch try value.decode(String.self) {
    case "Foo":
      self = .Foo
    default:
      let context = DecodingError.Context(codingPath: [], debugDescription: "enum variant")
      throw DecodingError.dataCorrupted(context)
    }
  }
}

extension Test_RootType_NestedEnum: Encodable {
  public func encode(to encoder: Encoder) throws {
    var value = encoder.singleValueContainer()

    switch self {
    case .Foo:
      try value.encode("Foo")
    }
  }
}

public struct Test_RootType_NestedTuple {
}
extension Test_RootType_NestedTuple: Decodable {
  public init(from decoder: Decoder) throws {
    var values = try decoder.unkeyedContainer()

  }
}
extension Test_RootType_NestedTuple: Encodable {
  public func encode(to encoder: Encoder) throws {
    var values = encoder.unkeyedContainer()

  }
}

public struct Test_RootInterface_Foo_NestedType: Codable {
}

public enum Test_RootInterface_Foo_NestedInterface {
  case NestedFoo(Test_RootInterface_Foo_NestedInterface_NestedFoo)
  enum CodingKeys: String, CodingKey {
    case tag = "type"
  }
}

extension Test_RootInterface_Foo_NestedInterface: Decodable {
  public init(from decoder: Decoder) throws {
    let values = try decoder.container(keyedBy: CodingKeys.self)

    switch try values.decode(String.self, forKey: .tag) {
    case "NestedFoo":
      self = try .NestedFoo(Test_RootInterface_Foo_NestedInterface_NestedFoo(from: decoder))
    default:
      let context = DecodingError.Context(codingPath: [], debugDescription: "type")
      throw DecodingError.dataCorrupted(context)
    }
  }
}

extension Test_RootInterface_Foo_NestedInterface: Encodable {
  public func encode(to encoder: Encoder) throws {
    var values = encoder.container(keyedBy: CodingKeys.self)

    switch self {
    case .NestedFoo(let d):
      try values.encode("NestedFoo", forKey: .tag)
      try d.encode(to: encoder)
    }
  }
}

public struct Test_RootInterface_Foo_NestedInterface_NestedFoo: Codable {
}

public enum Test_RootInterface_Foo_NestedEnum {
  case Foo
}

extension Test_RootInterface_Foo_NestedEnum: Decodable {
  public init(from decoder: Decoder) throws {
    let value = try decoder.singleValueContainer()

    switch try value.decode(String.self) {
    case "Foo":
      self = .Foo
    default:
      let context = DecodingError.Context(codingPath: [], debugDescription: "enum variant")
      throw DecodingError.dataCorrupted(context)
    }
  }
}

extension Test_RootInterface_Foo_NestedEnum: Encodable {
  public func encode(to encoder: Encoder) throws {
    var value = encoder.singleValueContainer()

    switch self {
    case .Foo:
      try value.encode("Foo")
    }
  }
}

public struct Test_RootInterface_Foo_NestedTuple {
}
extension Test_RootInterface_Foo_NestedTuple: Decodable {
  public init(from decoder: Decoder) throws {
    var values = try decoder.unkeyedContainer()

  }
}
extension Test_RootInterface_Foo_NestedTuple: Encodable {
  public func encode(to encoder: Encoder) throws {
    var values = encoder.unkeyedContainer()

  }
}

public struct Test_RootTuple_NestedType: Codable {
}

public enum Test_RootTuple_NestedInterface {
  case Foo(Test_RootTuple_NestedInterface_Foo)
  enum CodingKeys: String, CodingKey {
    case tag = "type"
  }
}

extension Test_RootTuple_NestedInterface: Decodable {
  public init(from decoder: Decoder) throws {
    let values = try decoder.container(keyedBy: CodingKeys.self)

    switch try values.decode(String.self, forKey: .tag) {
    case "Foo":
      self = try .Foo(Test_RootTuple_NestedInterface_Foo(from: decoder))
    default:
      let context = DecodingError.Context(codingPath: [], debugDescription: "type")
      throw DecodingError.dataCorrupted(context)
    }
  }
}

extension Test_RootTuple_NestedInterface: Encodable {
  public func encode(to encoder: Encoder) throws {
    var values = encoder.container(keyedBy: CodingKeys.self)

    switch self {
    case .Foo(let d):
      try values.encode("Foo", forKey: .tag)
      try d.encode(to: encoder)
    }
  }
}

public struct Test_RootTuple_NestedInterface_Foo: Codable {
}

public enum Test_RootTuple_NestedEnum {
  case Foo
}

extension Test_RootTuple_NestedEnum: Decodable {
  public init(from decoder: Decoder) throws {
    let value = try decoder.singleValueContainer()

    switch try value.decode(String.self) {
    case "Foo":
      self = .Foo
    default:
      let context = DecodingError.Context(codingPath: [], debugDescription: "enum variant")
      throw DecodingError.dataCorrupted(context)
    }
  }
}

extension Test_RootTuple_NestedEnum: Encodable {
  public func encode(to encoder: Encoder) throws {
    var value = encoder.singleValueContainer()

    switch self {
    case .Foo:
      try value.encode("Foo")
    }
  }
}

public struct Test_RootTuple_NestedTuple {
}
extension Test_RootTuple_NestedTuple: Decodable {
  public init(from decoder: Decoder) throws {
    var values = try decoder.unkeyedContainer()

  }
}
extension Test_RootTuple_NestedTuple: Encodable {
  public func encode(to encoder: Encoder) throws {
    var values = encoder.unkeyedContainer()

  }
}

public struct Test_RootService_NestedType: Codable {
}

public enum Test_RootService_NestedInterface {
  case Foo(Test_RootService_NestedInterface_Foo)
  enum CodingKeys: String, CodingKey {
    case tag = "type"
  }
}

extension Test_RootService_NestedInterface: Decodable {
  public init(from decoder: Decoder) throws {
    let values = try decoder.container(keyedBy: CodingKeys.self)

    switch try values.decode(String.self, forKey: .tag) {
    case "Foo":
      self = try .Foo(Test_RootService_NestedInterface_Foo(from: decoder))
    default:
      let context = DecodingError.Context(codingPath: [], debugDescription: "type")
      throw DecodingError.dataCorrupted(context)
    }
  }
}

extension Test_RootService_NestedInterface: Encodable {
  public func encode(to encoder: Encoder) throws {
    var values = encoder.container(keyedBy: CodingKeys.self)

    switch self {
    case .Foo(let d):
      try values.encode("Foo", forKey: .tag)
      try d.encode(to: encoder)
    }
  }
}

public struct Test_RootService_NestedInterface_Foo: Codable {
}

public enum Test_RootService_NestedEnum {
  case Foo
}

extension Test_RootService_NestedEnum: Decodable {
  public init(from decoder: Decoder) throws {
    let value = try decoder.singleValueContainer()

    switch try value.decode(String.self) {
    case "Foo":
      self = .Foo
    default:
      let context = DecodingError.Context(codingPath: [], debugDescription: "enum variant")
      throw DecodingError.dataCorrupted(context)
    }
  }
}

extension Test_RootService_NestedEnum: Encodable {
  public func encode(to encoder: Encoder) throws {
    var value = encoder.singleValueContainer()

    switch self {
    case .Foo:
      try value.encode("Foo")
    }
  }
}

public struct Test_RootService_NestedTuple {
}
extension Test_RootService_NestedTuple: Decodable {
  public init(from decoder: Decoder) throws {
    var values = try decoder.unkeyedContainer()

  }
}
extension Test_RootService_NestedTuple: Encodable {
  public func encode(to encoder: Encoder) throws {
    var values = encoder.unkeyedContainer()

  }
}

public struct Test_RootType_NestedInterface_Foo_Nested: Codable {
}

public struct Test_RootType_NestedTuple_Nested: Codable {
}

public struct Test_RootType_NestedService_Nested: Codable {
}

public struct Test_RootInterface_Foo_NestedInterface_NestedFoo_Nested: Codable {
}

public struct Test_RootInterface_Foo_NestedTuple_Nested: Codable {
}

public struct Test_RootInterface_Foo_NestedService_Nested: Codable {
}

public struct Test_RootTuple_NestedInterface_Foo_Nested: Codable {
}

public struct Test_RootTuple_NestedTuple_Nested: Codable {
}

public struct Test_RootTuple_NestedService_Nested: Codable {
}

public struct Test_RootService_NestedInterface_Foo_Nested: Codable {
}

public struct Test_RootService_NestedTuple_Nested: Codable {
}

public struct Test_RootService_NestedService_Nested: Codable {
}
