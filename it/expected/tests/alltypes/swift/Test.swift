import Foundation

public struct Test_Entry: Codable {
  let boolean_type: Bool?
  let string_type: String?
  let datetime_type: Date?
  let unsigned_32: UInt32?
  let unsigned_64: UInt64?
  let signed_32: Int32?
  let signed_64: Int64?
  let float_type: Float?
  let double_type: Double?
  let bytes_type: Data?
  let any_type: AnyCodable?
  let array_type: [Test_Entry]?
  let map_type: [String: Test_Entry]?

  enum CodingKeys: String, CodingKey {
    case boolean_type = "boolean_type"
    case string_type = "string_type"
    case datetime_type = "datetime_type"
    case unsigned_32 = "unsigned_32"
    case unsigned_64 = "unsigned_64"
    case signed_32 = "signed_32"
    case signed_64 = "signed_64"
    case float_type = "float_type"
    case double_type = "double_type"
    case bytes_type = "bytes_type"
    case any_type = "any_type"
    case array_type = "array_type"
    case map_type = "map_type"
  }
}
