import SwiftUI

/// Automatique, clair, sombre.
///
/// L'application suivait déjà le système toute seule : ses couleurs viennent
/// du catalogue d'assets, qui porte une variante sombre pour chacune. Ce qui
/// manquait, c'est de pouvoir dire le contraire — lire au lit sur un téléphone
/// resté en thème clair, ou l'inverse.
///
/// `automatic` est le cas par défaut, et il **ne stocke rien**. Une valeur
/// absente et « automatique » sont donc la même chose, ce qui évite d'avoir à
/// migrer quoi que ce soit pour les comptes existants.
enum AppearancePreference: String, CaseIterable, Identifiable, Sendable {
    // Les valeurs brutes sont écrites à la main, et c'est délibéré : elles
    // partent dans les réglages de l'appareil. Laisser Swift les dériver du
    // nom du cas ferait qu'un renommage remettrait tout le monde en
    // automatique, sans erreur de compilation et sans que personne ne
    // comprenne pourquoi. `AppearanceTests` les fige.
    case automatic = "automatic"
    case light = "light"
    case dark = "dark"

    var id: String { rawValue }

    /// Ce que SwiftUI attend. `nil` veut dire « ne force rien », donc suit le
    /// système — la valeur par défaut est aussi la plus simple à exprimer.
    var colorScheme: ColorScheme? {
        switch self {
        case .automatic: nil
        case .light: .light
        case .dark: .dark
        }
    }

    var label: String {
        switch self {
        case .automatic: "Automatique"
        case .light: "Clair"
        case .dark: "Sombre"
        }
    }

    /// Le dessin de la pastille, repris du site — même vocabulaire des deux
    /// côtés pour un réglage qui veut dire la même chose.
    var symbol: String {
        switch self {
        case .automatic: "circle.lefthalf.filled"
        case .light: "sun.max"
        case .dark: "moon"
        }
    }

    /// La clé dans les réglages de l'appareil. Partagée par `@AppStorage` et
    /// les tests ; deux littéraux identiques finiraient par ne plus l'être.
    static let storageKey = "apparence"
}
