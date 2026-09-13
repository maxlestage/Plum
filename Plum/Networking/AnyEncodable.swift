import Foundation

/// Lets an ``Endpoint`` carry a body without being generic over it.
struct AnyEncodable: Encodable, @unchecked Sendable {
    private let encodeValue: (Encoder) throws -> Void

    init<Value: Encodable>(_ value: Value) {
        encodeValue = { encoder in try value.encode(to: encoder) }
    }

    func encode(to encoder: Encoder) throws {
        try encodeValue(encoder)
    }
}
