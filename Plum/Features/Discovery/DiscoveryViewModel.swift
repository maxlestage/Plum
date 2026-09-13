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

    /// Refill once the deck gets this short, so the stack never visibly runs
    /// dry mid-session.
    private static let prefetchThreshold = 4

    private var cursor: String?
    private var isFetching = false
    private let discovery: any DiscoveryServicing

    init(discovery: any DiscoveryServicing) {
        self.discovery = discovery
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

    func report(_ profile: Profile, reason: String) async {
        profiles.removeAll { $0.id == profile.id }
        try? await discovery.report(profileId: profile.id, reason: reason)
    }

    func block(_ profile: Profile) async {
        profiles.removeAll { $0.id == profile.id }
        try? await discovery.block(profileId: profile.id)
    }

    func dismissMatch() {
        newMatch = nil
    }
}
