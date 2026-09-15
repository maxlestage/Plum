import UIKit
import SwiftUI
import XCTest
@testable import Plum

/// Le réglage d'apparence, et surtout ce qui en fait un réglage qu'on ne perd
/// pas.
final class AppearanceTests: XCTestCase {
    /// Les valeurs brutes partent dans les réglages de l'appareil, donc elles
    /// sont un format de stockage, pas des identifiants internes.
    ///
    /// Renommer un cas — ou laisser Swift les dériver et renommer le cas — ne
    /// casserait pas la compilation : ça remettrait simplement en automatique
    /// tous ceux qui avaient choisi. Personne ne le remarquerait avant que
    /// quelqu'un se plaigne que son téléphone « oublie ».
    func testTheStoredSpellingIsFrozen() {
        XCTAssertEqual(
            AppearancePreference.allCases.map(\.rawValue),
            ["automatic", "light", "dark"]
        )
        XCTAssertEqual(AppearancePreference.storageKey, "apparence")
    }

    /// `nil` n'est pas une absence de réponse : c'est la réponse. Si
    /// `automatic` rendait `.light`, l'application cesserait de suivre le
    /// système — exactement ce que ce cas promet de faire.
    func testAutomaticForcesNothing() {
        XCTAssertNil(AppearancePreference.automatic.colorScheme)
        XCTAssertEqual(AppearancePreference.light.colorScheme, .light)
        XCTAssertEqual(AppearancePreference.dark.colorScheme, .dark)
    }

    /// Ce que `@AppStorage` écrit vraiment, relu sans passer par SwiftUI.
    ///
    /// La chaîne est le contrat entre deux lancements de l'application ; la
    /// vérifier depuis l'enum seule ne prouverait que la cohérence de l'enum
    /// avec elle-même.
    func testTheChoiceSurvivesARelaunch() throws {
        let defaults = try XCTUnwrap(UserDefaults(suiteName: #function))
        defer { defaults.removePersistentDomain(forName: #function) }

        defaults.set(AppearancePreference.dark.rawValue, forKey: AppearancePreference.storageKey)

        let relu = AppearancePreference(
            rawValue: defaults.string(forKey: AppearancePreference.storageKey) ?? ""
        )
        XCTAssertEqual(relu, .dark)
    }

    /// Rien de stocké, ou quelque chose d'illisible — une version future, un
    /// réglage bricolé à la main — doit donner « automatique » plutôt que
    /// rien. C'est le seul cas où l'écran peut se retrouver sans couleurs.
    func testAnythingUnreadableFallsBackToAutomatic() {
        for stocke in ["", "sombre", "Dark", "system", "auto"] {
            XCTAssertNil(
                AppearancePreference(rawValue: stocke),
                "« \(stocke) » ne devrait pas être décodé"
            )
        }
    }

    /// L'ordre des cas est celui de la liste de réglages : automatique
    /// d'abord, parce que c'est l'état par défaut et celui auquel on revient.
    /// Il vient de `allCases`, donc de l'ordre de déclaration — un
    /// réagencement du fichier déplacerait la liste à l'écran sans que rien ne
    /// le dise.
    func testAutomaticComesFirst() {
        XCTAssertEqual(AppearancePreference.allCases.first, .automatic)
    }

    /// Les symboles doivent exister dans SF Symbols : un nom inconnu ne fait
    /// pas échouer la compilation, il affiche un rectangle vide.
    func testEveryChoiceHasARealSymbol() {
        for choix in AppearancePreference.allCases {
            XCTAssertNotNil(
                UIImage(systemName: choix.symbol),
                "SF Symbols ne connaît pas « \(choix.symbol) »"
            )
        }
    }
}
