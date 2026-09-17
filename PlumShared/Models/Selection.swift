import Foundation

/// La sélection du jour : une poignée de profils, et deux issues pour chacun.
///
/// Elle remplace le deck. Un deck n'avait pas de fond : il se paginait, et il
/// fallait un geste par carte pour avancer. Un geste qu'on répète cent fois
/// doit être minuscule — d'où le balayage, et d'où le fait qu'on décidait de
/// quelqu'un en un quart de seconde. Deux à cinq profils tiennent sur un écran
/// et se lisent.
///
/// Le nombre change d'un jour à l'autre, et c'est voulu : un compte fixe se
/// transforme en habitude — on sait ce qu'on va trouver avant d'ouvrir, on
/// l'expédie, et l'application redevient la pile qu'elle ne voulait pas être.
struct DailySelection: Decodable, Sendable {
    let items: [Profile]
    /// Quand la prochaine sélection sera tirée. L'écran le dit, parce que
    /// « revenez demain » sans heure n'est pas une information.
    let refreshesAt: Date
    /// Combien de profils ont été servis aujourd'hui, pour pouvoir dire « il
    /// en reste deux sur quatre » sans le déduire. C'est bien ce qui a été
    /// servi et pas ce que le serveur visait : les jours où le voisinage n'a
    /// pas de quoi remplir la sélection, la phrase resterait vraie.
    let size: Int
}

/// Le premier message, celui qui ouvre tout.
///
/// Il n'y a plus de « j'aime » qui attend sa réciproque : écrire *est* le geste
/// positif. Ne rien répondre est une réponse, et elle n'a besoin d'aucun écran.
struct WriteFirstRequest: Encodable, Sendable {
    let body: String
}
