import Foundation

/// Ce que le téléphone envoie à la montre pour qu'elle puisse travailler
/// seule.
///
/// **Pourquoi la montre parle au serveur plutôt qu'au téléphone.** Relayer
/// chaque requête par le téléphone paraît plus simple et ne l'est pas : une
/// montre cellulaire qu'on a emportée en courant n'a plus son téléphone à
/// portée, et une application qui montre alors un écran vide est pire qu'une
/// application absente. Le téléphone ne transmet donc qu'un jeton, une fois,
/// et la montre se débrouille.
///
/// **Pourquoi pas une connexion depuis la montre.** Taper une adresse et un
/// mot de passe sur un écran de quarante millimètres est une épreuve. Le
/// téléphone s'est déjà connecté ; il passe le relais.
struct WatchCredentials: Codable, Hashable, Sendable {
    /// Le jeton d'accès, tel que l'API l'attend.
    var accessToken: String

    /// Qui est connecté, pour que la montre sache quelles bulles sont les
    /// siennes sans redemander le profil.
    var currentUserId: UUID

    /// L'adresse du serveur, transmise plutôt que recompilée.
    ///
    /// Elle évite d'avoir deux constantes à changer le jour d'un changement
    /// d'hébergement — et surtout d'en oublier une, ce qui donnerait une
    /// montre qui parle dans le vide pendant qu'on cherche pourquoi.
    var baseURL: URL

    init(accessToken: String, currentUserId: UUID, baseURL: URL) {
        self.accessToken = accessToken
        self.currentUserId = currentUserId
        self.baseURL = baseURL
    }
}

/// Les clés du dictionnaire échangé par WatchConnectivity.
///
/// `WCSession` ne transporte que des types de liste de propriétés, pas des
/// `Codable`. On encode donc en JSON sous une clé unique, et la version dans
/// le nom de la clé permettra à une montre restée en arrière de reconnaître
/// qu'elle ne comprend pas ce qu'on lui envoie — au lieu de décoder à moitié.
enum WatchLink {
    static let payloadKey = "plum.credentials.v1"

    static func encode(_ credentials: WatchCredentials) -> [String: Any] {
        guard let data = try? JSONEncoder().encode(credentials) else { return [:] }
        return [payloadKey: data]
    }

    static func decode(_ message: [String: Any]) -> WatchCredentials? {
        guard let data = message[payloadKey] as? Data else { return nil }
        return try? JSONDecoder().decode(WatchCredentials.self, from: data)
    }
}
