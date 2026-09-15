import Observation
import SwiftUI

/// Tient la sélection du jour : ce qui reste à décider, et ce qui a été fait.
@MainActor
@Observable
final class DiscoveryViewModel {
    private(set) var profiles: [Profile] = []
    private(set) var state: ActivityState = .idle
    /// Ce que la sélection contient quand elle est pleine. `nil` tant qu'on
    /// n'a rien chargé.
    private(set) var size: Int?
    /// Quand la prochaine sélection sera tirée.
    private(set) var refreshesAt: Date?
    /// Le fil qui vient d'être ouvert, pour proposer d'y aller.
    private(set) var justOpened: OpenedThread?
    /// Ce que l'écran doit dire quand un geste n'a pas abouti. `nil` tant
    /// qu'il n'y a rien à dire.
    private(set) var safetyFailure: String?

    /// Un fil qui vient de s'ouvrir, et de quoi y emmener.
    struct OpenedThread: Identifiable, Equatable {
        let id: UUID
        let displayName: String
    }

    private var isFetching = false
    private let discovery: any DiscoveryServicing

    init(discovery: any DiscoveryServicing) {
        self.discovery = discovery
    }

    /// Combien il reste à décider sur les trois du jour.
    var remaining: Int { profiles.count }

    var isEmpty: Bool {
        profiles.isEmpty && !state.isLoading && state.isReady
    }

    // MARK: - Chargement

    func load() async {
        guard !isFetching else { return }
        if profiles.isEmpty { state = .loading }
        isFetching = true
        defer { isFetching = false }

        do {
            let selection = try await discovery.selection()
            profiles = selection.items
            size = selection.size
            refreshesAt = selection.refreshesAt
            state = .ready
        } catch {
            state = .failed(error.asAPIError)
        }
    }

    // MARK: - Les deux issues

    /// Écrire ouvre le fil tout de suite : il n'y a personne à attendre.
    ///
    /// La carte ne part qu'une fois le message parti, à l'inverse du balayage
    /// qu'elle remplace. Un balayage se voyait à l'écran avant d'atteindre le
    /// serveur parce qu'il fallait que le geste paraisse instantané ; un
    /// message qu'on croit envoyé et qui ne l'est pas est une autre affaire.
    func write(to profile: Profile, body: String) async -> Bool {
        let propre = body.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !propre.isEmpty else { return false }

        do {
            let message = try await discovery.write(profileId: profile.id, body: propre)
            profiles.removeAll { $0.id == profile.id }
            justOpened = OpenedThread(
                id: message.conversationId,
                displayName: profile.displayName
            )
            Haptics.play(.success)
            return true
        } catch {
            safetyFailure = "Le message n'est pas parti. Vérifiez votre connexion et réessayez."
            return false
        }
    }

    /// Laisser passer. Définitif, et l'écran le dit avant de le faire.
    func pass(_ profile: Profile) async {
        await applySafely(
            to: profile,
            notice: "Ce profil n'a pas été écarté. Vérifiez votre connexion et réessayez."
        ) {
            try await self.discovery.pass(profileId: profile.id)
        }
    }

    /// Signalement et blocage : la carte part tout de suite, mais elle revient
    /// si le serveur n'a rien enregistré.
    ///
    /// Le `try?` d'avant avalait l'échec. La carte disparaissait, l'utilisateur
    /// croyait la personne écartée, et elle réapparaissait au prochain
    /// chargement sans explication. Sur un geste de sécurité, c'est le pire des
    /// silences : on laisse quelqu'un se croire protégé alors qu'il ne l'est
    /// pas. Ici l'échec se voit, et le geste peut être refait.
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

    func dismissOpenedThread() {
        justOpened = nil
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
            // qu'elle avait, sinon un échec réordonnerait la sélection.
            profiles.insert(profile, at: min(wasAt ?? 0, profiles.count))
            safetyFailure = notice
        }
    }
}
