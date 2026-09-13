import Foundation

struct Conversation: Identifiable, Codable, Hashable, Sendable {
    let id: UUID
    var matchId: UUID
    var participant: Profile
    var lastMessage: Message?
    var unreadCount: Int
    var updatedAt: Date

    init(
        id: UUID,
        matchId: UUID,
        participant: Profile,
        lastMessage: Message? = nil,
        unreadCount: Int = 0,
        updatedAt: Date
    ) {
        self.id = id
        self.matchId = matchId
        self.participant = participant
        self.lastMessage = lastMessage
        self.unreadCount = unreadCount
        self.updatedAt = updatedAt
    }

    var preview: String {
        lastMessage?.body ?? "Dites quelque chose, n'importe quoi."
    }
}
