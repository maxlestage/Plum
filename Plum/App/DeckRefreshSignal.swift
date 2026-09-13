import Observation
import SwiftUI

/// Tells the deck its filters changed.
///
/// The screen that edits the search criteria and the screen that shows the
/// results are in different tabs, and a `TabView` keeps both alive — so the
/// deck's `.task` never runs again and the new criteria only took effect on
/// the next launch. One counter in the environment closes that gap.
@MainActor
@Observable
final class DeckRefreshSignal {
    private(set) var token = 0

    func invalidate() {
        token += 1
    }
}
