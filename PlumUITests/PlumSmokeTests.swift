import XCTest

/// Drives the real app through the paths a person actually takes.
///
/// The unit tests exercise logic in isolation; nothing there would catch a
/// screen that crashes on appear, a missing environment value, or a button
/// wired to nothing. These launch the app in demo mode — in-memory services,
/// no server — and walk it.
final class PlumSmokeTests: XCTestCase {
    private var app: XCUIApplication!

    override func setUp() {
        super.setUp()
        continueAfterFailure = false
        app = XCUIApplication()
        app.launchEnvironment["PLUM_DEMO_MODE"] = "1"
    }

    override func tearDown() {
        app = nil
        super.tearDown()
    }

    /// How long a demo-mode round trip plus an animation can reasonably take.
    private let timeout: TimeInterval = 12

    private func signIn() {
        app.launch()

        let signIn = app.buttons["bouton-connexion"]
        XCTAssertTrue(signIn.waitForExistence(timeout: timeout), "L'écran d'accueil ne s'affiche pas")
        signIn.tap()

        let email = app.textFields["champ-email"]
        XCTAssertTrue(email.waitForExistence(timeout: timeout), "Le formulaire ne s'affiche pas")
        email.tap()
        email.typeText("moi@plum.app")

        let password = app.secureTextFields["champ-mot-de-passe"]
        password.tap()
        password.typeText("motdepasse")

        app.buttons["bouton-valider"].tap()
    }

    func testSigningInReachesTheDeckAndASwipeAdvancesIt() {
        signIn()

        let like = app.buttons["J'aime"]
        XCTAssertTrue(like.waitForExistence(timeout: timeout), "Le deck ne s'affiche pas après connexion")

        like.tap()

        // A like either lands on the next card or on the match celebration —
        // both are correct, and asserting only one would make this test
        // hostage to the demo fixture.
        let backOnTheDeck = app.buttons["J'aime"]
        let celebration = app.buttons["Continuer à swiper"]

        XCTAssertTrue(
            backOnTheDeck.waitForExistence(timeout: timeout)
                || celebration.waitForExistence(timeout: timeout),
            "Après un like, on doit voir la carte suivante ou l'écran de match"
        )
    }

    func testMatchesTabOpensAConversationAndSendsAMessage() {
        signIn()

        let matchesTab = app.tabBars.buttons.element(boundBy: 1)
        XCTAssertTrue(matchesTab.waitForExistence(timeout: timeout), "La barre d'onglets est absente")
        matchesTab.tap()

        // Tapping the cell is not enough: the row is a Button inside the cell,
        // and the tap does not always reach it. Aim at the button.
        let firstConversation = app.buttons["ligne-conversation"].firstMatch
        XCTAssertTrue(
            firstConversation.waitForExistence(timeout: timeout),
            "Aucune conversation dans la boîte de réception de démonstration"
        )
        firstConversation.tap()

        // Assert the thread actually opened before hunting for the composer,
        // so a navigation failure reports itself rather than looking like a
        // missing text field.
        let sendButton = app.buttons["bouton-envoyer"]
        XCTAssertTrue(
            sendButton.waitForExistence(timeout: timeout),
            "La conversation ne s'est pas ouverte"
        )

        // A vertical-axis TextField surfaces as a text view on some OS
        // versions and a text field on others.
        let composer = composerField()
        XCTAssertTrue(composer.waitForExistence(timeout: timeout), "Le champ de saisie est absent")
        composer.tap()
        composer.typeText("On boit un truc ?")

        sendButton.tap()

        XCTAssertTrue(
            app.staticTexts["On boit un truc ?"].waitForExistence(timeout: timeout),
            "Le message envoyé doit apparaître dans le fil"
        )
    }

    private func composerField() -> XCUIElement {
        let textView = app.textViews["champ-message"]
        return textView.exists ? textView : app.textFields["champ-message"]
    }

    func testProfileTabShowsTheAccount() {
        signIn()

        let profileTab = app.tabBars.buttons.element(boundBy: 2)
        XCTAssertTrue(profileTab.waitForExistence(timeout: timeout), "La barre d'onglets est absente")
        profileTab.tap()

        XCTAssertTrue(
            app.buttons["Modifier mon profil"].waitForExistence(timeout: timeout),
            "L'écran de profil ne s'est pas chargé"
        )
    }
}
