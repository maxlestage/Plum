import XCTest
@testable import Plum

@MainActor
final class AuthValidationTests: XCTestCase {
    func testEmailValidationCatchesTheObviousTypos() {
        XCTAssertTrue(AuthViewModel.isValidEmail("moi@plum.app"))
        XCTAssertTrue(AuthViewModel.isValidEmail("  moi@plum.app "))
        XCTAssertFalse(AuthViewModel.isValidEmail("moi@plum"))
        XCTAssertFalse(AuthViewModel.isValidEmail("@plum.app"))
        XCTAssertFalse(AuthViewModel.isValidEmail("moi plum@plum.app"))
        XCTAssertFalse(AuthViewModel.isValidEmail(""))
    }

    /// The age gate is the one rule the client must not get wrong.
    func testAgeGateRejectsMinors() {
        let seventeen = Calendar.current.date(byAdding: .year, value: -17, to: .now)!
        let eighteen = Calendar.current.date(byAdding: .year, value: -18, to: .now)!

        XCTAssertFalse(AuthViewModel.isOldEnough(seventeen))
        XCTAssertTrue(AuthViewModel.isOldEnough(eighteen))
    }

    func testSignUpRequiresNameAndValidCredentials() {
        let viewModel = AuthViewModel(
            mode: .signUp,
            auth: DemoAuthService(),
            session: SessionStore(auth: DemoAuthService())
        )

        XCTAssertFalse(viewModel.canSubmit)

        viewModel.email = "moi@plum.app"
        viewModel.password = "motdepasse"
        XCTAssertFalse(viewModel.canSubmit, "Le prénom manque encore")

        viewModel.displayName = "Camille"
        XCTAssertTrue(viewModel.canSubmit)

        viewModel.password = "court"
        XCTAssertFalse(viewModel.canSubmit)
        XCTAssertEqual(viewModel.passwordHint, "8 caractères minimum.")
    }

    func testSuccessfulSignInPutsTheUserIntoTheSession() async {
        let auth = DemoAuthService()
        let session = SessionStore(auth: auth)
        let viewModel = AuthViewModel(mode: .signIn, auth: auth, session: session)

        viewModel.email = "moi@plum.app"
        viewModel.password = "motdepasse"
        await viewModel.submit()

        XCTAssertEqual(session.state.user?.email, "moi@plum.app")
        XCTAssertNil(viewModel.errorMessage)
    }

    /// An invalid form must not reach the network at all.
    func testSubmitIsANoOpWhileTheFormIsInvalid() async {
        let auth = DemoAuthService()
        let session = SessionStore(auth: auth)
        let viewModel = AuthViewModel(mode: .signIn, auth: auth, session: session)

        viewModel.email = "moi@plum"
        viewModel.password = "1234567"
        await viewModel.submit()

        XCTAssertNil(session.state.user)
        XCTAssertNil(viewModel.errorMessage)
    }
}

@MainActor
final class SelectionViewModelTests: XCTestCase {
    func testLoadsTheSelectionOfTheDay() async {
        let viewModel = DiscoveryViewModel(discovery: DemoDiscoveryService())

        await viewModel.load()

        XCTAssertEqual(viewModel.profiles.first?.id, SampleData.selection.first?.id)
        XCTAssertEqual(viewModel.size, SampleData.selection.count)
        XCTAssertNotNil(viewModel.refreshesAt, "l'écran doit pouvoir dire quand revenir")
    }

    /// Écrire ouvre un fil tout de suite : il n'y a personne à attendre.
    func testWritingOpensAThreadAndTakesTheProfileOut() async {
        let viewModel = DiscoveryViewModel(discovery: DemoDiscoveryService())
        await viewModel.load()
        guard let premier = viewModel.profiles.first else {
            return XCTFail("La sélection devait contenir au moins un profil")
        }

        let parti = await viewModel.write(to: premier, body: "  Votre deuxième phrase.  ")

        XCTAssertTrue(parti)
        XCTAssertFalse(viewModel.profiles.contains { $0.id == premier.id })
        XCTAssertEqual(
            viewModel.justOpened?.displayName,
            premier.displayName,
            "l'écran doit pouvoir dire vers quelle conversation aller"
        )
    }

