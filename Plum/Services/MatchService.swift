import Foundation

protocol MatchServicing: Sendable {
    func matches(cursor: String?) async throws -> Page<Match>
    func unmatch(matchId: UUID) async throws
    /// Opens (or reuses) the conversation attached to a match.
    func conversation(forMatch matchId: UUID) async throws -> Conversation
}

struct MatchService: MatchServicing {
    let client: any APIClientProtocol

    init(client: any APIClientProtocol) {
        self.client = client
    }

    func matches(cursor: String?) async throws -> Page<Match> {
        var query: [URLQueryItem] = []
        if let cursor {
            query.append(URLQueryItem(name: "cursor", value: cursor))
        }
        return try await client.send(.get("matches", query: query), as: Page<Match>.self)
    }

    func unmatch(matchId: UUID) async throws {
        try await client.send(.delete("matches/\(matchId.uuidString)"))
    }

    func conversation(forMatch matchId: UUID) async throws -> Conversation {
        try await client.send(
            .post("matches/\(matchId.uuidString)/conversation"),
            as: Conversation.self
        )
    }
}
