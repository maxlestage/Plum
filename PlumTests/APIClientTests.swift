import XCTest
@testable import Plum

/// Answers requests from a script, and records what it was asked for.
private actor StubTransport: HTTPTransport {
    struct Reply {
        var status: Int
        var body: Data

        static func json(_ raw: String, status: Int = 200) -> Reply {
            Reply(status: status, body: Data(raw.utf8))
        }
    }

    private var replies: [Reply]
    private(set) var requests: [URLRequest] = []

    init(replies: [Reply]) {
        self.replies = replies
    }

    func data(for request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        requests.append(request)
        let reply = replies.isEmpty ? Reply.json("{}") : replies.removeFirst()
        let response = HTTPURLResponse(
            url: request.url!,
            statusCode: reply.status,
            httpVersion: nil,
            headerFields: nil
        )!
        return (reply.body, response)
    }

    func recordedRequests() -> [URLRequest] { requests }
}

final class APIClientTests: XCTestCase {
    private let configuration = APIConfiguration(
        baseURL: URL(string: "https://api.plum.app/api/v1")!,
        webSocketURL: URL(string: "wss://api.plum.app/ws")!
    )

    private func tokens(expiringIn seconds: TimeInterval) -> AuthTokens {
        AuthTokens(
            accessToken: "access-1",
            refreshToken: "refresh-1",
            expiresAt: Date.now.addingTimeInterval(seconds)
        )
    }

    private var profileJSON: String {
        """
        {
          "id": "8A1B0A2C-1111-4444-8888-0123456789AB",
          "display_name": "Inès",
          "birth_date": "1998-04-12T00:00:00Z",
          "gender": "woman",
          "bio": "",
          "city": "Paris",
          "photos": [],
          "interests": []
        }
        """
    }

    func testAttachesStoredAccessTokenToAuthenticatedRequests() async throws {
        let transport = StubTransport(replies: [.json(profileJSON)])
        let client = APIClient(
            configuration: configuration,
            transport: transport,
            tokenStore: InMemoryTokenStore(tokens: tokens(expiringIn: 600))
        )

        _ = try await client.send(.get("me/profile"), as: Profile.self)

        let sent = await transport.recordedRequests()
        XCTAssertEqual(sent.count, 1)
        XCTAssertEqual(sent.first?.value(forHTTPHeaderField: "Authorization"), "Bearer access-1")
    }

    /// An expired access token should be renewed before the call goes out, not
    /// after it comes back 401.
    func testRefreshesExpiredTokenBeforeSendingRequest() async throws {
        let refreshed = """
        {
          "access_token": "access-2",
          "refresh_token": "refresh-2",
          "expires_at": "2099-01-01T00:00:00Z"
        }
        """
        let transport = StubTransport(replies: [.json(refreshed), .json(profileJSON)])
        let store = InMemoryTokenStore(tokens: tokens(expiringIn: -60))
        let client = APIClient(configuration: configuration, transport: transport, tokenStore: store)

        _ = try await client.send(.get("me/profile"), as: Profile.self)

        let sent = await transport.recordedRequests()
        XCTAssertEqual(sent.count, 2)
        XCTAssertEqual(sent.first?.url?.lastPathComponent, "refresh")
        XCTAssertEqual(sent.last?.value(forHTTPHeaderField: "Authorization"), "Bearer access-2")
        XCTAssertEqual(store.read()?.accessToken, "access-2")
    }

    /// A 401 on a token that looked fresh means the server revoked it: refresh
    /// once, retry once, and no more than that.
    func testRetriesOnceAfterUnexpectedUnauthorized() async throws {
        let refreshed = """
        {
          "access_token": "access-2",
          "refresh_token": "refresh-2",
          "expires_at": "2099-01-01T00:00:00Z"
        }
        """
        let transport = StubTransport(replies: [
            .json("{}", status: 401),
            .json(refreshed),
            .json(profileJSON)
        ])
        let client = APIClient(
            configuration: configuration,
            transport: transport,
            tokenStore: InMemoryTokenStore(tokens: tokens(expiringIn: 600))
        )

        _ = try await client.send(.get("me/profile"), as: Profile.self)

        let sent = await transport.recordedRequests()
        XCTAssertEqual(sent.count, 3)
    }

    func testFailedRefreshSignsTheUserOutLocally() async {
        let transport = StubTransport(replies: [
            .json("{}", status: 401),
            .json("{}", status: 401)
        ])
        let store = InMemoryTokenStore(tokens: tokens(expiringIn: 600))
        let client = APIClient(configuration: configuration, transport: transport, tokenStore: store)

        do {
            _ = try await client.send(.get("me/profile"), as: Profile.self)
            XCTFail("La requête devait échouer")
        } catch {
            XCTAssertEqual(error as? APIError, .unauthorized)
        }
        XCTAssertNil(store.read())
    }

