import SwiftUI

/// The three things the app does, in the order people use them.
struct MainTabView: View {
    @Environment(\.services) private var services
    @Environment(SessionStore.self) private var session
    @State private var selection: Tab = .discovery
    @State private var unreadCount = 0
    @State private var locationSync: LocationSync?

    /// Injecté plutôt qu'appelé directement : ActivityKit ne fonctionne pas
    /// dans un test unitaire — il lui faut un système vivant. Sans cette
    /// couture, la logique qui décide *quand* démarrer une activité ne serait
    /// vérifiée par rien, et c'est elle qui compte.
    private let activities: any MatchActivityPresenting

    init(activities: any MatchActivityPresenting = MatchActivityService()) {
        self.activities = activities
    }

    enum Tab: Hashable {
        case discovery
        case matches
        case profile
    }

    var body: some View {
        TabView(selection: $selection) {
            SelectionView()
                // « Aujourd'hui » et non « Découvrir » : ce qu'il y a derrière
                // est daté et fini, pas un robinet. Et la flamme partait avec
                // le balayage — c'est l'icône d'une autre application.
                .tabItem {
                    Label("Aujourd'hui", systemImage: "sparkles")
                }
                .tag(Tab.discovery)

            MatchesView()
                // Plus de « matchs » : il n'y a plus de double oui. Ce qui est
                // dans cet onglet, ce sont des conversations.
                .tabItem {
                    Label("Messages", systemImage: "bubble.left.and.bubble.right.fill")
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
            await refreshLocation()
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

    /// Distances are computed server-side from the last position we pushed,
    /// so a stale one means a deck sorted by where the person used to be.
    private func refreshLocation() async {
        if locationSync == nil {
            locationSync = LocationSync(provider: services.location, profiles: services.profiles)
        }
        await locationSync?.sync()
    }

    /// Un seul abonnement au socket pour toute l'application : un message qui
    /// arrive pendant qu'on lit la sélection se voit sans ouvrir l'onglet.
    private func watchForActivity() async {
        guard let stream = try? await services.chat.eventStream() else { return }
        for await event in stream {
            let suite = Self.reaction(
                to: event,
                mine: session.currentUserId,
                showingMessages: selection == .matches
            )
            if suite.badge { unreadCount += 1 }
            if let match = suite.beginActivity {
                await activities.begin(
                    matchId: match.id,
                    displayName: match.profile.displayName,
                    conversationId: match.conversationId,
                    matchedAt: match.matchedAt
                )
            }
        }
    }

    /// Ce qu'un événement du socket doit déclencher.
    ///
    /// Sorti de la vue parce que c'est la seule partie qui peut se tromper en
    /// silence, et qu'une vue SwiftUI ne s'interroge pas depuis un test. Deux
    /// règles y vivent :
    ///
    /// - le socket nous renvoie nos propres messages ; se mettre une pastille
    ///   à soi-même pour un message qu'on vient d'écrire est absurde ;
    /// - un premier message est ce qui pose l'activité en direct. Avant, elle
    ///   naissait d'un double oui — il n'y en a plus, et si le déclencheur
    ///   n'avait pas déménagé avec, la carte de l'écran verrouillé ne serait
    ///   simplement jamais apparue. Rien ne l'aurait dit.
    static func reaction(
        to event: ChatEvent,
        mine: UUID?,
        showingMessages: Bool
    ) -> Reaction {
        switch event {
        case let .messageReceived(message):
            let leMien = message.senderId == mine
            return Reaction(badge: !leMien && !showingMessages, beginActivity: nil)
        case let .matchCreated(match):
            return Reaction(badge: true, beginActivity: match)
        default:
            return Reaction(badge: false, beginActivity: nil)
        }
    }

    struct Reaction: Equatable {
        var badge: Bool
        var beginActivity: Match?
    }
}

#Preview {
    MainTabView()
        .environment(\.services, .preview)
        .environment(SessionStore(auth: AppEnvironment.preview.auth))
        .environment(DeckRefreshSignal())
}
