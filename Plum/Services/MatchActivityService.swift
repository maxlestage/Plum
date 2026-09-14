import ActivityKit
import Foundation

/// Démarre, met à jour et termine la Live Activity d'un match.
///
/// Tout est silencieux par conception. Une Live Activity est un agrément :
/// si le système la refuse — l'utilisateur les a désactivées, il y en a déjà
/// trop, l'appareil est trop ancien — l'application doit continuer comme si
/// de rien n'était. Remonter ces échecs à l'écran reviendrait à interrompre
/// quelqu'un pour lui parler d'une décoration.
///
/// **Ce que ça ne fait pas, et il faut le dire.** Sans certificat APNs, une
/// activité ne se met à jour que pendant que l'application tourne. Une fois
/// l'écran verrouillé, le compteur continue d'avancer — c'est le système qui
/// l'anime à partir d'une date — mais « conversation commencée » n'arrivera
/// qu'au prochain lancement. Le jour où les notifications poussées existent,
/// c'est ici qu'on branchera le jeton de poussée.
protocol MatchActivityPresenting: Sendable {
    func begin(matchId: UUID, displayName: String, conversationId: UUID?, matchedAt: Date) async
    func end(matchId: UUID) async
}

/// L'implémentation réelle, adossée à ActivityKit.
struct MatchActivityService: MatchActivityPresenting {
    /// Au-delà, on ne démarre plus rien.
    ///
    /// Le système accepte plusieurs activités par application et les empile
    /// sur l'écran verrouillé. Trois matchs frais, c'est une information ;
    /// dix, c'est un écran illisible que la personne finira par désactiver
    /// pour toute l'application — et elle ne le réactivera pas.
    static let maximumLive = 3

    /// Au-delà, l'activité s'efface d'elle-même.
    ///
    /// Six heures : un match qu'on n'a pas ouvert de la demi-journée n'est
    /// plus une nouvelle, et laisser la carte traîner ferait de l'écran
    /// verrouillé un cimetière. Le système finirait par l'enlever après huit
    /// heures de toute façon ; mieux vaut choisir le moment que le subir.
    static let lifetime: TimeInterval = 6 * 60 * 60

    func begin(matchId: UUID, displayName: String, conversationId: UUID?, matchedAt: Date) async {
        guard ActivityAuthorizationInfo().areActivitiesEnabled else { return }

        let live = Activity<MatchActivityAttributes>.activities
        guard live.count < Self.maximumLive else { return }
        // Déjà là : un second `request` créerait une carte de plus pour le
        // même match, et rien ne les distinguerait sur l'écran verrouillé.
        guard !live.contains(where: { $0.attributes.matchId == matchId }) else { return }

        let attributes = MatchActivityAttributes(
            displayName: displayName,
            conversationId: conversationId,
            matchId: matchId
        )
        let state = MatchActivityAttributes.ContentState(matchedAt: matchedAt, hasStarted: false)

        _ = try? Activity.request(
            attributes: attributes,
            content: .init(state: state, staleDate: matchedAt.addingTimeInterval(Self.lifetime)),
            pushType: nil
        )
    }

    func end(matchId: UUID) async {
        guard let activity = Self.activity(for: matchId) else { return }
        // `.after` et non `.immediate` : la carte reste quelques secondes en
        // « conversation commencée ». Escamoter une carte à l'instant précis
        // où l'on appuie dessus donne l'impression d'avoir raté son geste.
        let state = MatchActivityAttributes.ContentState(
            matchedAt: activity.content.state.matchedAt,
            hasStarted: true
        )
        await activity.end(
            .init(state: state, staleDate: nil),
            dismissalPolicy: .after(.now.addingTimeInterval(4))
        )
    }

    private static func activity(for matchId: UUID) -> Activity<MatchActivityAttributes>? {
        Activity<MatchActivityAttributes>.activities.first { $0.attributes.matchId == matchId }
    }
}

/// Celle des aperçus et des tests : elle enregistre au lieu d'afficher.
///
/// ActivityKit ne fonctionne pas dans un test unitaire — il lui faut un
/// système vivant. Sans cette version, la logique qui décide *quand* démarrer
/// une activité ne serait vérifiée par rien, et c'est elle qui compte.
actor RecordingMatchActivityService: MatchActivityPresenting {
    private(set) var begun: [UUID] = []
    private(set) var ended: [UUID] = []

    func begin(matchId: UUID, displayName: String, conversationId: UUID?, matchedAt: Date) async {
        begun.append(matchId)
    }

    func end(matchId: UUID) async {
        ended.append(matchId)
    }
}
