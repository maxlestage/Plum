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

    /// La sélection s'affiche, et écrire ouvre bien une conversation.
    ///
    /// C'est le chemin entier du nouveau geste : plus de glissement, deux
    /// boutons nommés, et un message qui part. Le test tape « Écrire » puis
    /// « Envoyer » — s'il manquait le moindre maillon, on le verrait ici et
    /// nulle part ailleurs, puisque aucun test unitaire ne traverse la vue.
    func testSigningInReachesTheSelectionAndWritingOpensAThread() {
        signIn()

        let write = app.buttons["Écrire"].firstMatch
        XCTAssertTrue(
            write.waitForExistence(timeout: timeout),
            "La sélection du jour ne s'affiche pas après connexion"
        )
        write.tap()

        let send = app.buttons["Envoyer"]
        XCTAssertTrue(send.waitForExistence(timeout: timeout), "La feuille d'écriture ne s'ouvre pas")

        // Le champ est un `TextField` à axe vertical : vue texte sur certaines
        // versions, champ texte sur d'autres. D'où l'identifiant plutôt qu'un
        // `firstMatch`, qui attraperait le premier venu.
        let byView = app.textViews["champ-premier-message"]
        let field = byView.exists ? byView : app.textFields["champ-premier-message"]
        XCTAssertTrue(field.waitForExistence(timeout: timeout), "Le champ de saisie est absent")
        field.tap()
        field.typeText("Votre deuxième phrase m'a fait rire.")

        send.tap()

        XCTAssertTrue(
            app.alerts["Message envoyé"].waitForExistence(timeout: timeout),
            "Après l'envoi, l'écran doit dire que la conversation est ouverte"
        )
    }

    /// Laisser passer demande confirmation : c'est définitif, et le dire avant
    /// remplace le retour en arrière qu'on ne peut plus offrir.
    func testPassingAsksBeforeItIsFinal() {
        signIn()

        let pass = app.buttons["Passer"].firstMatch
        XCTAssertTrue(pass.waitForExistence(timeout: timeout), "La sélection ne s'affiche pas")
        pass.tap()

        XCTAssertTrue(
            app.buttons["Laisser passer"].waitForExistence(timeout: timeout),
            "Un geste définitif doit demander confirmation"
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

extension PlumSmokeTests {
    /// La carte montre la photo, le nom et les deux phrases ; le reste — les
    /// autres photos, les centres d'intérêt — est derrière. Toucher la carte
    /// doit l'ouvrir, sans quoi on décide encore sur une seule image.
    func testTheFullProfileOpensFromTheCard() {
        signIn()

        // La photo de la première carte porte le nom de la personne : c'est
        // elle qui ouvre le profil. On vise le premier bouton dont l'étiquette
        // contient « ans », qui est la forme de cette étiquette.
        let card = app.buttons.containing(
            NSPredicate(format: "label CONTAINS[c] %@", " ans")
        ).firstMatch
        XCTAssertTrue(
            card.waitForExistence(timeout: timeout),
            "La première carte de la sélection doit être ouvrable"
        )
        card.tap()

        // Ne pas s'appuyer sur le texte affiché : plumSectionHeader() applique
        // .textCase(.uppercase), donc « À propos » se rend « À PROPOS ». Le
        // bouton de fermeture, lui, n'existe que quand la feuille est ouverte.
        let close = app.buttons["Fermer"]
        XCTAssertTrue(
            close.waitForExistence(timeout: timeout),
            "La feuille de profil ne s'est pas ouverte"
        )
        XCTAssertTrue(
            app.descendants(matching: .any)["feuille-profil"].exists,
            "Le contenu de la feuille est absent"
        )

        close.tap()
        XCTAssertTrue(
            app.buttons["Passer"].firstMatch.waitForExistence(timeout: timeout),
            "Fermer doit ramener à la sélection"
        )
    }
}
