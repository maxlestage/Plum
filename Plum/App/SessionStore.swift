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
    private var expiryTask: Task<Void, Never>?

    init(auth: any AuthServicing) {
        self.auth = auth
    }

    var currentUserId: UUID? { state.user?.id }

    /// Called once at launch: restores a keychain session so returning users
    /// land straight on today's selection.
    func restore() async {
        watchForExpiry()
        if let user = await auth.restoreSession() {
            state = .signedIn(user)
        } else {
            state = .signedOut
        }
    }

    /// A refused refresh signs the client out on its own; without this the UI
    /// stayed on a dead session, piling up errors until someone force-quit.
    private func watchForExpiry() {
        guard expiryTask == nil else { return }
        expiryTask = Task { [weak self] in
            guard let self else { return }
            for await _ in await self.auth.sessionExpirations() {
                self.expire()
            }
        }
    }

    /// Signed out by the server, not by the person: no network call to make,
    /// the credentials are already gone.
    func expire() {
        currentProfile = nil
        state = .signedOut
    }

    func adopt(_ session: AuthenticatedSession) {
        adopt(session.user)
    }

    /// Re-reads the account, which is how finishing onboarding switches the
    /// root view over to the tabs.
    func adopt(_ user: User) {
        state = .signedIn(user)
    }

    /// A signed-in account that has not been through onboarding yet.
    var needsOnboarding: Bool {
        guard let user = state.user else { return false }
        return !user.profileCompleted
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
