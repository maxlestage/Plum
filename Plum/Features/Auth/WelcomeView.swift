import SwiftUI

/// The first screen: sets the tone, then gets out of the way.
struct WelcomeView: View {
    @Environment(\.services) private var services
    @Environment(SessionStore.self) private var session
    @State private var route: AuthMode?

    var body: some View {
        NavigationStack {
            ZStack {
                PlumTheme.Palette.warmGradient
                    .ignoresSafeArea()

                VStack(spacing: PlumTheme.Spacing.l) {
                    Spacer()
                    PlumMark(size: 96)
                        .shadow(color: .black.opacity(0.2), radius: 20, y: 8)

                    VStack(spacing: PlumTheme.Spacing.s) {
                        Text("Plum")
                            .font(.plumDisplay)
                            .foregroundStyle(.white)
                        Text("Des rencontres pas sérieuses.\nC'est déjà beaucoup.")
                            .font(.plumBody)
                            .multilineTextAlignment(.center)
                            .foregroundStyle(.white.opacity(0.9))
                    }

                    Spacer()

                    VStack(spacing: PlumTheme.Spacing.m) {
                        Button("Créer un compte") { route = .signUp }
                            .accessibilityIdentifier("bouton-inscription")
                            .buttonStyle(PlumPrimaryButtonStyle())
                            .tint(.white)

                        Button("J'ai déjà un compte") { route = .signIn }
                            .accessibilityIdentifier("bouton-connexion")
                            .font(.plumButton)
                            .foregroundStyle(.white)
                            .frame(maxWidth: .infinity, minHeight: 54)
                            .overlay(
                                RoundedRectangle(cornerRadius: PlumTheme.Radius.medium, style: .continuous)
                                    .stroke(.white.opacity(0.6), lineWidth: 1.5)
                            )
                    }

                    Text("En continuant, vous acceptez nos conditions et confirmez avoir 18 ans ou plus.")
                        .font(.plumCaption)
                        .multilineTextAlignment(.center)
                        .foregroundStyle(.white.opacity(0.75))
                        .padding(.bottom, PlumTheme.Spacing.s)
                }
                .padding(.horizontal, PlumTheme.Spacing.l)
            }
            .navigationDestination(item: $route) { mode in
                AuthFormView(
                    viewModel: AuthViewModel(mode: mode, auth: services.auth, session: session)
                )
            }
        }
    }
}

#Preview {
    WelcomeView()
        .environment(SessionStore(auth: AppEnvironment.preview.auth))
        .environment(\.services, .preview)
}
