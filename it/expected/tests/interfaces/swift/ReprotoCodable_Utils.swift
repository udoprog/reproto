class AnyCodable: Codable {
  public let value: Any

  public required init(from decoder: Decoder) throws {
    if var array = try? decoder.unkeyedContainer() {
      self.value = try AnyCodable.decodeArray(from: &array)
      return
    }
    if var c = try? decoder.container(keyedBy: AnyCodingKey.self) {
      self.value = try AnyCodable.decodeDictionary(from: &c)
      return
    }
    let c = try decoder.singleValueContainer()
    self.value = try AnyCodable.decode(from: c)
  }

  public func encode(to encoder: Encoder) throws {
    if let arr = self.value as? [Any] {
      var c = encoder.unkeyedContainer()
      try AnyCodable.encode(to: &c, array: arr)
      return
    }
    if let dict = self.value as? [String: Any] {
      var c = encoder.container(keyedBy: AnyCodingKey.self)
      try AnyCodable.encode(to: &c, dictionary: dict)
      return
    }
    var c = encoder.singleValueContainer()
    try AnyCodable.encode(to: &c, value: self.value)
  }

  static func decodingError(forCodingPath codingPath: [CodingKey]) -> DecodingError {
    let context = DecodingError.Context(
    codingPath: codingPath, 
    debugDescription: "Cannot decode AnyCodable")
    return DecodingError.typeMismatch(AnyCodable.self, context)
  }

  static func encodingError(forValue value: Any, codingPath: [CodingKey]) -> EncodingError {
    let context = EncodingError.Context(
    codingPath: codingPath, 
    debugDescription: "Cannot encode AnyCodable")
    return EncodingError.invalidValue(value, context)
  }

  static func decode(from c: SingleValueDecodingContainer) throws -> Any {
    if let value = try? c.decode(Bool.self) {
      return value
    }

    if let value = try? c.decode(Int.self) {
      return value
    }

    if let value = try? c.decode(UInt.self) {
      return value
    }

    if let value = try? c.decode(Int32.self) {
      return value
    }

    if let value = try? c.decode(Int64.self) {
      return value
    }

    if let value = try? c.decode(UInt32.self) {
      return value
    }

    if let value = try? c.decode(UInt64.self) {
      return value
    }

    if let value = try? c.decode(Float.self) {
      return value
    }

    if let value = try? c.decode(Double.self) {
      return value
    }

    if let value = try? c.decode(String.self) {
      return value
    }

    if c.decodeNil() {
      return AnyNull()
    }

    throw decodingError(forCodingPath: c.codingPath)
  }

  static func decode(from c: inout UnkeyedDecodingContainer) throws -> Any {
    if let value = try? c.decode(Bool.self) {
      return value
    }

    if let value = try? c.decode(Int.self) {
      return value
    }

    if let value = try? c.decode(UInt.self) {
      return value
    }

    if let value = try? c.decode(Int32.self) {
      return value
    }

    if let value = try? c.decode(Int64.self) {
      return value
    }

    if let value = try? c.decode(UInt32.self) {
      return value
    }

    if let value = try? c.decode(UInt64.self) {
      return value
    }

    if let value = try? c.decode(Float.self) {
      return value
    }

    if let value = try? c.decode(Double.self) {
      return value
    }

    if let value = try? c.decode(String.self) {
      return value
    }

    if let value = try? c.decodeNil() {
      if value {
        return AnyNull()
      }
    }

    if var c = try? c.nestedUnkeyedContainer() {
      return try decodeArray(from: &c)
    }

    if var c = try? c.nestedContainer(keyedBy: AnyCodingKey.self) {
      return try decodeDictionary(from: &c)
    }

    throw decodingError(forCodingPath: c.codingPath)
  }

  static func decode(from c: inout KeyedDecodingContainer<AnyCodingKey>, forKey key: AnyCodingKey) throws -> Any {
    if let value = try? c.decode(Bool.self, forKey: key) {
      return value
    }

    if let value = try? c.decode(Int.self, forKey: key) {
      return value
    }

    if let value = try? c.decode(UInt.self, forKey: key) {
      return value
    }

    if let value = try? c.decode(Int32.self, forKey: key) {
      return value
    }

    if let value = try? c.decode(Int64.self, forKey: key) {
      return value
    }

    if let value = try? c.decode(UInt32.self, forKey: key) {
      return value
    }

    if let value = try? c.decode(UInt64.self, forKey: key) {
      return value
    }

    if let value = try? c.decode(Float.self, forKey: key) {
      return value
    }

    if let value = try? c.decode(Double.self, forKey: key) {
      return value
    }

    if let value = try? c.decode(String.self, forKey: key) {
      return value
    }

    if let value = try? c.decodeNil(forKey: key) {
      if value {
        return AnyNull()
      }
    }

    if var c = try? c.nestedUnkeyedContainer(forKey: key) {
      return try decodeArray(from: &c)
    }

    if var c = try? c.nestedContainer(keyedBy: AnyCodingKey.self, forKey: key) {
      return try decodeDictionary(from: &c)
    }

    throw decodingError(forCodingPath: c.codingPath)
  }

  static func decodeArray(from c: inout UnkeyedDecodingContainer) throws -> [Any] {
    var array: [Any] = []

    while !c.isAtEnd {
      array.append(try decode(from: &c))
    }

    return array
  }

  static func decodeDictionary(from c: inout KeyedDecodingContainer<AnyCodingKey>) throws -> [String: Any] {
    var dict = [String: Any]()

    for key in c.allKeys {
      dict[key.stringValue] = try decode(from: &c, forKey: key)
    }

    return dict
  }

  static func encode(to c: inout SingleValueEncodingContainer, value: Any) throws {
    switch value {
    case let value as Bool:
      try c.encode(value)
    case let value as Int:
      try c.encode(value)
    case let value as UInt:
      try c.encode(value)
    case let value as Int32:
      try c.encode(value)
    case let value as Int64:
      try c.encode(value)
    case let value as UInt32:
      try c.encode(value)
    case let value as UInt64:
      try c.encode(value)
    case let value as Float:
      try c.encode(value)
    case let value as Double:
      try c.encode(value)
    case let value as String:
      try c.encode(value)
    case _ as AnyNull:
      try c.encodeNil()
    default:
      throw encodingError(forValue: value, codingPath: c.codingPath)
    }
  }

  static func encode(to c: inout UnkeyedEncodingContainer, array: [Any]) throws {
    for value in array {
      switch value {
      case let value as Bool:
        try c.encode(value)
      case let value as Int:
        try c.encode(value)
      case let value as UInt:
        try c.encode(value)
      case let value as Int32:
        try c.encode(value)
      case let value as Int64:
        try c.encode(value)
      case let value as UInt32:
        try c.encode(value)
      case let value as UInt64:
        try c.encode(value)
      case let value as Float:
        try c.encode(value)
      case let value as Double:
        try c.encode(value)
      case let value as String:
        try c.encode(value)
      case let value as [Any]:
        var c = c.nestedUnkeyedContainer()
        try encode(to: &c, array: value)
      case let value as [String: Any]:
        var c = c.nestedContainer(keyedBy: AnyCodingKey.self)
        try encode(to: &c, dictionary: value)
      case _ as AnyNull:
        try c.encodeNil()
      default:
        throw encodingError(forValue: value, codingPath: c.codingPath)
      }
    }
  }

  static func encode(to c: inout KeyedEncodingContainer<AnyCodingKey>, dictionary: [String: Any]) throws {
    for (key, value) in dictionary {
      let key = AnyCodingKey(stringValue: key)!
      switch value {
      case let value as Bool:
        try c.encode(value, forKey: key)
      case let value as Int:
        try c.encode(value, forKey: key)
      case let value as UInt:
        try c.encode(value, forKey: key)
      case let value as Int32:
        try c.encode(value, forKey: key)
      case let value as Int64:
        try c.encode(value, forKey: key)
      case let value as UInt32:
        try c.encode(value, forKey: key)
      case let value as UInt64:
        try c.encode(value, forKey: key)
      case let value as Float:
        try c.encode(value, forKey: key)
      case let value as Double:
        try c.encode(value, forKey: key)
      case let value as String:
        try c.encode(value, forKey: key)
      case let value as [Any]:
        var c = c.nestedUnkeyedContainer(forKey: key)
        try encode(to: &c, array: value)
      case let value as [String: Any]:
        var c = c.nestedContainer(keyedBy: AnyCodingKey.self, forKey: key)
        try encode(to: &c, dictionary: value)
      case _ as AnyNull:
        try c.encodeNil(forKey: key)
      default:
        throw encodingError(forValue: value, codingPath: c.codingPath)
      }
    }
  }
}
class AnyCodingKey: CodingKey {
  let key: String

  required init?(intValue: Int) {
    return nil
  }

  required init?(stringValue: String) {
    key = stringValue
  }

  var intValue: Int? {
    return nil
  }

  var stringValue: String {
    return key
  }
}
class AnyNull: Codable {
  public init() {
  }

  public required init(from decoder: Decoder) throws {
    let c = try decoder.singleValueContainer()
    if !c.decodeNil() {
      throw DecodingError.typeMismatch(AnyNull.self, DecodingError.Context(codingPath: decoder.codingPath, debugDescription: "Wrong type for AnyNull"))
    }
  }

  public func encode(to encoder: Encoder) throws {
    var c = encoder.singleValueContainer()
    try c.encodeNil()
  }
}
