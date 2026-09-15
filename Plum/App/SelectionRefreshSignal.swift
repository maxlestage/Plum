import Observation
import SwiftUI

/// Dit à la sélection du jour que les critères ont changé.
///
/// The screen that edits the search criteria and the screen that shows the
/// results are in different tabs, and a `TabView` keeps both alive — so the
/// `.task` de l'écran ne tourne plus jamais, et les nouveaux critères ne
/// the next launch. One counter in the environment closes that gap.
@MainActor
@Observable
final class SelectionRefreshSignal {
    private(set) var token = 0

    func invalidate() {
        token += 1
    }
}
