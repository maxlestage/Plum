import SwiftUI
import WidgetKit

/// Le point d'entrée de l'extension.
///
/// Un seul widget pour l'instant, mais le bundle est ce que le système
/// charge : ajouter un widget d'écran d'accueil plus tard se fera ici, sans
/// toucher au projet.
@main
struct PlumWidgetBundle: WidgetBundle {
    var body: some Widget {
        MatchLiveActivity()
    }
}
