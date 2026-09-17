import Foundation

/// Quelqu'un qu'on a bloqué.
///
/// Bloquer doit être facile à faire quand on se sent menacé — donc facile à
/// faire trop vite, sur un doute. Sans cette liste le geste serait définitif et
/// muet : la personne a disparu de partout, il n'y a nulle part où la
/// retrouver, et rien à l'écran ne dirait même qui on a bloqué.
struct BlockedPerson: Decodable, Sendable, Identifiable, Equatable {
    let id: UUID
    /// Absent quand le compte est parti depuis. L'écran dit « Compte
    /// supprimé » plutôt qu'une ligne vide qui ressemble à un bogue.
    let displayName: String?
    let blockedAt: Date

    /// Ce qu'on affiche, compte fermé ou non.
    var label: String { displayName ?? "Compte supprimé" }
}
