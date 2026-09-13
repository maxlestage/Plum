import XCTest
@testable import Plum

/// When the server invalidates a session, the client clears its credentials on
/// its own. Before this was wired up, the interface never found out: it stayed
/// on the tabs, piling up errors on every request, until someone force-quit.
final class SessionExpiryTests: XCTestCase {
    private let configuration = APIConfiguration(
        baseURL: URL(string: "https://api.plum.app/api/v1")!,
        webSocketURL: URL(string: "wss://api.plum.app/ws")!
    )

    private func expiringTokens() -> AuthTokens {
        AuthTokens(
            accessToken: "access-1",
            refreshToken: "refresh-1",
            expiresAt: Date.now.addingTimeInterval(600)
        )
    }

    func testARefusedRefreshAnnouncesTheExpiry() async {
        let transport = ExpiryStubTransport(statuses: [401, 401])
        let store = InMemoryTokenStore(tokens: expiringTokens())
        let client = APIClient(configuration: configuration, transport: transport, tokenStore: store)

        // Subscribe first: the announcement happens during the request.
        let stream = await client.sessionExpirations()

        // Bounded on purpose. Awaiting the stream directly would hang forever
        // the day this regresses, and a hanging test is worse than a failing
        // one: it burns the whole CI budget before reporting anything.
        let listener = Task {
            for await _ in stream { return true }
            return false
        }
        let watchdog = Task {
            try? await Task.sleep(for: .seconds(2))
            listener.cancel()
        }

        _ = try? await client.send(.get("me/profile"), as: Profile.self)

        let announced = await listener.value
        watchdog.cancel()

        XCTAssertTrue(announced, "Un rafraîchissement refusé doit être annoncé")
        XCTAssertNil(store.read(), "Les jetons doivent être effacés")
    }

    /// Signing out on purpose clears the same credentials, but is not an
    /// expiry: the screen that asked already knows. Asserting the absence of
    /// an announcement would mean awaiting a stream that never finishes, so
    /// this checks what is observable — the credentials are gone.
    func testAnIntentionalSignOutClearsTheCredentials() async {
        let store = InMemoryTokenStore(tokens: expiringTokens())
        let client = APIClient(
            configuration: configuration,
            transport: ExpiryStubTransport(statuses: [200]),
            tokenStore: store
        )

        await client.signOutLocally()

        XCTAssertNil(store.read())
        let tokens = await client.currentTokens()
        XCTAssertNil(tokens)
    }

    @MainActor
    func testExpiringSendsTheSessionBackToTheWelcomeScreen() async {
        let session = SessionStore(auth: DemoAuthService())
        session.adopt(SampleData.currentUser)
        session.currentProfile = SampleData.myProfile

        session.expire()

        XCTAssertNil(session.state.user)
        XCTAssertNil(session.currentProfile)
        XCTAssertEqual(session.state, .signedOut)
    }

    @MainActor
    func testTheSessionObservesTheExpiryStream() async {
        let auth = DemoAuthService()
        let session = SessionStore(auth: auth)
        _ = try? await auth.signIn(email: "moi@plum.app", password: "motdepasse")
        await session.restore()
        XCTAssertNotNil(session.state.user)

        // The observer subscribes in a task of its own; announce until it is
        // listening rather than guessing at a delay.
        let deadline = Date.now.addingTimeInterval(3)
        while session.state.user != nil, Date.now < deadline {
            await auth.simulateExpiry()
            try? await Task.sleep(for: .milliseconds(20))
        }

        XCTAssertNil(session.state.user, "Une session expirée doit ramener à l'accueil")
    }
}

/// Answers every request with the next status in the list.
private actor ExpiryStubTransport: HTTPTransport {
    private var statuses: [Int]

    init(statuses: [Int]) {
        self.statuses = statuses
    }

    func data(for request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        let status = statuses.isEmpty ? 200 : statuses.removeFirst()
        let response = HTTPURLResponse(
            url: request.url!,
            statusCode: status,
            httpVersion: nil,
            headerFields: nil
        )!
        return (Data("{}".utf8), response)
    }
}

/// An app that says "you accept our terms" has to make them reachable, and the
/// App Store will not take it otherwise.
final class LegalLinksTests: XCTestCase {
    func testEveryLinkIsAWellFormedHTTPSURL() {
        for url in [LegalLinks.terms, LegalLinks.privacy, LegalLinks.support] {
            XCTAssertEqual(url.scheme, "https", "\(url) doit être en HTTPS")
            XCTAssertNotNil(url.host, "\(url) doit avoir un hôte")
        }
    }

    func testLinksAreDistinct() {
        let urls = Set([LegalLinks.terms, LegalLinks.privacy, LegalLinks.support])
        XCTAssertEqual(urls.count, 3, "Trois documents, trois adresses")
    }
}
