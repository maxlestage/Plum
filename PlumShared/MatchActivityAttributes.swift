import ActivityKit
import Foundation

/// Ce qu'une Live Activity de Plum montre, et ce qui en change.
///
/// Ce fichier est compilé dans l'application *et* dans l'extension : l'une
/// démarre l'activité, l'autre la dessine. Les deux doivent s'accorder au
/// champ près, sinon l'activité se lance et n'affiche rien — sans erreur de
/// compilation, puisque ce sont deux modules.
///
/// **Pourquoi un match, et pas un message.** Une Live Activity est faite pour
/// un évènement qui dure et qui se termine : une livraison, un match de
/// football, un minuteur. Une conversation n'a ni début ni fin. Un match, si :
/// il naît à une seconde précise, il perd sa valeur avec le temps — dans une
/// application de rencontres, le premier message part dans l'heure ou ne part
/// jamais — et il se clôt dès qu'on écrit.
///
/// C'est donc le seul évènement de Plum dont la forme corresponde à l'outil.
/// Le choix reste discutable ; il est écrit ici pour pouvoir l'être.
struct MatchActivityAttributes: ActivityAttributes {
    /// Ce qui bouge pendant que l'activité vit.
    public struct ContentState: Codable, Hashable {
        /// Quand le match a eu lieu.
        ///
        /// La date plutôt qu'une durée déjà calculée : le système sait
        /// afficher un compteur qui avance tout seul à partir d'une date, et
        /// c'est la seule façon d'avoir un temps juste sans rafraîchir. Une
        /// durée figée vieillirait sous les yeux de la personne.
        public var matchedAt: Date

        /// Vrai dès que l'un des deux a écrit.
        ///
        /// L'activité ne disparaît pas aussitôt : elle passe à « conversation
        /// commencée » avant de s'effacer. Escamoter une carte au moment
        /// précis où l'on appuie dessus donne l'impression d'avoir raté son
        /// geste.
        public var hasStarted: Bool

        public init(matchedAt: Date, hasStarted: Bool) {
            self.matchedAt = matchedAt
            self.hasStarted = hasStarted
        }
    }

    /// Le prénom affiché. Rien d'autre du profil ne voyage.
    ///
    /// Une Live Activity s'affiche sur un écran verrouillé, donc visible par
    /// qui tient le téléphone sans l'avoir déverrouillé. Le prénom suffit à
    /// savoir de qui il s'agit ; l'âge, la ville ou la photo n'y ajouteraient
    /// que de l'exposition.
    public var displayName: String

    /// L'identifiant de la conversation, pour que le geste ouvre le bon fil.
    public var conversationId: UUID?

    /// L'identifiant du match, qui sert de clé à l'activité.
    public var matchId: UUID

    public init(displayName: String, conversationId: UUID?, matchId: UUID) {
        self.displayName = displayName
        self.conversationId = conversationId
        self.matchId = matchId
    }
}
