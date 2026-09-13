import Foundation

/// The token pair issued by the API. The access token is short-lived; the
/// refresh token is what keeps someone signed in between launches.
struct AuthTokens: Codable, Hashable, Sendable {
    var accessToken: String
    var refreshToken: String
    var expiresAt: Date

    init(accessToken: String, refreshToken: String, expiresAt: Date) {
        self.accessToken = accessToken
        self.refreshToken = refreshToken
        self.expiresAt = expiresAt
    }

    /// Refresh a little early so a request never races the expiry.
    func isExpired(asOf date: Date = .now, leeway: TimeInterval = 30) -> Bool {
        expiresAt.addingTimeInterval(-leeway) <= date
    }
}

struct AuthenticatedSession: Codable, Hashable, Sendable {
    var user: User
    var tokens: AuthTokens
}

struct SignInRequest: Codable, Sendable {
    let email: String
    let password: String
}

struct SignUpRequest: Codable, Sendable {
    let email: String
    let password: String
    let displayName: String
    let birthDate: Date
    let gender: Gender
}

struct RefreshRequest: Codable, Sendable {
    let refreshToken: String
}
