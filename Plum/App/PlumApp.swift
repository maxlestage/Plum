import SwiftUI

@main
struct PlumApp: App {
    @State private var services: AppEnvironment
    @State private var session: SessionStore
    @State private var deckRefresh = DeckRefreshSignal()

    init() {
        // Demo mode keeps the app fully usable without the Rust API running,
        // which is how previews, UI tests and design reviews get their data.
        let environment = DemoMode.isEnabled ? AppEnvironment.demo() : AppEnvironment.live()
        _services = State(initialValue: environment)
        _session = State(initialValue: SessionStore(auth: environment.auth))
    }

    var body: some Scene {
        WindowGroup {
            RootView()
                .environment(\.services, services)
                .environment(session)
                .environment(deckRefresh)
                .tint(PlumTheme.Palette.plum)
        }
    }
}
