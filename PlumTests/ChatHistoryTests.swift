import XCTest
@testable import Plum

/// Serves the thread in two pages, newest first, the way the API does.
private actor PagedChatService: ChatServicing {
    private let conversationId: UUID
    private(set) var requestedCursors: [String?] = []

    init(conversationId: UUID) {
        self.conversationId = conversationId
    }

    private func message(_ suffix: String, minutesAgo: Int) -> Message {
        Message(
            id: UUID(uuidString: "00000000-0000-0000-0000-0000000000\(suffix)")!,
            conversationId: conversationId,
            senderId: SampleData.currentUser.id,
            body: "message \(suffix)",
            sentAt: Date.now.addingTimeInterval(TimeInterval(-minutesAgo * 60))
        )
    }

    func conversations(cursor: String?) async throws -> Page<Conversation> {
        Page(items: [], nextCursor: nil)
    }

    func messages(conversationId: UUID, before: String?) async throws -> Page<Message> {
        requestedCursors.append(before)
        if before == nil {
            // The newest page, plus a cursor pointing further back.
            return Page(
                items: [message("A3", minutesAgo: 3), message("A4", minutesAgo: 2)],
                nextCursor: "older"
            )
        }
        return Page(
            items: [message("A1", minutesAgo: 9), message("A2", minutesAgo: 8)],
            nextCursor: nil
        )
    }

    func send(conversationId: UUID, clientId: UUID, body: String) async throws -> Message {
        throw APIError.notFound
    }

    func markRead(conversationId: UUID) async throws {}

    func notifyTyping(conversationId: UUID) async {}

    func eventStream() async throws -> AsyncStream<ChatEvent> {
        AsyncStream { $0.finish() }
    }

    func closeStream() async {}

    func cursors() -> [String?] { requestedCursors }
}

@MainActor
final class ChatHistoryTests: XCTestCase {
    private let conversation = SampleData.conversations[0]

    private func makeViewModel() -> (ChatViewModel, PagedChatService) {
        let service = PagedChatService(conversationId: conversation.id)
        let viewModel = ChatViewModel(
            conversation: conversation,
            chat: service,
            discovery: DemoDiscoveryService(),
            currentUserId: SampleData.currentUser.id
        )
        return (viewModel, service)
    }

    func testOlderMessagesAreInsertedAboveAndStayInOrder() async {
        let (viewModel, _) = makeViewModel()
        await viewModel.start()
        viewModel.stop()
        XCTAssertEqual(viewModel.items.count, 2)

        await viewModel.loadOlderMessages()

        XCTAssertEqual(viewModel.items.count, 4)
        let dates = viewModel.items.map(\.message.sentAt)
        XCTAssertEqual(dates, dates.sorted(), "L'historique doit rester chronologique")
        XCTAssertEqual(viewModel.items.first?.message.body, "message A1")
    }

    /// The returned anchor is what lets the view put the reader back where
    /// they were; without it the thread jumps by a whole page.
    func testLoadingOlderMessagesReturnsThePreviousTopMessage() async {
        let (viewModel, _) = makeViewModel()
        await viewModel.start()
        viewModel.stop()
        let topBefore = viewModel.items.first?.id

        let anchor = await viewModel.loadOlderMessages()

        XCTAssertEqual(anchor, topBefore)
    }

    func testThereIsNothingToAnchorToWhenTheHistoryIsExhausted() async {
        let (viewModel, _) = makeViewModel()
        await viewModel.start()
        viewModel.stop()
        await viewModel.loadOlderMessages()

        let anchor = await viewModel.loadOlderMessages()

        XCTAssertNil(anchor, "Plus de curseur : rien à charger, donc rien à ancrer")
        XCTAssertEqual(viewModel.items.count, 4)
    }

    /// The sentinel fires on every layout pass; two overlapping loads would
    /// fetch the same page twice.
    func testOverlappingLoadsFetchThePageOnce() async {
        let (viewModel, service) = makeViewModel()
        await viewModel.start()
        viewModel.stop()

        async let first = viewModel.loadOlderMessages()
        async let second = viewModel.loadOlderMessages()
        _ = await (first, second)

        XCTAssertEqual(viewModel.items.count, 4)
        let cursors = await service.cursors()
        XCTAssertEqual(cursors, [nil, "older"])
    }
}
