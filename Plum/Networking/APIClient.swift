import Foundation
import OSLog

/// Everything the app needs from the network layer. Services depend on this,
/// not on ``APIClient``, so they can be exercised against a stub.
protocol APIClientProtocol: Sendable {
    func send<Response: Decodable>(_ endpoint: Endpoint, as type: Response.Type) async throws -> Response
    func send(_ endpoint: Endpoint) async throws
    func upload(_ data: Data, filename: String, to path: String) async throws -> Photo
    func currentTokens() async -> AuthTokens?
    func store(_ tokens: AuthTokens) async
    func signOutLocally() async
    /// Emits when the server has invalidated the session — a refused refresh,
    /// a revoked token. Without it the client signs itself out and the screen
    /// never finds out, leaving someone stranded on a dead session.
    func sessionExpirations() async -> AsyncStream<Void>
}

/// The one object that talks to the Rust API. An actor because the token pair
/// it guards is mutated from every feature at once.
actor APIClient: APIClientProtocol {
    /// How a dropped connection or a struggling server is handled: a couple of
    /// quick retries on reads, then give up and let the screen say so.
    struct RetryPolicy: Sendable {
        var maximumAttempts: Int
        var baseDelay: Duration

        static let `default` = RetryPolicy(maximumAttempts: 3, baseDelay: .milliseconds(300))
        static let singleAttempt = RetryPolicy(maximumAttempts: 1, baseDelay: .zero)

        /// 300ms, 900ms… a server that just rejected us deserves a pause, and
        /// a person waiting deserves an answer before they give up.
        func delay(beforeAttempt attempt: Int) -> Duration {
            baseDelay * Int(pow(3.0, Double(max(0, attempt - 1))))
        }
    }

    private let configuration: APIConfiguration
    private let transport: any HTTPTransport
    private let tokenStore: any TokenStoring
    private let retryPolicy: RetryPolicy
    private let decoder = JSONDecoder.plum
    private let encoder = JSONEncoder.plum
    private let logger = Logger(subsystem: "app.plum", category: "api")

    private var tokens: AuthTokens?
    private var expirySubscribers: [UUID: AsyncStream<Void>.Continuation] = [:]
    /// Holds the in-flight refresh so ten parallel 401s cause one refresh call
    /// rather than ten, and nine of them invalidating each other.
    private var refreshTask: Task<AuthTokens, Error>?

    init(
        configuration: APIConfiguration,
        transport: any HTTPTransport = URLSessionTransport(),
        tokenStore: any TokenStoring = KeychainTokenStore(),
        retryPolicy: RetryPolicy = .default
    ) {
        self.configuration = configuration
        self.transport = transport
        self.tokenStore = tokenStore
        self.retryPolicy = retryPolicy
        self.tokens = tokenStore.read()
    }

    // MARK: - Session

    func currentTokens() async -> AuthTokens? { tokens }

    func store(_ tokens: AuthTokens) async {
        self.tokens = tokens
        tokenStore.write(tokens)
    }

    func signOutLocally() async {
        clearCredentials()
    }

    func sessionExpirations() async -> AsyncStream<Void> {
        let id = UUID()
        let (stream, continuation) = AsyncStream<Void>.makeStream()
        continuation.onTermination = { [weak self] _ in
            Task { await self?.removeExpirySubscriber(id) }
        }
        expirySubscribers[id] = continuation
        return stream
    }

    private func removeExpirySubscriber(_ id: UUID) {
        expirySubscribers[id] = nil
    }

    private func clearCredentials() {
        tokens = nil
        refreshTask?.cancel()
        refreshTask = nil
        tokenStore.clear()
    }

    /// Signing out because the server said so, rather than because someone
    /// asked: the difference is that this one has to reach the UI.
    private func expireSession() {
        clearCredentials()
        for continuation in expirySubscribers.values {
            continuation.yield(())
        }
    }

    // MARK: - Requests

    func send<Response: Decodable>(_ endpoint: Endpoint, as type: Response.Type) async throws -> Response {
        let data = try await perform(endpoint, allowingRefresh: true)
        do {
            return try decoder.decode(Response.self, from: data)
        } catch {
            logger.error("Décodage impossible pour \(endpoint.path, privacy: .public): \(error.localizedDescription, privacy: .public)")
            throw APIError.decoding(String(describing: error))
        }
    }

    func send(_ endpoint: Endpoint) async throws {
        _ = try await perform(endpoint, allowingRefresh: true)
    }

    private func perform(_ endpoint: Endpoint, allowingRefresh: Bool) async throws -> Data {
        guard endpoint.isRetryable else {
            return try await performOnce(endpoint, allowingRefresh: allowingRefresh)
        }

        var lastError: APIError = .invalidResponse
        for attempt in 1...max(1, retryPolicy.maximumAttempts) {
            do {
                return try await performOnce(endpoint, allowingRefresh: allowingRefresh)
            } catch let error as APIError where error.isRetryable {
                lastError = error
                guard attempt < retryPolicy.maximumAttempts else { break }
                // A 429 tells us exactly how long to wait; anything else gets
                // the backoff.
                let wait: Duration
                if case let .rateLimited(retryAfter) = error, let retryAfter {
                    wait = .seconds(retryAfter)
                } else {
                    wait = retryPolicy.delay(beforeAttempt: attempt)
                }
                logger.notice(
                    "Nouvelle tentative \(attempt + 1) pour \(endpoint.path, privacy: .public)"
                )
                try? await Task.sleep(for: wait)
            }
        }
        throw lastError
    }

    private func performOnce(_ endpoint: Endpoint, allowingRefresh: Bool) async throws -> Data {
        var accessToken: String?
        if endpoint.requiresAuthentication {
            accessToken = try await validAccessToken()
        }

        let request = try endpoint.urlRequest(
            configuration: configuration,
            accessToken: accessToken,
            encoder: encoder
        )
        let (data, response) = try await transport.data(for: request)

        switch response.statusCode {
        case 200..<300:
            return data

        case 401 where endpoint.requiresAuthentication && allowingRefresh:
            // The access token was rejected even though it looked fresh; one
            // forced refresh, then a single retry.
            _ = try await refreshTokens(force: true)
            return try await performOnce(endpoint, allowingRefresh: false)

        case 401:
            expireSession()
            throw APIError.unauthorized

        case 404:
            throw APIError.notFound

        case 429:
            let retryAfter = response.value(forHTTPHeaderField: "Retry-After").flatMap(TimeInterval.init)
            throw APIError.rateLimited(retryAfter: retryAfter)

        default:
            let message = (try? decoder.decode(APIErrorBody.self, from: data))?.message
                ?? HTTPURLResponse.localizedString(forStatusCode: response.statusCode)
            throw APIError.server(status: response.statusCode, message: message)
        }
    }

    // MARK: - Token refresh

    private func validAccessToken() async throws -> String {
        guard let tokens else { throw APIError.unauthorized }
        if tokens.isExpired() {
            return try await refreshTokens(force: false).accessToken
        }
        return tokens.accessToken
    }

    @discardableResult
    private func refreshTokens(force: Bool) async throws -> AuthTokens {
        if let refreshTask {
            return try await refreshTask.value
        }
        guard let current = tokens else { throw APIError.unauthorized }
        if !force, !current.isExpired() {
            return current
        }

        let task = Task<AuthTokens, Error> { [configuration, transport, decoder, encoder] in
            let endpoint = Endpoint.post(
                "auth/refresh",
                body: RefreshRequest(refreshToken: current.refreshToken),
                requiresAuthentication: false
            )
            let request = try endpoint.urlRequest(configuration: configuration, encoder: encoder)
            let (data, response) = try await transport.data(for: request)
            guard (200..<300).contains(response.statusCode) else {
                throw APIError.unauthorized
            }
            return try decoder.decode(AuthTokens.self, from: data)
        }
        refreshTask = task

        do {
            let refreshed = try await task.value
            refreshTask = nil
            await store(refreshed)
            return refreshed
        } catch {
            refreshTask = nil
            expireSession()
            throw APIError.unauthorized
        }
    }

    // MARK: - Uploads

    /// Multipart upload for profile photos. Hand-rolled because a single
    /// endpoint does not justify a dependency.
    func upload(_ data: Data, filename: String, to path: String) async throws -> Photo {
        let accessToken = try await validAccessToken()
        let boundary = "plum.\(UUID().uuidString)"
        let trimmed = path.hasPrefix("/") ? String(path.dropFirst()) : path

        var request = URLRequest(
            url: configuration.baseURL.appendingPathComponent(trimmed),
            timeoutInterval: 60
        )
        request.httpMethod = HTTPMethod.post.rawValue
        request.setValue("multipart/form-data; boundary=\(boundary)", forHTTPHeaderField: "Content-Type")
        request.setValue("Bearer \(accessToken)", forHTTPHeaderField: "Authorization")
        request.httpBody = Self.multipartBody(data: data, filename: filename, boundary: boundary)

        let (responseData, response) = try await transport.data(for: request)
        guard (200..<300).contains(response.statusCode) else {
            let message = (try? decoder.decode(APIErrorBody.self, from: responseData))?.message
                ?? "Envoi de la photo impossible."
            throw APIError.server(status: response.statusCode, message: message)
        }
        return try decoder.decode(Photo.self, from: responseData)
    }

    static func multipartBody(data: Data, filename: String, boundary: String) -> Data {
        var body = Data()
        let disposition = "Content-Disposition: form-data; name=\"photo\"; filename=\"\(filename)\"\r\n"
        body.append(Data("--\(boundary)\r\n".utf8))
        body.append(Data(disposition.utf8))
        body.append(Data("Content-Type: image/jpeg\r\n\r\n".utf8))
        body.append(data)
        body.append(Data("\r\n--\(boundary)--\r\n".utf8))
        return body
    }
}
