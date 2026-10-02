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
                // straight to the selection puts an empty profile in front of people.
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

/// La marque : deux disques qui se chevauchent, en blanc cassé sur prune
/// profond.
///
/// C'était un cœur sur un dégradé — une marque qui n'était celle de personne :
/// le site, le favicon et l'icône d'application montrent les deux disques, et
/// le cœur est précisément le « j'aime » que le site dit ne pas exister.
///
/// Même géométrie, mêmes coordonnées, même repère de 64 unités que
/// `web/src/components/marque.rs`, `web/public/favicon.svg` et
/// `Scripts/generate_appicon.py`. `Scripts/check_mark.py` vérifie que les
/// quatre disent la même chose : c'est faute d'un tel contrôle que celle-ci
/// avait dérivé.
struct PlumMark: View {
    var size: CGFloat = 64

    var body: some View {
        ZStack {
            Circle()
                .fill(PlumTheme.Palette.plumDeep)
            Croissant(grand: Self.gaucheGrand, petit: Self.gauchePetit)
                .fill(PlumTheme.Palette.onPlum, style: FillStyle(eoFill: true))
            Croissant(grand: Self.droiteGrand, petit: Self.droitePetit)
                .fill(PlumTheme.Palette.onPlum, style: FillStyle(eoFill: true))
        }
        .frame(width: size, height: size)
        .accessibilityHidden(true)
    }

    // Chaque moitié : un grand disque dont on retire un petit disque décalé.
    // (centre x, centre y, rayon), dans le repère de 64 unités.
    static let gaucheGrand = Disque(x: 23.5, y: 32.0, rayon: 17.5)
    static let gauchePetit = Disque(x: 30.0, y: 32.0, rayon: 15.2)
    static let droiteGrand = Disque(x: 40.5, y: 32.0, rayon: 17.5)
    static let droitePetit = Disque(x: 34.0, y: 32.0, rayon: 15.2)

    struct Disque: Sendable {
        let x: CGFloat
        let y: CGFloat
        let rayon: CGFloat
    }

    /// Ce qui appartient à l'un *ou* à l'autre disque, jamais aux deux : d'où
    /// l'épaisseur variable d'un trait de plume, épais sur les flancs et
    /// affiné en haut et en bas.
    private struct Croissant: Shape {
        let grand: Disque
        let petit: Disque

        func path(in rect: CGRect) -> Path {
            let echelle = min(rect.width, rect.height) / 64
            func cadre(_ d: Disque) -> CGRect {
                CGRect(
                    x: rect.minX + (d.x - d.rayon) * echelle,
                    y: rect.minY + (d.y - d.rayon) * echelle,
                    width: 2 * d.rayon * echelle,
                    height: 2 * d.rayon * echelle
                )
            }
            var trace = Path()
            trace.addEllipse(in: cadre(grand))
            trace.addEllipse(in: cadre(petit))
            return trace
        }
    }
}

#Preview {
    RootView()
        .environment(SessionStore(auth: AppEnvironment.preview.auth))
        .environment(\.services, .preview)
        .environment(SelectionRefreshSignal())
}
