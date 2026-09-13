import Foundation

protocol ChatServicing: Sendable {
    func conversations(cursor: String?) async throws -> Page<Conversation>
    func messages(conversationId: UUID, before: String?) async throws -> Page<Message>
    func send(conversationId: UUID, clientId: UUID, body: String) async throws -> Message
    func markRead(conversationId: UUID) async throws
    /// The live stream, already authenticated. Ends when the caller's task is
    /// cancelled or ``closeStream()`` is called.
    func eventStream() async throws -> AsyncStream<ChatEvent>
    func closeStream() async
}

struct ChatService: ChatServicing {
    let client: any APIClientProtocol
    let socket: ChatSocket

    init(client: any APIClientProtocol, socket: ChatSocket) {
        self.client = client
        self.socket = socket
    }

    func conversations(cursor: String?) async throws -> Page<Conversation> {
        var query: [URLQueryItem] = []
        if let cursor {
            query.append(URLQueryItem(name: "cursor", value: cursor))
        }
        return try await client.send(.get("conversations", query: query), as: Page<Conversation>.self)
    }

    func messages(conversationId: UUID, before: String?) async throws -> Page<Message> {
        var query = [URLQueryItem(name: "limit", value: "50")]
        if let before {
            query.append(URLQueryItem(name: "before", value: before))
        }
        return try await client.send(
            .get("conversations/\(conversationId.uuidString)/messages", query: query),
            as: Page<Message>.self
        )
    }

    func send(conversationId: UUID, clientId: UUID, body: String) async throws -> Message {
        struct Payload: Codable, Sendable {
            let clientId: UUID
            let body: String
        }
        let endpoint = Endpoint.post(
            "conversations/\(conversationId.uuidString)/messages",
            body: Payload(clientId: clientId, body: body)
        )
        return try await client.send(endpoint, as: Message.self)
    }

    func markRead(conversationId: UUID) async throws {
        try await client.send(.post("conversations/\(conversationId.uuidString)/read"))
    }

    func eventStream() async throws -> AsyncStream<ChatEvent> {
        guard let tokens = await client.currentTokens() else {
            throw APIError.unauthorized
        }
        return await socket.connect(accessToken: tokens.accessToken)
    }

    func closeStream() async {
        await socket.disconnect()
    }
}