    /// Un message vide n'en est pas un, et le geste ne doit rien consommer.
    ///
    /// C'est le pendant du seuil de glissement qui séparait autrefois « j'ai
    /// changé d'avis » d'une décision : ici, la décision est le texte.
    func testAnEmptyMessageIsNotSentAndCostsNothing() async {
        let viewModel = DiscoveryViewModel(discovery: DemoDiscoveryService())
        await viewModel.load()
        guard let premier = viewModel.profiles.first else {
            return XCTFail("La sélection devait contenir au moins un profil")
        }
        let avant = viewModel.profiles.count

        for vide in ["", "   ", "\n\t "] {
            let parti = await viewModel.write(to: premier, body: vide)
            XCTAssertFalse(parti, "« \(vide) » est parti")
        }

        XCTAssertEqual(viewModel.profiles.count, avant, "la sélection a été entamée")
        XCTAssertNil(viewModel.justOpened)
    }

    func testPassingTakesTheProfileOut() async {
        let viewModel = DiscoveryViewModel(discovery: DemoDiscoveryService())
        await viewModel.load()
        guard let premier = viewModel.profiles.first else {
            return XCTFail("La sélection devait contenir au moins un profil")
        }

        await viewModel.pass(premier)

        XCTAssertFalse(viewModel.profiles.contains { $0.id == premier.id })
        XCTAssertNil(viewModel.safetyFailure)
    }

    /// Une sélection vide au chargement et une sélection épuisée ne disent pas
    /// la même chose à l'écran : l'une attend, l'autre annonce demain.
    func testAnEmptySelectionIsOnlyEmptyOnceItHasLoaded() async {
        let viewModel = DiscoveryViewModel(discovery: DemoDiscoveryService())

        XCTAssertFalse(viewModel.isEmpty, "rien n'a encore été demandé")

        await viewModel.load()
        for profile in viewModel.profiles {
            await viewModel.pass(profile)
        }

        XCTAssertTrue(viewModel.isEmpty)
    }
}

@MainActor
final class ChatViewModelTests: XCTestCase {
    private func makeViewModel() -> ChatViewModel {
        ChatViewModel(
            conversation: SampleData.conversations[0],
            chat: DemoChatService(),
            discovery: DemoDiscoveryService(),
            currentUserId: SampleData.currentUser.id
        )
    }

    func testSendShowsTheBubbleImmediatelyAndThenMarksItSent() async {
        let viewModel = makeViewModel()
        viewModel.draft = "On boit un truc ?"

        await viewModel.send()

        let mine = viewModel.items.filter { viewModel.isMine($0) }
        XCTAssertEqual(mine.last?.message.body, "On boit un truc ?")
        XCTAssertEqual(mine.last?.deliveryState, .sent)
        XCTAssertTrue(viewModel.draft.isEmpty, "Le champ se vide dès l'envoi")
    }

    func testEmptyDraftsAreNotSent() async {
        let viewModel = makeViewModel()
        viewModel.draft = "   \n "

        await viewModel.send()

        XCTAssertTrue(viewModel.items.isEmpty)
        XCTAssertFalse(viewModel.canSend)
    }

    func testHistoryArrivesInChronologicalOrder() async {
        let viewModel = makeViewModel()

        await viewModel.start()

        let dates = viewModel.items.map(\.message.sentAt)
        XCTAssertEqual(dates, dates.sorted())
        XCTAssertFalse(viewModel.items.isEmpty)
        viewModel.stop()
    }
}

final class PreferencesTests: XCTestCase {
    /// Whatever the sliders do, the range that reaches the server has to make
    /// sense.
    func testSanitizationKeepsTheAgeRangeLegalAndCoherent() {
        let prefs = DiscoveryPreferences(
            interestedIn: .everyone,
            minAge: 12,
            maxAge: 8,
            maxDistanceKm: 5_000,
            showMeOnPlum: true
        ).sanitized

        XCTAssertEqual(prefs.minAge, 18)
        XCTAssertGreaterThanOrEqual(prefs.maxAge, prefs.minAge)
        XCTAssertEqual(prefs.maxDistanceKm, 300)
    }

