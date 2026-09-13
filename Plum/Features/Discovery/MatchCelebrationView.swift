import SwiftUI

/// The payoff screen. It has one job: make the moment feel like something, and
/// put a message box one tap away while the feeling lasts.
struct MatchCelebrationView: View {
    let match: Match
    let onDismiss: () -> Void

    @Environment(\.services) private var services
    @Environment(SessionStore.self) private var session
    @State private var hasAppeared = false
    @State private var openedConversation: Conversation?

    var body: some View {
        ZStack {
            PlumTheme.Palette.warmGradient
                .ignoresSafeArea()

            VStack(spacing: PlumTheme.Spacing.l) {
                Spacer()

                Text("C'est un match")
                    .font(.plumDisplay)
                    .foregroundStyle(.white)
                    .scaleEffect(hasAppeared ? 1 : 0.7)
                    .opacity(hasAppeared ? 1 : 0)

                Text("Vous et \(match.profile.displayName) vous êtes plu.")
                    .font(.plumBody)
                    .foregroundStyle(.white.opacity(0.9))

                HStack(spacing: -24) {
                    myAvatar
                        .overlay(Circle().stroke(.white, lineWidth: 4))
                        .rotationEffect(.degrees(hasAppeared ? -6 : -40))
                    AvatarView(profile: match.profile, diameter: 120)
                        .overlay(Circle().stroke(.white, lineWidth: 4))
                        .rotationEffect(.degrees(hasAppeared ? 6 : 40))
                }
                .padding(.vertical, PlumTheme.Spacing.l)

                Spacer()

                VStack(spacing: PlumTheme.Spacing.m) {
                    Button("Envoyer un message") {
                        Task { await openConversation() }
                    }
                    .buttonStyle(PlumPrimaryButtonStyle())

                    Button("Continuer à swiper", action: onDismiss)
                        .font(.plumButton)
                        .foregroundStyle(.white)
                        .frame(maxWidth: .infinity, minHeight: 50)
                }
                .padding(.horizontal, PlumTheme.Spacing.l)
                .padding(.bottom, PlumTheme.Spacing.xl)
            }
        }
        .sheet(item: $openedConversation) { conversation in
            NavigationStack {
                ChatView(conversation: conversation)
            }
        }
        .onAppear {
            withAnimation(.spring(response: 0.6, dampingFraction: 0.6)) {
                hasAppeared = true
            }
        }
    }

    /// The viewer's own photo, when the profile has been loaded; a neutral
    /// mark otherwise, so the screen never waits on a fetch to celebrate.
    @ViewBuilder
    private var myAvatar: some View {
        if let profile = session.currentProfile {
            AvatarView(profile: profile, diameter: 120)
        } else {
            PlumMark(size: 120)
        }
    }

    @MainActor
    private func openConversation() async {
        if let conversation = try? await services.matches.conversation(forMatch: match.id) {
            openedConversation = conversation
        } else {
            onDismiss()
        }
    }
}

#Preview {
    MatchCelebrationView(match: SampleData.matches[0]) {}
        .environment(\.services, .preview)
        .environment(SessionStore(auth: AppEnvironment.preview.auth))
}
