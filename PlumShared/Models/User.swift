import Foundation

/// An account as returned by the API. Account-level data only: everything
/// other users can see lives on ``Profile``.
struct User: Identifiable, Codable, Hashable, Sendable {
    let id: UUID
    var email: String
    var createdAt: Date
    var profileCompleted: Bool

    init(id: UUID, email: String, createdAt: Date, profileCompleted: Bool) {
        self.id = id
        self.email = email
        self.createdAt = createdAt
        self.profileCompleted = profileCompleted
    }
}
