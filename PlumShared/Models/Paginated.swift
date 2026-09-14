import Foundation

/// Cursor pagination, matching the shape the API uses for decks, matches and
/// message history.
struct Page<Item: Codable & Sendable>: Codable, Sendable {
    var items: [Item]
    var nextCursor: String?

    init(items: [Item], nextCursor: String? = nil) {
        self.items = items
        self.nextCursor = nextCursor
    }

    var hasMore: Bool { nextCursor != nil }
}