    func testDefaultsAreAlreadyValid() {
        XCTAssertEqual(DiscoveryPreferences.default, DiscoveryPreferences.default.sanitized)
    }
}

@MainActor
final class ChatDeliveryTests: XCTestCase {
    private func makeViewModel() -> ChatViewModel {
        ChatViewModel(
            conversation: SampleData.conversations[0],
            chat: DemoChatService(),
            discovery: DemoDiscoveryService(),
            currentUserId: SampleData.currentUser.id
        )
    }

    /// Retrying must not go through the composer: whatever has been typed
    /// since the failure has to survive.
    func testRetryLeavesTheDraftAlone() async {
        let viewModel = makeViewModel()
        viewModel.draft = "autre chose, en cours de frappe"

        let failed = ChatItem(
            message: Message(
                id: UUID(),
                conversationId: SampleData.conversations[0].id,
                senderId: SampleData.currentUser.id,
                body: "message qui avait échoué",
                sentAt: .now
            ),
            deliveryState: .failed
        )

        await viewModel.retry(failed)

        XCTAssertEqual(viewModel.draft, "autre chose, en cours de frappe")
        XCTAssertTrue(viewModel.items.contains { $0.message.body == "message qui avait échoué" })
        XCTAssertFalse(viewModel.items.contains { $0.id == failed.id })
    }

    /// Sending twice must never leave two rows sharing an identifier — SwiftUI
    /// renders that wrong and the thread jumps.
    func testIdentifiersStayUnique() async {
        let viewModel = makeViewModel()

        viewModel.draft = "un"
        await viewModel.send()
        viewModel.draft = "deux"
        await viewModel.send()

        let ids = viewModel.items.map(\.id)
        XCTAssertEqual(ids.count, Set(ids).count)
    }
}

@MainActor
final class OnboardingViewModelTests: XCTestCase {
    private func makeViewModel(session: SessionStore) -> OnboardingViewModel {
        OnboardingViewModel(profiles: DemoProfileService(), session: session)
    }

    /// The photo gate is the whole point of the flow: without it a new account
    /// reaches the selection as an empty card.
    func testAPhotoIsRequiredBeforeLeavingTheFirstStep() async {
        let viewModel = makeViewModel(session: SessionStore(auth: DemoAuthService()))

        XCTAssertFalse(viewModel.canAdvance)
        XCTAssertEqual(viewModel.blockedReason, "Ajoutez au moins une photo pour continuer.")

        await viewModel.addPhoto(Data("jpeg".utf8))

        XCTAssertTrue(viewModel.canAdvance)
        XCTAssertNil(viewModel.blockedReason)

        _ = await viewModel.advance()
        XCTAssertEqual(viewModel.step, .about)
    }

    func testCityIsRequiredOnTheSecondStep() async {
        let viewModel = makeViewModel(session: SessionStore(auth: DemoAuthService()))
        await viewModel.addPhoto(Data("jpeg".utf8))
        _ = await viewModel.advance()

        XCTAssertEqual(viewModel.step, .about)
        XCTAssertFalse(viewModel.canAdvance)

        viewModel.city = "Paris"
        XCTAssertTrue(viewModel.canAdvance)
    }

    func testGoingBackDoesNotSkipPastTheFirstStep() async {
        let viewModel = makeViewModel(session: SessionStore(auth: DemoAuthService()))

        viewModel.goBack()
        XCTAssertEqual(viewModel.step, .photos, "Il n'y a rien avant la première étape")

        await viewModel.addPhoto(Data("jpeg".utf8))
        _ = await viewModel.advance()
        viewModel.goBack()
        XCTAssertEqual(viewModel.step, .photos)
    }

