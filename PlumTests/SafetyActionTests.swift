import XCTest
@testable import Plum

/// Un service de découverte qui refuse tout ce qui touche à la sécurité.
///
/// C'est le cas qui compte : le réseau tombe au moment précis où quelqu'un
/// bloque la personne qui le harcèle. Le reste du protocole répond
/// normalement, pour qu'un test ne puisse pas passer par accident sur un
/// chargement raté.
private struct RefusingDiscoveryService: DiscoveryServicing {
    struct Down: Error {}

    func selection() async throws -> DailySelection {
        DailySelection(
            items: SampleData.selection,
            refreshesAt: .now.addingTimeInterval(3_600),
            size: SampleData.selection.count
        )
    }

    func write(profileId: UUID, body: String) async throws -> Message {
        Message(id: UUID(), conversationId: UUID(), senderId: UUID(), body: body, sentAt: .now)
    }

    func pass(profileId: UUID) async throws {}

    func report(profileId: UUID, reason: String) async throws { throw Down() }

    func block(profileId: UUID) async throws { throw Down() }
}

@MainActor
final class SelectionSafetyActionTests: XCTestCase {
    /// Le silence d'avant : la carte partait, l'appel échouait dans un `try?`,
    /// et la personne revenait au chargement suivant sans explication. Sur un
    /// geste de sécurité, laisser quelqu'un se croire protégé est pire que de
    /// lui dire que ça a raté.
    func testAFailedBlockSaysSoAndPutsTheCardBack() async {
        let viewModel = DiscoveryViewModel(discovery: RefusingDiscoveryService())
        await viewModel.load()

        guard let profile = viewModel.profiles.first else {
            return XCTFail("La sélection de démonstration doit avoir des profils")
        }
        let before = viewModel.profiles.map(\.id)

        await viewModel.block(profile)

        XCTAssertNotNil(viewModel.safetyFailure, "Un blocage raté doit se voir")
        XCTAssertEqual(
            viewModel.profiles.map(\.id),
            before,
            "La carte doit revenir à sa place : le blocage n'a pas eu lieu"
        )
    }

    func testAFailedReportSaysSoToo() async {
        let viewModel = DiscoveryViewModel(discovery: RefusingDiscoveryService())
        await viewModel.load()

        guard let profile = viewModel.profiles.first else {
            return XCTFail("La sélection de démonstration doit avoir des profils")
        }

        await viewModel.report(profile, reason: "Harcèlement")

        XCTAssertNotNil(viewModel.safetyFailure)
        XCTAssertTrue(viewModel.profiles.contains { $0.id == profile.id })
    }

    /// Et quand ça marche, rien ne s'affiche et la carte reste partie.
    func testASuccessfulBlockIsSilentAndKeepsTheCardGone() async {
        let viewModel = DiscoveryViewModel(discovery: DemoDiscoveryService())
        await viewModel.load()

        guard let profile = viewModel.profiles.first else {
            return XCTFail("La sélection de démonstration doit avoir des profils")
        }

        await viewModel.block(profile)

        XCTAssertNil(viewModel.safetyFailure)
        XCTAssertFalse(viewModel.profiles.contains { $0.id == profile.id })
    }

    func testTheNoticeCanBeDismissed() async {
        let viewModel = DiscoveryViewModel(discovery: RefusingDiscoveryService())
        await viewModel.load()
        guard let profile = viewModel.profiles.first else { return XCTFail("sélection vide") }

        await viewModel.block(profile)
        XCTAssertNotNil(viewModel.safetyFailure)

        viewModel.dismissSafetyFailure()
        XCTAssertNil(viewModel.safetyFailure)
    }
}

@MainActor
final class ConversationSafetyActionTests: XCTestCase {
    private func makeViewModel(discovery: any DiscoveryServicing) -> ChatViewModel {
        ChatViewModel(
            conversation: SampleData.conversations[0],
            chat: DemoChatService(),
            discovery: discovery,
            currentUserId: SampleData.currentUser.id
        )
    }

