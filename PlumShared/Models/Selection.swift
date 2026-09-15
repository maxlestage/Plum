import Foundation

/// La sélection du jour : trois profils, et deux issues pour chacun.
///
/// Elle remplace le deck. Un deck n'avait pas de fond : il se paginait, et il
/// fallait un geste par carte pour avancer. Un geste qu'on répète cent fois
/// doit être minuscule — d'où le balayage, et d'où le fait qu'on décidait de
/// quelqu'un en un quart de seconde. Trois profils tiennent sur un écran et se
/// lisent.
struct DailySelection: Decodable, Sendable {
    let items: [Profile]
    /// Quand la prochaine sélection sera tirée. L'écran le dit, parce que
    /// « revenez demain » sans heure n'est pas une information.
    let refreshesAt: Date
    /// Ce que la sélection contient quand elle est pleine, pour pouvoir dire
    /// « il en reste deux » sans le déduire.
    let size: Int
}

/// Le premier message, celui qui ouvre tout.
///
/// Il n'y a plus de « j'aime » qui attend sa réciproque : écrire *est* le geste
/// positif. Ne rien répondre est une réponse, et elle n'a besoin d'aucun écran.
struct WriteFirstRequest: Encodable, Sendable {
    let body: String
}
