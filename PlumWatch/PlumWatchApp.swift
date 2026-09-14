import SwiftUI

@main
struct PlumWatchApp: App {
    @State private var link = WatchSessionStore()

    var body: some Scene {
        WindowGroup {
            NavigationStack {
                ConversationsView(link: link)
            }
        }
    }
}
