import XCTest
@testable import Plum

/// ActivityKit ne fonctionne pas dans un test unitaire : il lui faut un
/// système vivant, un appareil, une autorisation. Ce qui se vérifie ici n'est
/// donc pas le dessin de la carte — c'est la logique qui décide *quand* elle
/// apparaît et quand elle s'efface. C'est elle qu'on casse en refactorant, et
/// elle seule qu'un test peut tenir.
@MainActor
final class MatchActivityTests: XCTestCase {
    /// Un premier message reçu doit poser sa carte, avec la date du serveur.
    ///
    /// Le déclencheur a déménagé : il naissait d'un double oui, et il n'y en a
    /// plus. S'il n'avait pas suivi, la carte de l'écran verrouillé ne serait
    /// simplement jamais apparue — et rien ne l'aurait dit, puisque personne
    /// ne teste l'absence d'une notification.
    func testAFirstMessageOpensItsCard() {
        let match = Match(
            id: UUID(),
            profile: SampleData.selection[0],
            matchedAt: Date(timeIntervalSince1970: 1_757_800_000),
            conversationId: UUID()
        )

        let suite = MainTabView.reaction(
            to: .matchCreated(match),
            mine: SampleData.currentUser.id,
            showingMessages: false
        )

        XCTAssertEqual(suite.beginActivity, match)
        XCTAssertTrue(suite.badge)
    }

    /// Un message ordinaire ne pose rien. C'est le cas qui se casse en
    /// silence : une carte « quelqu'un vous a écrit » à chaque réponse d'une
    /// conversation en cours serait une notification de trop, à chaque phrase.
    func testAnOrdinaryMessageOpensNothing() {
        let message = Message(
            id: UUID(),
            conversationId: UUID(),
            senderId: UUID(),
            body: "ça va ?",
            sentAt: .now
        )

        let suite = MainTabView.reaction(
            to: .messageReceived(message),
            mine: SampleData.currentUser.id,
            showingMessages: false
        )

        XCTAssertNil(suite.beginActivity)
        XCTAssertTrue(suite.badge, "un message d'autrui compte quand même")
    }

    /// Le socket renvoie nos propres messages : se mettre une pastille à
    /// soi-même pour ce qu'on vient d'écrire est absurde.
    func testMyOwnEchoedMessageDoesNotBadgeMe() {
        let mien = Message(
            id: UUID(),
            conversationId: UUID(),
            senderId: SampleData.currentUser.id,
            body: "salut",
            sentAt: .now
        )

        let suite = MainTabView.reaction(
            to: .messageReceived(mien),
            mine: SampleData.currentUser.id,
            showingMessages: false
        )

        XCTAssertFalse(suite.badge)
        XCTAssertNil(suite.beginActivity)
    }

    /// Le premier message efface la carte : elle a fait son travail.
    func testTheFirstMessageClosesTheCard() async {
        let journal = RecordingMatchActivityService()
        let conversation = SampleData.conversations[0]
        let viewModel = ChatViewModel(
            conversation: conversation,
            chat: DemoChatService(),
            discovery: DemoDiscoveryService(),
            currentUserId: SampleData.currentUser.id,
            activities: journal
        )

        viewModel.draft = "salut"
        await viewModel.send()

        let ended = await journal.ended
        XCTAssertEqual(ended, [conversation.matchId], "le premier message ferme la carte")
    }

    /// Les suivants ne la concernent plus.
    ///
    /// Sans ce test, une condition mal écrite fermerait l'activité à chaque
    /// message. Ça ne se verrait jamais — fermer ce qui est déjà fermé ne
    /// produit rien — mais chaque envoi paierait un aller-retour vers
    /// ActivityKit.
    func testLaterMessagesLeaveItAlone() async {
        let journal = RecordingMatchActivityService()
        let viewModel = ChatViewModel(
            conversation: SampleData.conversations[0],
            chat: DemoChatService(),
            discovery: DemoDiscoveryService(),
            currentUserId: SampleData.currentUser.id,
            activities: journal
        )

        for mot in ["salut", "ça va ?", "tu fais quoi ce soir"] {
            viewModel.draft = mot
            await viewModel.send()
        }

        let ended = await journal.ended
        XCTAssertEqual(ended.count, 1, "un seul appel, pour le premier message")
    }
}
