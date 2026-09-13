import SwiftUI

/// The auth gate. Everything below it can assume there is a signed-in user.
struct RootView: View {
    @Environment(SessionStore.self) private var session

    var body: some View {
        Group {
            switch session.state {
            case .launching:
                LaunchView()
            case .signedOut:
                WelcomeView()
                    .transition(.opacity)
            case .signedIn:
                // A brand-new account has no photo and no bio; sending it
                // straight to the deck wastes everyone's swipes.
                if session.needsOnboarding {
                    OnboardingView()
                        .transition(.opacity)
                } else {
                    MainTabView()
                        .transition(.opacity.combined(with: .scale(scale: 0.98)))
                }
            }
        }
        .animation(.easeInOut(duration: 0.3), value: session.state)
        .task {
            await session.restore()
        }
    }
}

struct LaunchView: View {
    var body: some View {
        PlumBackground {
            VStack(spacing: PlumTheme.Spacing.m) {
                PlumMark(size: 88)
                ProgressView()
                    .tint(PlumTheme.Palette.plum)
            }
        }
    }
}

/// The logo: a plum, drawn rather than shipped as an asset so it scales.
struct PlumMark: View {
    var size: CGFloat = 64

    var body: some View {
        ZStack {
            Circle()
                .fill(PlumTheme.Palette.warmGradient)
            Image(systemName: "heart.fill")
                .font(.system(size: size * 0.42, weight: .bold))
                .foregroundStyle(.white)
        }
        .frame(width: size, height: size)
        .accessibilityHidden(true)
    }
}

#Preview {
    RootView()
        .environment(SessionStore(auth: AppEnvironment.preview.auth))
        .environment(\.services, .preview)
        .environment(DeckRefreshSignal())
}
