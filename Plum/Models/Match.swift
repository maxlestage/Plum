import Foundation

struct Match: Identifiable, Codable, Hashable, Sendable {
    let id: UUID
    var profile: Profile
    var matchedAt: Date
    var conversationId: UUID?
    /// Nobody has said anything yet — worth nudging in the UI.
    var hasUnreadMessages: Bool

    init(
        id: UUID,
        profile: Profile,
        matchedAt: Date,
        conversationId: UUID? = nil,
        hasUnreadMessages: Bool = false
    ) {
        self.id = id
        self.profile = profile
        self.matchedAt = matchedAt
        self.conversationId = conversationId
        self.hasUnreadMessages = hasUnreadMessages
    }
}
