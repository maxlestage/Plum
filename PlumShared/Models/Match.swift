import Foundation

struct Match: Identifiable, Codable, Hashable, Sendable {
    let id: UUID
    var profile: Profile
    var matchedAt: Date
    var conversationId: UUID?

    init(
        id: UUID,
        profile: Profile,
        matchedAt: Date,
        conversationId: UUID? = nil
    ) {
        self.id = id
        self.profile = profile
        self.matchedAt = matchedAt
        self.conversationId = conversationId
    }
}
