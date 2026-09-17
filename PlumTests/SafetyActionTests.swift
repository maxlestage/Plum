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

    func blocks() async throws -> [BlockedPerson] { [] }

    func unblock(profileId: UUID) async throws { throw Down() }
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

/// Un service qui tient une liste de blocages, et qui peut refuser de la
/// défaire.
private actor ListingDiscoveryService: DiscoveryServicing {
    struct Down: Error {}

    private var held: [BlockedPerson]
    private let refuses: Bool

    init(held: [BlockedPerson], refuses: Bool = false) {
        self.held = held
        self.refuses = refuses
    }

    func selection() async throws -> DailySelection {
        DailySelection(items: [], refreshesAt: .now, size: 0)
    }

    func write(profileId: UUID, body: String) async throws -> Message {
        Message(id: UUID(), conversationId: UUID(), senderId: UUID(), body: body, sentAt: .now)
    }

    func pass(profileId: UUID) async throws {}
    func report(profileId: UUID, reason: String) async throws {}
    func block(profileId: UUID) async throws {}

    func blocks() async throws -> [BlockedPerson] { held }

    func unblock(profileId: UUID) async throws {
        if refuses { throw Down() }
        held.removeAll { $0.id == profileId }
    }
}

@MainActor
final class BlockedListTests: XCTestCase {
    private func someone(_ name: String?) -> BlockedPerson {
        BlockedPerson(id: UUID(), displayName: name, blockedAt: .now)
    }

    func testTheListShowsWhoWasBlocked() async {
        let person = someone("Camille")
        let viewModel = BlockedListViewModel(
            discovery: ListingDiscoveryService(held: [person])
        )

        await viewModel.load()

        XCTAssertEqual(viewModel.people.map(\.id), [person.id])
        XCTAssertEqual(viewModel.people.first?.label, "Camille")
    }

    /// Le compte a pu partir depuis. La ligne reste — sinon on ne pourrait
    /// plus la retirer — et elle doit dire quelque chose.
    func testAClosedAccountStillReadsAsSomething() async {
        let viewModel = BlockedListViewModel(
            discovery: ListingDiscoveryService(held: [someone(nil)])
        )

        await viewModel.load()

        XCTAssertEqual(viewModel.people.first?.label, "Compte supprimé")
    }

    /// Le silence serait pire qu'un échec visible : quelqu'un croirait avoir
    /// débloqué une personne qui l'est toujours, et l'inverse — une ligne qui
    /// disparaît de l'écran sans disparaître du serveur — est ce que le
    /// `try?` d'avant produisait ailleurs dans l'application.
    func testAFailedReleaseKeepsTheRowAndSaysSo() async {
        let person = someone("Camille")
        let viewModel = BlockedListViewModel(
            discovery: ListingDiscoveryService(held: [person], refuses: true)
        )
        await viewModel.load()

        await viewModel.unblock(person)

        XCTAssertNotNil(viewModel.failure, "Un déblocage raté doit se voir")
        XCTAssertEqual(
            viewModel.people.map(\.id),
            [person.id],
            "La ligne doit rester : la personne est toujours bloquée"
        )
    }

    func testAReleaseThatLandsRemovesTheRowSilently() async {
        let person = someone("Camille")
        let viewModel = BlockedListViewModel(
            discovery: ListingDiscoveryService(held: [person])
        )
        await viewModel.load()

        await viewModel.unblock(person)

        XCTAssertNil(viewModel.failure)
        XCTAssertTrue(viewModel.people.isEmpty)
    }
}

/// La phrase qui compte des gens.
///
/// Le nombre du jour vient du serveur et n'est plus toujours le même, donc
/// aucune formulation ne peut l'écrire en toutes lettres — et le singulier
/// arrive vraiment, les jours où le voisinage n'avait qu'une personne à
/// proposer.
final class SelectionCountLineTests: XCTestCase {
    func testAFullDayNamesTheNumberItActuallyHas() {
        XCTAssertEqual(
            SelectionView.countLine(remaining: 4, size: 4),
            "Vos 4 profils du jour."
        )
        XCTAssertEqual(
            SelectionView.countLine(remaining: 2, size: 2),
            "Vos 2 profils du jour."
        )
    }

    func testOneProfileDoesNotReadAsAPluralMistake() {
        XCTAssertEqual(
            SelectionView.countLine(remaining: 1, size: 1),
            "Un profil pour aujourd'hui."
        )
    }

    func testAPartlyDecidedDayCountsBothHalves() {
        XCTAssertEqual(
            SelectionView.countLine(remaining: 1, size: 5),
            "Il en reste 1 sur 5."
        )
    }

    /// Aucune des trois phrases ne doit annoncer un nombre écrit en toutes
    /// lettres : c'est exactement ce que disait l'écran d'avant, et c'était
    /// faux dès que le compte a cessé d'être fixe.
    func testNoLineSpellsOutAFixedNumber() {
        for phrase in [
            SelectionView.countLine(remaining: 3, size: 3),
            SelectionView.countLine(remaining: 1, size: 1),
            SelectionView.countLine(remaining: 2, size: 5),
        ] {
            XCTAssertFalse(
                phrase.lowercased().contains("trois"),
                "« \(phrase) » promet un nombre qui n'est plus garanti"
            )
        }
    }
}
