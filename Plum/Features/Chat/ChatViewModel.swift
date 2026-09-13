import Observation
import SwiftUI

/// One conversation. Handles the optimistic send, the live socket and the
/// "someone is typing" indicator.
@MainActor
@Observable
final class ChatViewModel {
    let conversation: Conversation

    private(set) var items: [ChatItem] = []
    private(set) var state: ActivityState = .idle
    private(set) var isParticipantTyping = false
    private(set) var isConnected = true
    var draft = ""

    private var oldestCursor: String?
    private var hasMoreHistory = true
    private var isLoadingHistory = false
    private var streamTask: Task<Void, Never>?
    private var typingResetTask: Task<Void, Never>?
    private var lastTypingNotice: Date?

    private let chat: any ChatServicing
    private let currentUserId: UUID

    init(conversation: Conversation, chat: any ChatServicing, currentUserId: UUID) {
        self.conversation = conversation
        self.chat = chat
        self.currentUserId = currentUserId
    }

    var canSend: Bool {
        !draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    func isMine(_ item: ChatItem) -> Bool {
        item.message.senderId == currentUserId
    }

    /// How often we are willing to tell the other side we are typing. Every
    /// keystroke would be a packet per character.
    private static let typingNoticeInterval: TimeInterval = 3

    /// Called on every keystroke, throttled to one notice every few seconds.
    func draftChanged() async {
        guard !draft.isEmpty else { return }
        let now = Date.now
        if let last = lastTypingNotice, now.timeIntervalSince(last) < Self.typingNoticeInterval {
            return
        }
        lastTypingNotice = now
        await chat.notifyTyping(conversationId: conversation.id)
    }

    // MARK: - Lifecycle

    func start() async {
        await loadHistory()
        _ = try? await chat.markRead(conversationId: conversation.id)
        listenForEvents()
    }

    func stop() {
        streamTask?.cancel()
        streamTask = nil
        typingResetTask?.cancel()
    }

    private func loadHistory() async {
        state = .loading
        do {
            let page = try await chat.messages(conversationId: conversation.id, before: nil)
            items = page.items
                .sorted { $0.sentAt < $1.sentAt }
                .map { ChatItem(message: $0) }
            oldestCursor = page.nextCursor
            hasMoreHistory = page.hasMore
            state = .ready
        } catch {
            state = .failed(error.asAPIError)
        }
    }

    /// Pulls the previous page when the person scrolls to the top of the
    /// thread.
    ///
    /// Returns the message that was at the top before the insert. Older
    /// messages go in *above* the current position, which shoves everything
    /// down: the view uses this to scroll back to where the reader was, so a
    /// page load stops feeling like the thread jumped.
    @discardableResult
    func loadOlderMessages() async -> UUID? {
        guard hasMoreHistory, !isLoadingHistory, let oldestCursor else { return nil }
        isLoadingHistory = true
        defer { isLoadingHistory = false }

        let previousTop = items.first?.id
        do {
            let page = try await chat.messages(conversationId: conversation.id, before: oldestCursor)
            let known = Set(items.map(\.id))
            let older = page.items
                .filter { !known.contains($0.id) }
                .sorted { $0.sentAt < $1.sentAt }
                .map { ChatItem(message: $0) }
            guard !older.isEmpty else {
                hasMoreHistory = page.hasMore
                self.oldestCursor = page.nextCursor
                return nil
            }
            items.insert(contentsOf: older, at: 0)
            self.oldestCursor = page.nextCursor
            hasMoreHistory = page.hasMore
            return previousTop
        } catch {
            state = .failed(error.asAPIError)
            return nil
        }
    }

    private func listenForEvents() {
        streamTask?.cancel()
        streamTask = Task { [weak self] in
            guard let self else { return }
            do {
                let stream = try await self.chat.eventStream()
                for await event in stream {
                    self.handle(event)
                }
            } catch {
                self.markDisconnected()
            }
        }
    }

    private func markDisconnected() {
        isConnected = false
    }

    private func handle(_ event: ChatEvent) {
        switch event {
        case let .messageReceived(message):
            guard message.conversationId == conversation.id else { return }
            insert(message)
            isParticipantTyping = false
            Task { _ = try? await chat.markRead(conversationId: conversation.id) }

        case let .messageRead(messageId, readAt):
            guard let index = items.firstIndex(where: { $0.id == messageId }) else { return }
            items[index].message.readAt = readAt

        case let .typing(conversationId, profileId):
            guard conversationId == conversation.id, profileId != currentUserId else { return }
            showTypingIndicator()

        case let .connectionChanged(isConnected):
            self.isConnected = isConnected

        case .matchCreated:
            break
        }
    }

    /// The indicator hides itself: the server does not send a "stopped
    /// typing" event and we should not wait for one.
    private func showTypingIndicator() {
        isParticipantTyping = true
        typingResetTask?.cancel()
        typingResetTask = Task { [weak self] in
            try? await Task.sleep(for: .seconds(4))
            guard !Task.isCancelled else { return }
            self?.hideTypingIndicator()
        }
    }

    private func hideTypingIndicator() {
        isParticipantTyping = false
    }

    private func insert(_ message: Message) {
        // The server echoes our own message back with the client id we chose,
        // so replace the optimistic copy rather than duplicating it.
        if let index = items.firstIndex(where: { $0.id == message.id }) {
            items[index] = ChatItem(message: message, deliveryState: .sent)
        } else {
            items.append(ChatItem(message: message))
            items.sort { $0.message.sentAt < $1.message.sentAt }
        }
    }

    // MARK: - Sending

    func send() async {
        let body = draft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !body.isEmpty else { return }
        draft = ""
        await deliver(body)
    }

    /// Retries a bubble that failed, in place. It does not go back through
    /// `draft`: whatever the person has started typing since must survive.
    func retry(_ item: ChatItem) async {
        guard item.deliveryState == .failed else { return }
        items.removeAll { $0.id == item.id }
        await deliver(item.message.body)
    }

    private func deliver(_ body: String) async {
        // Optimistic: the bubble appears immediately under an id we choose,
        // which the server persists as the message id.
        let clientId = UUID()
        let pending = Message(
            id: clientId,
            conversationId: conversation.id,
            senderId: currentUserId,
            body: body,
            sentAt: .now
        )
        items.append(ChatItem(message: pending, deliveryState: .sending))
        Haptics.play(.light)

        do {
            let sent = try await chat.send(
                conversationId: conversation.id,
                clientId: clientId,
                body: body
            )
            if let index = items.firstIndex(where: { $0.id == clientId }) {
                items[index] = ChatItem(message: sent, deliveryState: .sent)
            }
            // The socket can echo the message back before the REST call
            // returns; without this the bubble appears twice, and SwiftUI
            // gets two rows sharing an id.
            removeDuplicates()
        } catch {
            if let index = items.firstIndex(where: { $0.id == clientId }) {
                items[index].deliveryState = .failed
            }
            Haptics.play(.warning)
        }
    }

    private func removeDuplicates() {
        var seen = Set<UUID>()
        items = items.filter { seen.insert($0.id).inserted }
    }
}
