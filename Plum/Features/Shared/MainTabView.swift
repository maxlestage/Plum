import SwiftUI

/// The three things the app does, in the order people use them.
struct MainTabView: View {
    @Environment(\.services) private var services
    @Environment(SessionStore.self) private var session
    @State private var selection: Tab = .discovery
    @State private var unreadCount = 0

    enum Tab: Hashable {
        case discovery
        case matches
        case profile
    }

    var body: some View {
        TabView(selection: $selection) {
            DiscoveryView()
                .tabItem {
                    Label("Découvrir", systemImage: "flame.fill")
                }
                .tag(Tab.discovery)

            MatchesView()
                .tabItem {
                    Label("Matchs", systemImage: "bubble.left.and.bubble.right.fill")
                }
                .badge(unreadCount)
                .tag(Tab.matches)

            ProfileView()
                .tabItem {
                    Label("Profil", systemImage: "person.fill")
                }
                .tag(Tab.profile)
        }
        .task {
            await loadProfile()
            await watchForActivity()
        }
        .onChange(of: selection) { _, newValue in
            // Opening the tab is as good as reading the badge.
            if newValue == .matches { unreadCount = 0 }
        }
    }

    /// The current user's own profile is needed by several screens (the match
    /// celebration, the chat header); fetch it once, here.
    private func loadProfile() async {
        guard session.currentProfile == nil else { return }
        session.currentProfile = try? await services.profiles.myProfile()
    }

    /// One app-wide socket subscription drives the tab badge, so a message
    /// arriving while you are swiping is visible without opening the tab.
    private func watchForActivity() async {
        guard let stream = try? await services.chat.eventStream() else { return }
        for await event in stream {
            switch event {
            case .messageReceived:
                if selection != .matches { unreadCount += 1 }
            case .matchCreated:
                unreadCount += 1
            default:
                break
            }
        }
    }
}

#Preview {
    MainTabView()
        .environment(\.services, .preview)
        .environment(SessionStore(auth: AppEnvironment.preview.auth))
}