    /// C'est ce booléen qui décide si l'écran se referme. Avant, l'appel était
    /// un `try?` suivi d'une sortie inconditionnelle : la conversation
    /// disparaissait de la liste alors que le fil restait ouvert côté serveur,
    /// et le message suivant arrivait sans qu'on comprenne d'où.
    func testAFailedBlockDoesNotReportSuccess() async {
        let viewModel = makeViewModel(discovery: RefusingDiscoveryService())

        let done = await viewModel.block()

        XCTAssertFalse(done, "L'écran ne doit pas se fermer sur un blocage raté")
        XCTAssertNotNil(viewModel.safetyFailure)
    }

    func testAFailedReportDoesNotReportSuccess() async {
        let viewModel = makeViewModel(discovery: RefusingDiscoveryService())

        let done = await viewModel.report(reason: "Harcèlement")

        XCTAssertFalse(done)
        XCTAssertNotNil(viewModel.safetyFailure)
    }

    func testABlockThatLandsReportsSuccessAndSaysNothing() async {
        let viewModel = makeViewModel(discovery: DemoDiscoveryService())

        let done = await viewModel.block()

        XCTAssertTrue(done)
        XCTAssertNil(viewModel.safetyFailure)
    }
}

/// Un service de messagerie dont le fil a disparu côté serveur.
private struct VanishedThreadChatService: ChatServicing {
    func conversations(cursor: String?) async throws -> Page<Conversation> {
        Page(items: [], nextCursor: nil)
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

/// Un service qui échoue sur le réseau, sans que le fil ait disparu.
private struct OfflineChatService: ChatServicing {
    func conversations(cursor: String?) async throws -> Page<Conversation> {
        Page(items: [], nextCursor: nil)
    }

    func messages(conversationId: UUID, before: String?) async throws -> Page<Message> {
        Page(items: [], nextCursor: nil)
    }

    func send(conversationId: UUID, clientId: UUID, body: String) async throws -> Message {
        throw APIError.offline
    }

    func markRead(conversationId: UUID) async throws {}
    func notifyTyping(conversationId: UUID) async {}
    func eventStream() async throws -> AsyncStream<ChatEvent> {
        AsyncStream { $0.finish() }
    }
    func closeStream() async {}
}

@MainActor
final class VanishedThreadTests: XCTestCase {
    private func makeViewModel(chat: any ChatServicing) -> ChatViewModel {
        ChatViewModel(
            conversation: SampleData.conversations[0],
            chat: chat,
            discovery: DemoDiscoveryService(),
            currentUserId: SampleData.currentUser.id
        )
    }

    /// Depuis qu'un blocage supprime la conversation, l'autre appareil peut
    /// avoir l'écran encore ouvert. Sans ce garde-fou, chaque message échouait
    /// derrière un bouton « réessayer » qui ne pouvait plus aboutir.
    func testAThreadTheServerNoLongerKnowsStopsAcceptingMessages() async {
        let viewModel = makeViewModel(chat: VanishedThreadChatService())

        viewModel.draft = "tu es là ?"
        await viewModel.send()

        XCTAssertTrue(viewModel.isClosed, "Le fil doit se déclarer fermé")
        XCTAssertEqual(viewModel.items.last?.deliveryState, .failed)

        // Et le renvoi ne repart pas : ce serait une boucle sans fin.
        guard let failed = viewModel.items.last else { return XCTFail("une bulle attendue") }
        await viewModel.retry(failed)
        XCTAssertEqual(viewModel.items.count, 1, "Le renvoi ne doit rien retenter")
    }

    /// Un réseau coupé n'est pas un fil disparu : le renvoi doit rester
    /// possible, sinon une coupure de métro fermerait la conversation.
    func testAnOfflineFailureLeavesTheThreadOpen() async {
        let viewModel = makeViewModel(chat: OfflineChatService())

        viewModel.draft = "tu es là ?"
        await viewModel.send()

        XCTAssertFalse(viewModel.isClosed)
        XCTAssertEqual(viewModel.items.last?.deliveryState, .failed)
    }
}
