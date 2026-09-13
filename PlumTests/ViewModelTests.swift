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
final class DiscoveryViewModelTests: XCTestCase {
    func testLoadsTheDeckAndExposesTheTopCard() async {
        let viewModel = DiscoveryViewModel(discovery: DemoDiscoveryService())

        await viewModel.loadInitialDeck()

        XCTAssertEqual(viewModel.topProfile?.id, SampleData.deck.first?.id)
        XCTAssertLessThanOrEqual(
            viewModel.visibleProfiles.count,
            PlumTheme.Layout.cardStackDepth
        )
    }

    /// The card leaves the stack immediately: waiting for the network would
    /// make the gesture feel broken.
    func testSwipeRemovesTheCardBeforeTheServerAnswers() async {
        let viewModel = DiscoveryViewModel(discovery: DemoDiscoveryService())
        await viewModel.loadInitialDeck()
        guard let top = viewModel.topProfile else {
            return XCTFail("Le deck devait contenir au moins un profil")
        }

        await viewModel.swipe(top, decision: .pass)

        XCTAssertFalse(viewModel.profiles.contains { $0.id == top.id })
        XCTAssertTrue(viewModel.canRewind, "Un passe doit pouvoir être annulé")
    }

    func testRewindOnlyAppliesAfterAPass() async {
        let viewModel = DiscoveryViewModel(discovery: DemoDiscoveryService())
        await viewModel.loadInitialDeck()
        guard let top = viewModel.topProfile else {
            return XCTFail("Le deck devait contenir au moins un profil")
        }

        await viewModel.swipe(top, decision: .like)
        XCTAssertFalse(viewModel.canRewind)

        await viewModel.rewind()
        XCTAssertNotEqual(viewModel.topProfile?.id, top.id)
    }

    /// The thresholds are what separates "I changed my mind" from a decision.
    func testGestureThresholds() {
        XCTAssertNil(DiscoveryView.decision(for: CGSize(width: 40, height: 0)))
        XCTAssertEqual(DiscoveryView.decision(for: CGSize(width: 200, height: 0)), .like)
        XCTAssertEqual(DiscoveryView.decision(for: CGSize(width: -200, height: 0)), .pass)
        XCTAssertEqual(DiscoveryView.decision(for: CGSize(width: 0, height: -200)), .superLike)
        XCTAssertEqual(
            DiscoveryView.decision(for: CGSize(width: 200, height: -200)),
            .like,
            "Un geste franchement latéral reste un like, pas un coup de cœur"
        )
    }

    func testExitTranslationsLeaveTheScreen() {
        XCTAssertGreaterThan(DiscoveryView.exitTranslation(for: .like).width, 400)
        XCTAssertLessThan(DiscoveryView.exitTranslation(for: .pass).width, -400)
        XCTAssertLessThan(DiscoveryView.exitTranslation(for: .superLike).height, -400)
    }
}

@MainActor
final class ChatViewModelTests: XCTestCase {
    private func makeViewModel() -> ChatViewModel {
        ChatViewModel(
            conversation: SampleData.conversations[0],
            chat: DemoChatService(),
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
    /// reaches the deck as an empty card.
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
