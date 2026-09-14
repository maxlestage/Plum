import Observation
import SwiftUI

/// Owns the deck: what is on screen, what has been decided, and when to go
/// back to the server for more.
@MainActor
@Observable
final class DiscoveryViewModel {
    private(set) var profiles: [Profile] = []
    private(set) var state: ActivityState = .idle
    private(set) var newMatch: Match?
    private(set) var likesRemaining: Int?
    private(set) var canRewind = false
    /// Ce que l'écran doit dire quand un blocage ou un signalement n'a pas
    /// abouti. `nil` tant qu'il n'y a rien à dire.
    private(set) var safetyFailure: String?

    /// Refill once the deck gets this short, so the stack never visibly runs
    /// dry mid-session.
    private static let prefetchThreshold = 4

    private var cursor: String?
    private var isFetching = false
    private let discovery: any DiscoveryServicing
    /// Ce qui pose la carte du match sur l'écran verrouillé.
    ///
    /// Injecté plutôt qu'appelé directement : ActivityKit ne fonctionne pas
    /// dans un test unitaire — il lui faut un système vivant. Sans cette
    /// couture, la logique qui décide *quand* démarrer une activité ne serait
    /// vérifiée par rien, et c'est elle qui compte.
    private let activities: any MatchActivityPresenting

    init(
        discovery: any DiscoveryServicing,
        activities: any MatchActivityPresenting = MatchActivityService()
    ) {
        self.discovery = discovery
        self.activities = activities
    }

    var topProfile: Profile? { profiles.first }

    /// The two or three cards drawn behind the top one.
    var visibleProfiles: [Profile] {
        Array(profiles.prefix(PlumTheme.Layout.cardStackDepth))
    }

    var isEmpty: Bool {
        profiles.isEmpty && !state.isLoading
    }

    // MARK: - Loading

    func loadInitialDeck() async {
        guard profiles.isEmpty, !isFetching else { return }
        state = .loading
        await fetchMore(reset: true)
    }

    func refresh() async {
        cursor = nil
        await fetchMore(reset: true)
    }

    private func fetchMore(reset: Bool) async {
        guard !isFetching else { return }
        isFetching = true
        defer { isFetching = false }

        do {
            let page = try await discovery.deck(cursor: reset ? nil : cursor, limit: 20)
            let known = Set(profiles.map(\.id))
            let fresh = page.items.filter { !known.contains($0.id) }
            profiles = reset ? page.items : profiles + fresh
            cursor = page.nextCursor
            state = .ready
        } catch {
            state = .failed(error.asAPIError)
        }
    }

    // MARK: - Swiping

    func swipe(_ profile: Profile, decision: SwipeDecision) async {
        // Remove first: the card is already flying off screen, and waiting for
        // the network before updating the stack would feel broken.
        profiles.removeAll { $0.id == profile.id }
        canRewind = decision == .pass

        Haptics.play(decision == .superLike ? .success : .light)

        do {
            let outcome = try await discovery.swipe(profileId: profile.id, decision: decision)
            likesRemaining = outcome.likesRemaining
            if outcome.matched, let match = outcome.match {
                Haptics.play(.success)
                newMatch = match
                // La carte part avec la date du serveur, pas `.now` : c'est
                // elle qui alimente le compteur, et deux horloges qui
                // divergent donneraient un « il y a 3 min » au moment même du
                // match.
                await activities.begin(
                    matchId: match.id,
                    displayName: match.profile.displayName,
                    conversationId: match.conversationId,
                    matchedAt: match.matchedAt
                )
            }
        } catch {
            // A failed swipe is not worth interrupting the session for; the
            // card stays gone and the server reconciles on the next deck.
            state = .failed(error.asAPIError)
        }

        if profiles.count <= Self.prefetchThreshold {
            await fetchMore(reset: profiles.isEmpty && cursor == nil)
        }
    }

    func rewind() async {
        guard canRewind else { return }
        canRewind = false
        do {
            if let profile = try await discovery.rewind() {
                profiles.insert(profile, at: 0)
                Haptics.play(.light)
            }
        } catch {
            state = .failed(error.asAPIError)
        }
    }

    /// Signalement et blocage : la carte part tout de suite, mais elle revient
    /// si le serveur n'a rien enregistré.
    ///
    /// Le `try?` d'avant avalait l'échec. La carte disparaissait, l'utilisateur
    /// croyait la personne écartée, et elle réapparaissait au prochain
    /// chargement du deck sans explication. Sur un geste de sécurité, c'est le
    /// pire des silences : on laisse quelqu'un se croire protégé alors qu'il ne
    /// l'est pas. Ici l'échec se voit, et le geste peut être refait.
    func report(_ profile: Profile, reason: String) async {
        await applySafely(
            to: profile,
            notice: "Le signalement n'est pas parti. Vérifiez votre connexion et réessayez."
        ) {
            try await self.discovery.report(profileId: profile.id, reason: reason)
        }
    }

    func block(_ profile: Profile) async {
        await applySafely(
            to: profile,
            notice: "Le blocage n'a pas été enregistré. Vérifiez votre connexion et réessayez."
        ) {
            try await self.discovery.block(profileId: profile.id)
        }
    }

    func dismissSafetyFailure() {
        safetyFailure = nil
    }

    private func applySafely(
        to profile: Profile,
        notice: String,
        _ action: () async throws -> Void
    ) async {
        let wasAt = profiles.firstIndex { $0.id == profile.id }
        profiles.removeAll { $0.id == profile.id }
        do {
            try await action()
        } catch {
            // Remise à sa place, pas en tête : la carte reprend le rang
            // qu'elle avait, sinon un échec réordonnerait le deck.
            profiles.insert(profile, at: min(wasAt ?? 0, profiles.count))
            safetyFailure = notice
        }
    }

    func dismissMatch() {
        newMatch = nil
    }
}
