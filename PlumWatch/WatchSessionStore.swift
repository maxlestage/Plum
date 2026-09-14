import Foundation
import Observation
import WatchConnectivity

/// Ce que la montre sait du compte, et comment elle l'apprend.
///
/// Les identifiants arrivent du téléphone par WatchConnectivity et sont
/// gardés dans le trousseau de la montre — pas dans `UserDefaults`, où un
/// jeton d'accès n'a rien à faire.
///
/// Trois états, et l'écran doit savoir les distinguer : on n'a jamais reçu de
/// jeton, on en a un, ou celui qu'on avait vient d'être refusé. Le troisième
/// ressemble au premier et n'appelle pas le même message : « ouvrez Plum sur
/// votre iPhone » après une déconnexion est un conseil utile, avant la
/// première ouverture c'en est un autre.
@MainActor
@Observable
final class WatchSessionStore: NSObject {
    enum State: Equatable {
        case waiting
        case ready(WatchCredentials)
        case rejected
    }

    private(set) var state: State = .waiting

    private static let account = "plum.watch.credentials"

    override init() {
        super.init()
        if let gardees = Self.load() {
            state = .ready(gardees)
        }
        guard WCSession.isSupported() else { return }
        WCSession.default.delegate = self
        WCSession.default.activate()
    }

    /// Appelé quand le serveur a refusé le jeton.
    ///
    /// On oublie, on n'essaie pas de rafraîchir : le renouvellement vit dans
    /// l'application, avec le jeton de rafraîchissement que la montre n'a
    /// volontairement pas. Lui confier de quoi prolonger une session
    /// reviendrait à doubler l'endroit où un vol est possible.
    func forget() {
        Self.erase()
        state = .rejected
    }

    fileprivate func accept(_ credentials: WatchCredentials) {
        Self.save(credentials)
        state = .ready(credentials)
    }

    // MARK: - Le trousseau

    private static func save(_ credentials: WatchCredentials) {
        guard let data = try? JSONEncoder().encode(credentials) else { return }
        erase()
        let entree: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrAccount as String: account,
            kSecValueData as String: data,
            // La montre est déverrouillée quand on la porte ; ce niveau
            // suffit et évite de réclamer un code après chaque mise au
            // poignet.
            kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlock,
        ]
        SecItemAdd(entree as CFDictionary, nil)
    }

    private static func load() -> WatchCredentials? {
        let requete: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrAccount as String: account,
            kSecReturnData as String: true,
        ]
        var sortie: CFTypeRef?
        guard SecItemCopyMatching(requete as CFDictionary, &sortie) == errSecSuccess,
              let data = sortie as? Data
        else { return nil }
        return try? JSONDecoder().decode(WatchCredentials.self, from: data)
    }

    private static func erase() {
        SecItemDelete([
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrAccount as String: account,
        ] as CFDictionary)
    }
}

extension WatchSessionStore: WCSessionDelegate {
    nonisolated func session(
        _ session: WCSession,
        activationDidCompleteWith activationState: WCSessionActivationState,
        error: Error?
    ) {}

    nonisolated func session(_ session: WCSession, didReceiveMessage message: [String: Any]) {
        guard let credentials = WatchLink.decode(message) else { return }
        Task { @MainActor in self.accept(credentials) }
    }

    /// Le contexte, et pas seulement les messages directs.
    ///
    /// `sendMessage` exige que les deux appareils soient joignables à
    /// l'instant même ; `updateApplicationContext` dépose la dernière valeur
    /// et la délivre quand la montre se réveille. Sans ce second chemin, une
    /// montre laissée sur le chargeur pendant la connexion n'aurait jamais
    /// son jeton.
    nonisolated func session(
        _ session: WCSession,
        didReceiveApplicationContext applicationContext: [String: Any]
    ) {
        guard let credentials = WatchLink.decode(applicationContext) else { return }
        Task { @MainActor in self.accept(credentials) }
    }
}
