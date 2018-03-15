import Foundation
import Models

let decoder = JSONDecoder()
decoder.dateDecodingStrategy = .iso8601

let encoder = JSONEncoder()
encoder.dateEncodingStrategy = .iso8601

while let line = readLine() {
    let json = line.data(using: String.Encoding.utf8)!
    let entry = try decoder.decode(Test_Entry.self, from: json)
    let data = try encoder.encode(entry)
    let out = String(data: data, encoding: String.Encoding.utf8) as String!
    print(out!)
}

/*
while let line = readLine() {
    let json = line.data(using: String.Encoding.utf8)!
    let object = try? JSONSerialization.jsonObject(with: json)
    let entry = try Test_Entry.decode(json: object as! [String: Any])
    let out_data = try JSONSerialization.data(withJSONObject: entry.encode())
    let out = String(data: out_data, encoding: String.Encoding.utf8) as String!
    print(out!)
}
*/
