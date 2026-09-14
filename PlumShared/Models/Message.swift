import Foundation

struct Message: Identifiable, Codable, Hashable, Sendable {
    let id: UUID
    let conversationId: UUID
    let senderId: UUID
    var body: String
    var sentAt: Date
    var readAt: Date?

    init(
        id: UUID,
        conversationId: UUID,
        senderId: UUID,
        body: String,
        sentAt: Date,
        readAt: Date? = nil
    ) {
        self.id = id
        self.conversationId = conversationId
        self.senderId = senderId
        self.body = body
        self.sentAt = sentAt
        self.readAt = readAt
    }
}

/// Wraps a message with the state the UI needs but the server never sends:
/// a locally-echoed message is on screen before it exists server-side.
struct ChatItem: Identifiable, Hashable, Sendable {
    enum DeliveryState: Hashable, Sendable {
        case sending
        case sent
        case failed
    }

    var message: Message
    var deliveryState: DeliveryState

    var id: UUID { message.id }

    init(message: Message, deliveryState: DeliveryState = .sent) {
        self.message = message
        self.deliveryState = deliveryState
    }
}
