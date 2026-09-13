import Foundation
import OSLog

/// What the chat socket pushes down. The `type` discriminator matches the
/// tagged enum the Rust side serialises.
enum ChatEvent: Sendable, Equatable {
    case messageReceived(Message)
    case messageRead(messageId: UUID, readAt: Date)
    case typing(conversationId: UUID, profileId: UUID)
    case matchCreated(Match)
    case connectionChanged(isConnected: Bool)
}

extension ChatEvent: Decodable {
    private enum CodingKeys: String, CodingKey {
        case type, message, messageId, readAt, conversationId, profileId, match
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(String.self, forKey: .type)

        switch type {
        case "message":
            self = .messageReceived(try container.decode(Message.self, forKey: .message))
        case "read":
            self = .messageRead(
                messageId: try container.decode(UUID.self, forKey: .messageId),
                readAt: try container.decode(Date.self, forKey: .readAt)
            )
        case "typing":
            self = .typing(
                conversationId: try container.decode(UUID.self, forKey: .conversationId),
                profileId: try container.decode(UUID.self, forKey: .profileId)
            )
        case "match":
            self = .matchCreated(try container.decode(Match.self, forKey: .match))
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .type,
                in: container,
                debugDescription: "Évènement inconnu : \(type)"
            )
        }
    }
}

/// What the app sends up.
enum ChatCommand: Encodable, Sendable {
    case sendMessage(conversationId: UUID, clientId: UUID, body: String)
    case markRead(conversationId: UUID)
    case typing(conversationId: UUID)

    private enum CodingKeys: String, CodingKey {
        case type, conversationId, clientId, body
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case let .sendMessage(conversationId, clientId, body):
            try container.encode("send_message", forKey: .type)
            try container.encode(conversationId, forKey: .conversationId)
            try container.encode(clientId, forKey: .clientId)
            try container.encode(body, forKey: .body)
        case let .markRead(conversationId):
            try container.encode("mark_read", forKey: .type)
            try container.encode(conversationId, forKey: .conversationId)
        case let .typing(conversationId):
            try container.encode("typing", forKey: .type)
            try container.encode(conversationId, forKey: .conversationId)
        }
    }
}

/// A live connection to the chat stream, exposed as an `AsyncStream` so views
/// can `for await` it and stop simply by ending the task.
///
/// Reconnects with a capped exponential backoff: a phone that loses signal in
/// the métro should come back on its own, not sit there dead.
actor ChatSocket {
    private let configuration: APIConfiguration
    private let session: URLSession
    private let decoder = JSONDecoder.plum
    private let encoder = JSONEncoder.plum
    private let logger = Logger(subsystem: "app.plum", category: "socket")

    private var task: URLSessionWebSocketTask?
    /// Several screens listen at once — the tab badge and the open
    /// conversation, at least — so events are broadcast rather than handed to
    /// whoever subscribed last.
    private var subscribers: [UUID: AsyncStream<ChatEvent>.Continuation] = [:]
    private var receiveLoop: Task<Void, Never>?
    private var reconnectAttempt = 0
    private var accessToken: String?
    private var isStopped = true

    init(configuration: APIConfiguration, session: URLSession = .shared) {
        self.configuration = configuration
        self.session = session
    }

    /// Opens a stream of events. The socket itself is opened once and shared;
    /// it closes when the last subscriber goes away.
    func connect(accessToken: String) -> AsyncStream<ChatEvent> {
        self.accessToken = accessToken

        let id = UUID()
        let (stream, continuation) = AsyncStream<ChatEvent>.makeStream(
            bufferingPolicy: .bufferingNewest(64)
        )
        continuation.onTermination = { [weak self] _ in
            Task { await self?.removeSubscriber(id) }
        }
        subscribers[id] = continuation

        if task == nil {
            isStopped = false
            reconnectAttempt = 0
            openConnection()
        } else {
            continuation.yield(.connectionChanged(isConnected: true))
        }
        return stream
    }

    func disconnect() {
        isStopped = true
        receiveLoop?.cancel()
        receiveLoop = nil
        task?.cancel(with: .goingAway, reason: nil)
        task = nil
        for continuation in subscribers.values {
            continuation.finish()
        }
        subscribers.removeAll()
    }

    func send(_ command: ChatCommand) async throws {
        guard let task else { throw APIError.transport("Socket fermé.") }
        let data = try encoder.encode(command)
        try await task.send(.data(data))
    }

    private func removeSubscriber(_ id: UUID) {
        subscribers[id] = nil
        // Nobody is listening any more: stop holding the connection open.
        if subscribers.isEmpty {
            disconnect()
        }
    }

    private func broadcast(_ event: ChatEvent) {
        for continuation in subscribers.values {
            continuation.yield(event)
        }
    }

    // MARK: - Connection lifecycle

    private func openConnection() {
        guard !isStopped, let accessToken else { return }

        var request = URLRequest(url: configuration.webSocketURL)
        request.setValue("Bearer \(accessToken)", forHTTPHeaderField: "Authorization")

        let socket = session.webSocketTask(with: request)
        task = socket
        socket.resume()
        broadcast(.connectionChanged(isConnected: true))

        receiveLoop = Task { [weak self] in
            await self?.receiveMessages()
        }
    }

    private func receiveMessages() async {
        while !isStopped, let socket = task {
            do {
                let message = try await socket.receive()
                reconnectAttempt = 0
                handle(message)
            } catch {
                guard !isStopped else { return }
                logger.notice("Socket interrompu : \(error.localizedDescription, privacy: .public)")
                broadcast(.connectionChanged(isConnected: false))
                await scheduleReconnect()
                return
            }
        }
    }

    private func handle(_ message: URLSessionWebSocketTask.Message) {
        let data: Data?
        switch message {
        case let .data(payload):
            data = payload
        case let .string(text):
            data = Data(text.utf8)
        @unknown default:
            data = nil
        }

        guard let data else { return }
        do {
            broadcast(try decoder.decode(ChatEvent.self, from: data))
        } catch {
            logger.error("Évènement illisible : \(error.localizedDescription, privacy: .public)")
        }
    }

    private func scheduleReconnect() async {
        guard !isStopped, !subscribers.isEmpty else { return }
        reconnectAttempt += 1
        let delay = Self.backoffDelay(forAttempt: reconnectAttempt)
        try? await Task.sleep(for: .seconds(delay))
        guard !isStopped else { return }
        openConnection()
    }

    /// 1s, 2s, 4s… capped at 30s, so a long outage does not turn into a
    /// battery-draining retry loop.
    static func backoffDelay(forAttempt attempt: Int) -> Double {
        min(30, pow(2, Double(max(0, attempt - 1))))
    }
}
