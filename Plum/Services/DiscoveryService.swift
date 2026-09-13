import Foundation

protocol DiscoveryServicing: Sendable {
    func deck(cursor: String?, limit: Int) async throws -> Page<Profile>
    func swipe(profileId: UUID, decision: SwipeDecision) async throws -> SwipeOutcome
    /// Undoes the last pass. The one feature people actually pay for.
    func rewind() async throws -> Profile?
    func report(profileId: UUID, reason: String) async throws
    func block(profileId: UUID) async throws
}

struct DiscoveryService: DiscoveryServicing {
    let client: any APIClientProtocol

    init(client: any APIClientProtocol) {
        self.client = client
    }

    func deck(cursor: String?, limit: Int = 20) async throws -> Page<Profile> {
        var query = [URLQueryItem(name: "limit", value: String(limit))]
        if let cursor {
            query.append(URLQueryItem(name: "cursor", value: cursor))
        }
        return try await client.send(.get("discovery/deck", query: query), as: Page<Profile>.self)
    }

    func swipe(profileId: UUID, decision: SwipeDecision) async throws -> SwipeOutcome {
        let endpoint = Endpoint.post(
            "discovery/swipes",
            body: SwipeRequest(targetProfileId: profileId, decision: decision)
        )
        return try await client.send(endpoint, as: SwipeOutcome.self)
    }

    func rewind() async throws -> Profile? {
        struct RewindResponse: Decodable, Sendable { let profile: Profile? }
        let response = try await client.send(.post("discovery/rewind"), as: RewindResponse.self)
        return response.profile
    }

    func report(profileId: UUID, reason: String) async throws {
        struct Payload: Codable, Sendable { let reason: String }
        try await client.send(
            .post("profiles/\(profileId.uuidString)/report", body: Payload(reason: reason))
        )
    }

    func block(profileId: UUID) async throws {
        try await client.send(.post("profiles/\(profileId.uuidString)/block"))
    }
}
