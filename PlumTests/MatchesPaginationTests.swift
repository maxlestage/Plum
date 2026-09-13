import XCTest
@testable import Plum

/// Hands out two pages, then stops — enough to prove the list actually asks
/// for the second one.
private actor PagingMatchService: MatchServicing {
    private let pages: [Page<Match>]
    private(set) var requestedCursors: [String?] = []

    init(pages: [Page<Match>]) {
        self.pages = pages
    }

    func matches(cursor: String?) async throws -> Page<Match> {
        requestedCursors.append(cursor)
        if cursor == nil { return pages[0] }
        let index = Int(cursor ?? "") ?? 0
        return index < pages.count ? pages[index] : Page(items: [], nextCursor: nil)
    }

    func unmatch(matchId: UUID) async throws {}

    func conversation(forMatch matchId: UUID) async throws -> Conversation {
        throw APIError.notFound
    }

    func cursors() -> [String?] { requestedCursors }
}

private actor SinglePageChatService: ChatServicing {
    func conversations(cursor: String?) async throws -> Page<Conversation> {
        Page(items: SampleData.conversations, nextCursor: nil)
    }

    func messages(conversationId: UUID, before: String?) async throws -> Page<Message> {
        Page(items: [], nextCursor: nil)
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
}

@MainActor
final class MatchesPaginationTests: XCTestCase {
    private func match(_ suffix: String, daysAgo: Int) -> Match {
        Match(
            id: UUID(uuidString: "00000000-0000-0000-0000-0000000000\(suffix)")!,
            profile: SampleData.deck[0],
            matchedAt: Date.now.addingTimeInterval(TimeInterval(-daysAgo * 86_400))
        )
    }

    private func makeViewModel() -> (MatchesViewModel, PagingMatchService) {
        let service = PagingMatchService(pages: [
            Page(items: [match("F1", daysAgo: 1), match("F2", daysAgo: 2)], nextCursor: "1"),
            Page(items: [match("F3", daysAgo: 3)], nextCursor: nil)
        ])
        let viewModel = MatchesViewModel(matchService: service, chatService: SinglePageChatService())
        return (viewModel, service)
    }

    func testFirstLoadKeepsTheCursorForTheNextPage() async {
        let (viewModel, _) = makeViewModel()

        await viewModel.load()

        XCTAssertEqual(viewModel.matches.count, 2)
        XCTAssertTrue(viewModel.hasMoreToLoad, "Le curseur de la première page doit être retenu")
    }

    func testReachingTheEndLoadsTheNextPage() async {
        let (viewModel, service) = makeViewModel()
        await viewModel.load()

        await viewModel.loadNextPage()

        XCTAssertEqual(viewModel.matches.count, 3)
        XCTAssertFalse(viewModel.hasMoreToLoad, "Plus de curseur : la pagination s'arrête")
        let cursors = await service.cursors()
        XCTAssertEqual(cursors, [nil, "1"])
    }

    /// Scrolling fires `onAppear` repeatedly; the second call must not refetch
    /// the page the first one is already loading.
    func testPagingTwiceDoesNotDuplicateRows() async {
        let (viewModel, _) = makeViewModel()
        await viewModel.load()

        async let first: Void = viewModel.loadNextPage()
        async let second: Void = viewModel.loadNextPage()
        _ = await (first, second)

        let ids = viewModel.matches.map(\.id)
        XCTAssertEqual(ids.count, Set(ids).count)
        XCTAssertEqual(viewModel.matches.count, 3)
    }

    func testPagingStopsWhenThereIsNothingLeft() async {
        let (viewModel, service) = makeViewModel()
        await viewModel.load()
        await viewModel.loadNextPage()

        await viewModel.loadNextPage()

        let cursors = await service.cursors()
        XCTAssertEqual(cursors, [nil, "1"], "Sans curseur, aucune requête supplémentaire")
    }
}

@MainActor
final class MatchesRemovalTests: XCTestCase {
    private func makeViewModel() -> MatchesViewModel {
        MatchesViewModel(
            matchService: PagingMatchService(pages: [Page(items: [], nextCursor: nil)]),
            chatService: SinglePageChatService()
        )
    }

    /// Blocking someone from inside the conversation has to remove the row
    /// too, or the inbox keeps offering a thread that no longer exists.
    func testDroppingAConversationAlsoDropsItsMatch() async {
        let viewModel = makeViewModel()
        await viewModel.load()

        let conversation = SampleData.conversations[0]
        XCTAssertTrue(viewModel.conversations.contains { $0.id == conversation.id })

        viewModel.drop(conversationId: conversation.id)

        XCTAssertFalse(viewModel.conversations.contains { $0.id == conversation.id })
        XCTAssertFalse(viewModel.matches.contains { $0.id == conversation.matchId })
    }

    func testDroppingSomethingUnknownChangesNothing() async {
        let viewModel = makeViewModel()
        await viewModel.load()
        let before = viewModel.conversations.count

        viewModel.drop(conversationId: UUID())

        XCTAssertEqual(viewModel.conversations.count, before)
    }
}

@MainActor
final class MatchesLiveUpdateTests: XCTestCase {
    private func makeViewModel() -> MatchesViewModel {
        MatchesViewModel(
            matchService: PagingMatchService(pages: [Page(items: [], nextCursor: nil)]),
            chatService: SinglePageChatService()
        )
    }

    private func message(in conversation: Conversation, from senderId: UUID) -> Message {
        Message(
            id: UUID(),
            conversationId: conversation.id,
            senderId: senderId,
            body: "tout juste arrivé",
            sentAt: .now
        )
    }

    /// A message arriving while the inbox is on screen has to move its row:
    /// preview, unread count and order all come from it.
    func testAnIncomingMessageMovesItsRowToTheTop() async {
        let viewModel = makeViewModel()
        await viewModel.load()

        let older = SampleData.conversations[1]
        XCTAssertEqual(viewModel.conversations.last?.id, older.id)

        viewModel.apply(message(in: older, from: older.participant.id))

        XCTAssertEqual(viewModel.conversations.first?.id, older.id)
        XCTAssertEqual(viewModel.conversations.first?.preview, "tout juste arrivé")
    }

    func testAMessageFromThemCountsAsUnread() async {
        let viewModel = makeViewModel()
        await viewModel.load()
        let conversation = SampleData.conversations[1]

        viewModel.apply(message(in: conversation, from: conversation.participant.id))

        let updated = viewModel.conversations.first { $0.id == conversation.id }
        XCTAssertEqual(updated?.unreadCount, 1)
    }

    /// Our own message echoing back is not something to badge ourselves about.
    func testOurOwnMessageClearsTheUnreadCount() async {
        let viewModel = makeViewModel()
        await viewModel.load()
        let conversation = SampleData.conversations[0]
        XCTAssertGreaterThan(conversation.unreadCount, 0)

        viewModel.apply(message(in: conversation, from: SampleData.currentUser.id))

        let updated = viewModel.conversations.first { $0.id == conversation.id }
        XCTAssertEqual(updated?.unreadCount, 0)
    }

    func testAMessageForAnUnknownConversationIsIgnored() async {
        let viewModel = makeViewModel()
        await viewModel.load()
        let before = viewModel.conversations.map(\.id)

        let stray = Message(
            id: UUID(),
            conversationId: UUID(),
            senderId: UUID(),
            body: "pour quelqu'un d'autre",
            sentAt: .now
        )
        viewModel.apply(stray)

        XCTAssertEqual(viewModel.conversations.map(\.id), before)
    }
}
