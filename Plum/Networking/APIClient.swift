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
}

/// The one object that talks to the Rust API. An actor because the token pair
/// it guards is mutated from every feature at once.
actor APIClient: APIClientProtocol {
    private let configuration: APIConfiguration
    private let transport: any HTTPTransport
    private let tokenStore: any TokenStoring
    private let decoder = JSONDecoder.plum
    private let encoder = JSONEncoder.plum
    private let logger = Logger(subsystem: "app.plum", category: "api")

    private var tokens: AuthTokens?
    /// Holds the in-flight refresh so ten parallel 401s cause one refresh call
    /// rather than ten, and nine of them invalidating each other.
    private var refreshTask: Task<AuthTokens, Error>?

    init(
        configuration: APIConfiguration,
        transport: any HTTPTransport = URLSessionTransport(),
        tokenStore: any TokenStoring = KeychainTokenStore()
    ) {
        self.configuration = configuration
        self.transport = transport
        self.tokenStore = tokenStore
        self.tokens = tokenStore.read()
    }

    // MARK: - Session

    func currentTokens() async -> AuthTokens? { tokens }

    func store(_ tokens: AuthTokens) async {
        self.tokens = tokens
        tokenStore.write(tokens)
    }

    func signOutLocally() async {
        tokens = nil
        refreshTask?.cancel()
        refreshTask = nil
        tokenStore.clear()
    }

    // MARK: - Requests

    func send<Response: Decodable>(_ endpoint: Endpoint, as type: Response.Type) async throws -> Response {
        let data = try await perform(endpoint, allowingRefresh: true)
        if Response.self == EmptyResponse.self, data.isEmpty {
            // A 204 has no body to decode, but callers still expect a value.
            return EmptyResponse() as! Response
        }
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
            return try await perform(endpoint, allowingRefresh: false)

        case 401:
            await signOutLocally()
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
            await signOutLocally()
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
