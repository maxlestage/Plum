import Foundation

protocol DiscoveryServicing: Sendable {
    /// Les profils du jour. Stable jusqu'à minuit : la même sélection le matin
    /// et le soir.
    func selection() async throws -> DailySelection
    /// Écrire, et ouvrir le fil du même geste. Rend le message écrit, qui
    /// porte l'identifiant de la conversation.
    func write(profileId: UUID, body: String) async throws -> Message
    /// Laisser passer. C'est une décision, et elle est définitive.
    func pass(profileId: UUID) async throws
    func report(profileId: UUID, reason: String) async throws
    func block(profileId: UUID) async throws
}

struct DiscoveryService: DiscoveryServicing {
    let client: any APIClientProtocol

    init(client: any APIClientProtocol) {
        self.client = client
    }

    func selection() async throws -> DailySelection {
        try await client.send(.get("discovery/selection"), as: DailySelection.self)
    }

    func write(profileId: UUID, body: String) async throws -> Message {
        try await client.send(
            .post("profiles/\(profileId.uuidString)/write", body: WriteFirstRequest(body: body)),
            as: Message.self
        )
    }

    func pass(profileId: UUID) async throws {
        try await client.send(.post("profiles/\(profileId.uuidString)/pass"))
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
