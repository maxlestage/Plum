import Observation
import SwiftUI

/// Who is signed in, and how the rest of the app finds out.
///
/// Owned by ``PlumApp`` and handed down through the environment; it is the
/// single source of truth for the auth-gated navigation in ``RootView``.
@MainActor
@Observable
final class SessionStore {
    enum State: Equatable {
        case launching
        case signedOut
        case signedIn(User)

        var user: User? {
            if case let .signedIn(user) = self { return user }
            return nil
        }
    }

    private(set) var state: State = .launching
    var currentProfile: Profile?

    private let auth: any AuthServicing

    init(auth: any AuthServicing) {
        self.auth = auth
    }

    var currentUserId: UUID? { state.user?.id }

    /// Called once at launch: restores a keychain session so returning users
    /// land straight on the deck.
    func restore() async {
        if let user = await auth.restoreSession() {
            state = .signedIn(user)
        } else {
            state = .signedOut
        }
    }

    func adopt(_ session: AuthenticatedSession) {
        state = .signedIn(session.user)
    }

    func signOut() async {
        await auth.signOut()
        currentProfile = nil
        state = .signedOut
    }

    func deleteAccount() async throws {
        try await auth.deleteAccount()
        currentProfile = nil
        state = .signedOut
    }
}