    func testSurfacesServerMessageFromErrorEnvelope() async {
        let transport = StubTransport(replies: [
            .json(#"{"message": "Cette adresse est déjà prise.", "code": "email_taken"}"#, status: 409)
        ])
        let client = APIClient(
            configuration: configuration,
            transport: transport,
            tokenStore: InMemoryTokenStore()
        )

        do {
            _ = try await client.send(
                .post("auth/sign-up", body: ["email": "a@b.fr"], requiresAuthentication: false),
                as: User.self
            )
            XCTFail("La requête devait échouer")
        } catch {
            XCTAssertEqual(
                error as? APIError,
                .server(status: 409, message: "Cette adresse est déjà prise.")
            )
            XCTAssertEqual(
                (error as? APIError)?.userMessage,
                "Cette adresse est déjà prise."
            )
        }
    }

    func testUnauthenticatedClientDoesNotReachTheNetwork() async {
        let transport = StubTransport(replies: [.json(profileJSON)])
        let client = APIClient(
            configuration: configuration,
            transport: transport,
            tokenStore: InMemoryTokenStore()
        )

        do {
            _ = try await client.send(.get("me/profile"), as: Profile.self)
            XCTFail("La requête devait échouer")
        } catch {
            XCTAssertEqual(error as? APIError, .unauthorized)
        }
        let sent = await transport.recordedRequests()
        XCTAssertTrue(sent.isEmpty)
    }

    // MARK: - Réessais

    /// A read that hits a struggling server should recover on its own rather
    /// than putting an error in front of someone.
    func testReadsAreRetriedUntilTheySucceed() async throws {
        let transport = StubTransport(replies: [
            .json("{}", status: 503),
            .json(profileJSON)
        ])
        let client = APIClient(
            configuration: configuration,
            transport: transport,
            tokenStore: InMemoryTokenStore(tokens: tokens(expiringIn: 600)),
            retryPolicy: APIClient.RetryPolicy(maximumAttempts: 3, baseDelay: .milliseconds(1))
        )

        _ = try await client.send(.get("me/profile"), as: Profile.self)

        let sent = await transport.recordedRequests()
        XCTAssertEqual(sent.count, 2)
    }

    /// Replaying a swipe would count it twice. Writes get exactly one attempt.
    func testWritesAreNeverReplayed() async {
        let transport = StubTransport(replies: [
            .json("{}", status: 503),
            .json("{}", status: 503),
            .json("{}", status: 503)
        ])
        let client = APIClient(
            configuration: configuration,
            transport: transport,
            tokenStore: InMemoryTokenStore(tokens: tokens(expiringIn: 600)),
            retryPolicy: APIClient.RetryPolicy(maximumAttempts: 3, baseDelay: .milliseconds(1))
        )

        let endpoint = Endpoint.post(
            "discovery/swipes",
            body: SwipeRequest(targetProfileId: UUID(), decision: .like)
        )
        _ = try? await client.send(endpoint, as: SwipeOutcome.self)

        let sent = await transport.recordedRequests()
        XCTAssertEqual(sent.count, 1, "Un swipe rejoué serait compté deux fois")
    }

    func testRetriesGiveUpAndSurfaceTheLastError() async {
        let transport = StubTransport(replies: [
            .json("{}", status: 503),
            .json("{}", status: 503),
            .json("{}", status: 503)
        ])
        let client = APIClient(
            configuration: configuration,
            transport: transport,
            tokenStore: InMemoryTokenStore(tokens: tokens(expiringIn: 600)),
            retryPolicy: APIClient.RetryPolicy(maximumAttempts: 3, baseDelay: .milliseconds(1))
        )

        do {
            _ = try await client.send(.get("me/profile"), as: Profile.self)
            XCTFail("La requête devait finir par échouer")
        } catch {
            XCTAssertEqual((error as? APIError)?.isRetryable, true)
        }
        let sent = await transport.recordedRequests()
        XCTAssertEqual(sent.count, 3, "Trois tentatives, puis on rend la main")
    }

    /// A 400 is an answer, not a hiccup: retrying it only wastes time.
    func testClientErrorsAreNotRetried() async {
        let transport = StubTransport(replies: [
            .json(#"{"message": "Requête invalide."}"#, status: 400),
            .json(profileJSON)
        ])
        let client = APIClient(
            configuration: configuration,
            transport: transport,
            tokenStore: InMemoryTokenStore(tokens: tokens(expiringIn: 600)),
            retryPolicy: APIClient.RetryPolicy(maximumAttempts: 3, baseDelay: .milliseconds(1))
        )

        _ = try? await client.send(.get("me/profile"), as: Profile.self)

        let sent = await transport.recordedRequests()
        XCTAssertEqual(sent.count, 1)
    }

    /// `singleAttempt` is what a caller reaches for when a retry would be
    /// wrong; it must really mean one.
    func testSingleAttemptPolicyNeverRetries() async {
        let transport = StubTransport(replies: [
            .json("{}", status: 503),
            .json(profileJSON)
        ])
        let client = APIClient(
            configuration: configuration,
            transport: transport,
            tokenStore: InMemoryTokenStore(tokens: tokens(expiringIn: 600)),
            retryPolicy: .singleAttempt
        )

        _ = try? await client.send(.get("me/profile"), as: Profile.self)

        let sent = await transport.recordedRequests()
        XCTAssertEqual(sent.count, 1)
    }

    func testBackoffGrowsBetweenAttempts() {
        let policy = APIClient.RetryPolicy(maximumAttempts: 3, baseDelay: .milliseconds(300))
        XCTAssertEqual(policy.delay(beforeAttempt: 1), .milliseconds(300))
        XCTAssertEqual(policy.delay(beforeAttempt: 2), .milliseconds(900))
    }

    func testOnlyReadsAreConsideredSafeToReplay() {
        XCTAssertTrue(Endpoint.get("matches").isRetryable)
        XCTAssertFalse(Endpoint.post("discovery/rewind").isRetryable)
        XCTAssertFalse(Endpoint.delete("matches/1").isRetryable)
        XCTAssertFalse(Endpoint.patch("me/profile", body: ProfileUpdate()).isRetryable)
    }

    func testSocketBackoffGrowsAndIsCapped() {
        XCTAssertEqual(ChatSocket.backoffDelay(forAttempt: 1), 1)
        XCTAssertEqual(ChatSocket.backoffDelay(forAttempt: 4), 8)
        XCTAssertEqual(ChatSocket.backoffDelay(forAttempt: 20), 30)
    }
}
