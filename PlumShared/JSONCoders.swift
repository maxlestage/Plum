import Foundation

/// The API speaks snake_case and RFC 3339; Postgres hands back timestamps with
/// or without fractional seconds depending on the column, so both are parsed.
enum PlumDateFormat {
    private static let withFractionalSeconds: ISO8601DateFormatter = {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return formatter
    }()

    private static let withoutFractionalSeconds: ISO8601DateFormatter = {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime]
        return formatter
    }()

    static func date(from string: String) -> Date? {
        withFractionalSeconds.date(from: string)
            ?? withoutFractionalSeconds.date(from: string)
    }

    static func string(from date: Date) -> String {
        withFractionalSeconds.string(from: date)
    }
}

/// The only coders the app uses, so the wire convention lives in one place.
extension JSONDecoder {
    static var plum: JSONDecoder {
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        decoder.dateDecodingStrategy = .custom { decoder in
            let container = try decoder.singleValueContainer()
            let raw = try container.decode(String.self)
            guard let date = PlumDateFormat.date(from: raw) else {
                throw DecodingError.dataCorruptedError(
                    in: container,
                    debugDescription: "Date non conforme à ISO 8601 : \(raw)"
                )
            }
            return date
        }
        return decoder
    }
}

extension JSONEncoder {
    static var plum: JSONEncoder {
        let encoder = JSONEncoder()
        encoder.keyEncodingStrategy = .convertToSnakeCase
        encoder.dateEncodingStrategy = .custom { date, encoder in
            var container = encoder.singleValueContainer()
            try container.encode(PlumDateFormat.string(from: date))
        }
        return encoder
    }
}
