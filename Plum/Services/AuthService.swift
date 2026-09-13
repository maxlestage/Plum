import Foundation

protocol AuthServicing: Sendable {
    func signIn(email: String, password: String) async throws -> AuthenticatedSession
    func signUp(_ request: SignUpRequest) async throws -> AuthenticatedSession
    func currentUser() async throws -> User
    func signOut() async
    func deleteAccount() async throws
    /// A session restored from the keychain at launch, or nil if there is none.
    func restoreSession() async -> User?
    /// Emits when the server invalidates the session, so the UI can go back to
    /// the sign-in screen instead of sitting on a dead one.
    func sessionExpirations() async -> AsyncStream<Void>
}

struct AuthService: AuthServicing {
    let client: any APIClientProtocol

    init(client: any APIClientProtocol) {
        self.client = client
    }

    func signIn(email: String, password: String) async throws -> AuthenticatedSession {
        let endpoint = Endpoint.post(
            "auth/sign-in",
            body: SignInRequest(email: email, password: password),
            requiresAuthentication: false
        )
        let session = try await client.send(endpoint, as: AuthenticatedSession.self)
        await client.store(session.tokens)
        return session
    }

    func signUp(_ request: SignUpRequest) async throws -> AuthenticatedSession {
        let endpoint = Endpoint.post("auth/sign-up", body: request, requiresAuthentication: false)
        let session = try await client.send(endpoint, as: AuthenticatedSession.self)
        await client.store(session.tokens)
        return session
    }

    func currentUser() async throws -> User {
        try await client.send(.get("me"), as: User.self)
    }

    func signOut() async {
        // Best effort: the server revokes the refresh token, but a failure here
        // must not leave someone stuck in a session they asked to leave.
        _ = try? await client.send(.post("auth/sign-out"))
        await client.signOutLocally()
    }

    func deleteAccount() async throws {
        try await client.send(.delete("me"))
        await client.signOutLocally()
    }

    func restoreSession() async -> User? {
        guard await client.currentTokens() != nil else { return nil }
        return try? await currentUser()
    }

    func sessionExpirations() async -> AsyncStream<Void> {
        await client.sessionExpirations()
    }
}
