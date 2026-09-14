import Foundation
import WatchConnectivity

/// Le téléphone qui passe le relais à la montre.
///
/// Il n'envoie qu'une chose — un jeton d'accès, l'identifiant du compte et
/// l'adresse du serveur — et il l'envoie deux fois, par deux chemins qui ne
/// couvrent pas les mêmes cas :
///
/// - `sendMessage` arrive tout de suite, mais seulement si la montre est
///   joignable à l'instant même ;
/// - `updateApplicationContext` dépose la dernière valeur connue et la
///   délivre au réveil de la montre.
///
/// Sans le second, une montre laissée sur son chargeur pendant la connexion
/// n'aurait jamais son jeton, et l'utilisateur verrait « ouvrez Plum sur
/// votre iPhone » alors qu'il vient précisément de le faire.
///
/// **Ce qui ne part pas : le jeton de rafraîchissement.** La montre ne peut
/// donc pas prolonger une session toute seule — quand le sien expire, elle
/// oublie et attend le téléphone. C'est volontaire : lui confier de quoi
/// renouveler indéfiniment doublerait l'endroit d'où une session peut être
/// volée, pour économiser une ouverture d'application.
@MainActor
final class WatchBridge: NSObject {
    private let configuration: APIConfiguration
    private var lastSent: WatchCredentials?

    init(configuration: APIConfiguration) {
        self.configuration = configuration
        super.init()
        guard WCSession.isSupported() else { return }
        WCSession.default.delegate = self
        WCSession.default.activate()
    }

    /// À appeler quand la session change : connexion, renouvellement,
    /// déconnexion.
    func publish(accessToken: String?, currentUserId: UUID?) {
        guard WCSession.isSupported() else { return }

        guard let accessToken, let currentUserId else {
            // Déconnexion : on efface plutôt que de laisser la montre vivre
            // sur un jeton dont le téléphone ne veut plus.
            lastSent = nil
            try? WCSession.default.updateApplicationContext([:])
            return
        }

        let credentials = WatchCredentials(
            accessToken: accessToken,
            currentUserId: currentUserId,
            baseURL: configuration.baseURL
        )
        // Rien à faire si c'est le même : `updateApplicationContext` réveille
        // la montre, et la réveiller pour lui répéter ce qu'elle sait coûte
        // de la batterie à quelqu'un qui n'a rien demandé.
        guard credentials != lastSent else { return }
        lastSent = credentials

        let charge = WatchLink.encode(credentials)
        try? WCSession.default.updateApplicationContext(charge)
        if WCSession.default.isReachable {
            WCSession.default.sendMessage(charge, replyHandler: nil)
        }
    }
}

extension WatchBridge: WCSessionDelegate {
    nonisolated func session(
        _ session: WCSession,
        activationDidCompleteWith activationState: WCSessionActivationState,
        error: Error?
    ) {}

    // Requis sur iOS : une montre peut être remplacée par une autre.
    nonisolated func sessionDidBecomeInactive(_ session: WCSession) {}

    nonisolated func sessionDidDeactivate(_ session: WCSession) {
        // Réactiver tout de suite, sinon le lien reste mort jusqu'au
        // prochain lancement de l'application.
        WCSession.default.activate()
    }
}
