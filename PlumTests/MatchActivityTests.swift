import XCTest
@testable import Plum

/// ActivityKit ne fonctionne pas dans un test unitaire : il lui faut un
/// système vivant, un appareil, une autorisation. Ce qui se vérifie ici n'est
/// donc pas le dessin de la carte — c'est la logique qui décide *quand* elle
/// apparaît et quand elle s'efface. C'est elle qu'on casse en refactorant, et
/// elle seule qu'un test peut tenir.
@MainActor
final class MatchActivityTests: XCTestCase {
    /// Un match doit poser sa carte, avec la date du serveur.
    func testAMatchOpensItsCard() async {
        let journal = RecordingMatchActivityService()
        let viewModel = DiscoveryViewModel(discovery: DemoDiscoveryService(), activities: journal)
        await viewModel.loadInitialDeck()

        guard let profile = viewModel.topProfile else {
            return XCTFail("Le deck de démonstration doit avoir des cartes")
        }
        await viewModel.swipe(profile, decision: .like)

        // Le service de démonstration ne fait matcher que certains profils :
        // on ne vérifie donc qu'une implication, et elle suffit.
        if let match = viewModel.newMatch {
            let begun = await journal.begun
            XCTAssertEqual(begun, [match.id], "un match doit poser exactement une carte")
        } else {
            let begun = await journal.begun
            XCTAssertTrue(begun.isEmpty, "sans match, aucune carte ne doit être posée")
        }
    }

    /// Un passe ne doit rien poser. C'est le cas qui se casse en silence :
    /// une carte « vous avez matché » après un refus serait une erreur qu'on
    /// ne pardonne pas à une application de rencontres.
    func testAPassOpensNothing() async {
        let journal = RecordingMatchActivityService()
        let viewModel = DiscoveryViewModel(discovery: DemoDiscoveryService(), activities: journal)
        await viewModel.loadInitialDeck()

        guard let profile = viewModel.topProfile else {
            return XCTFail("Le deck de démonstration doit avoir des cartes")
        }
        await viewModel.swipe(profile, decision: .pass)

        let begun = await journal.begun
        XCTAssertTrue(begun.isEmpty, "un passe ne pose pas de carte")
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