    /// Finishing has to flip the account over, or the app loops back into
    /// onboarding on the next launch.
    func testFinishingMarksTheAccountAsComplete() async {
        let session = SessionStore(auth: DemoAuthService())
        session.adopt(
            User(
                id: SampleData.currentUser.id,
                email: "neuf@plum.app",
                createdAt: .now,
                profileCompleted: false
            )
        )
        XCTAssertTrue(session.needsOnboarding)

        let viewModel = makeViewModel(session: session)
        await viewModel.addPhoto(Data("jpeg".utf8))
        _ = await viewModel.advance()
        viewModel.city = "Paris"
        viewModel.bio = "Ici pour les terrasses."
        _ = await viewModel.advance()

        XCTAssertEqual(viewModel.step, .preferences)
        XCTAssertTrue(viewModel.isLastStep)

        let finished = await viewModel.advance()

        XCTAssertTrue(finished)
        XCTAssertFalse(session.needsOnboarding)
        XCTAssertNotNil(session.currentProfile)
    }

    func testProgressReachesOneOnTheLastStep() {
        let viewModel = makeViewModel(session: SessionStore(auth: DemoAuthService()))
        XCTAssertEqual(viewModel.progress, 1.0 / 3.0, accuracy: 0.001)
        viewModel.step = .preferences
        XCTAssertEqual(viewModel.progress, 1.0, accuracy: 0.001)
    }
}

@MainActor
final class ChatTypingNoticeTests: XCTestCase {
    /// Emitting on every keystroke would be a packet per character; emitting
    /// never — which is what the app did before — means the other side sees
    /// nothing at all.
    func testTypingNoticesAreThrottled() async {
        let chat = DemoChatService()
        let viewModel = ChatViewModel(
            conversation: SampleData.conversations[0],
            chat: chat,
            discovery: DemoDiscoveryService(),
            currentUserId: SampleData.currentUser.id
        )

        viewModel.draft = "s"
        await viewModel.draftChanged()
        viewModel.draft = "sa"
        await viewModel.draftChanged()
        viewModel.draft = "sal"
        await viewModel.draftChanged()

        let notices = await chat.typingNotices
        XCTAssertEqual(notices, 1, "Trois frappes rapprochées, une seule notification")
    }

    func testAnEmptyDraftSaysNothing() async {
        let chat = DemoChatService()
        let viewModel = ChatViewModel(
            conversation: SampleData.conversations[0],
            chat: chat,
            discovery: DemoDiscoveryService(),
            currentUserId: SampleData.currentUser.id
        )

        viewModel.draft = ""
        await viewModel.draftChanged()

        let notices = await chat.typingNotices
        XCTAssertEqual(notices, 0)
    }
}

@MainActor
final class ProfilePhotoOrderTests: XCTestCase {
    private func makeViewModel() -> ProfileViewModel {
        ProfileViewModel(
            profiles: DemoProfileService(),
            session: SessionStore(auth: DemoAuthService())
        )
    }

    /// The cover photo decides whether anyone reads the rest of the profile,
    /// and until now there was no way to choose it.
    func testPromotingAPhotoMakesItTheCover() async {
        let viewModel = makeViewModel()
        await viewModel.load()

        guard let profile = viewModel.profile, profile.photos.count > 1 else {
            return XCTFail("Le profil de démonstration doit avoir plusieurs photos")
        }
        let second = profile.orderedPhotos[1]

        await viewModel.makeCover(second)

        XCTAssertEqual(viewModel.profile?.coverPhoto?.id, second.id)
        XCTAssertEqual(viewModel.profile?.orderedPhotos.first?.id, second.id)
    }

    func testPositionsStayContiguousAfterPromotion() async {
        let viewModel = makeViewModel()
        await viewModel.load()
        guard let second = viewModel.profile?.orderedPhotos.dropFirst().first else {
            return XCTFail("Le profil de démonstration doit avoir plusieurs photos")
        }

        await viewModel.makeCover(second)

        let positions = viewModel.profile?.orderedPhotos.map(\.position) ?? []
        XCTAssertEqual(positions, Array(0..<positions.count))
    }

    func testPromotingTheCoverAgainChangesNothing() async {
        let viewModel = makeViewModel()
        await viewModel.load()
        guard let cover = viewModel.profile?.coverPhoto else {
            return XCTFail("Le profil de démonstration doit avoir une photo")
        }

        await viewModel.makeCover(cover)

        XCTAssertEqual(viewModel.profile?.coverPhoto?.id, cover.id)
    }
}
