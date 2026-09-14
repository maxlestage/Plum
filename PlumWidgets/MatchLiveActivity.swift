import ActivityKit
import SwiftUI
import WidgetKit

/// Le match frais, sur l'écran verrouillé et dans l'île.
///
/// Ce qu'elle montre tient en une ligne : avec qui, depuis combien de temps,
/// et un geste pour écrire. Rien d'autre — une Live Activity qu'on doit lire
/// a déjà échoué, elle se consulte d'un regard en sortant le téléphone de sa
/// poche.
///
/// Le compteur est un `Text(_:style: .timer)` alimenté par une date, pas une
/// durée calculée à l'avance. C'est le système qui l'anime, donc il reste
/// juste même quand l'application ne tourne plus — ce qui est précisément le
/// cas dès que l'écran se verrouille.
struct MatchLiveActivity: Widget {
    var body: some WidgetConfiguration {
        ActivityConfiguration(for: MatchActivityAttributes.self) { context in
            LockScreenView(context: context)
                .activityBackgroundTint(PlumBrand.plumDeep)
                .activitySystemActionForegroundColor(PlumBrand.cream)
        } dynamicIsland: { context in
            DynamicIsland {
                DynamicIslandExpandedRegion(.leading) {
                    Mark(diameter: 34)
                        .padding(.leading, 4)
                }
                DynamicIslandExpandedRegion(.trailing) {
                    Elapsed(since: context.state.matchedAt)
                        .padding(.trailing, 4)
                }
                DynamicIslandExpandedRegion(.bottom) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(title(for: context))
                            .font(.headline)
                            .foregroundStyle(PlumBrand.cream)
                        Text(subtitle(for: context))
                            .font(.caption)
                            .foregroundStyle(PlumBrand.cream.opacity(0.75))
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
            } compactLeading: {
                Mark(diameter: 18)
            } compactTrailing: {
                // Compact : le compteur seul, sans unité ni libellé. L'espace
                // est celui d'une pilule ; tout ce qui n'y tient pas est
                // tronqué par le système, ce qui est pire que de l'omettre.
                Elapsed(since: context.state.matchedAt)
                    .font(.caption2.monospacedDigit())
            } minimal: {
                Mark(diameter: 16)
            }
            .keylineTint(PlumBrand.blush)
            .widgetURL(url(for: context))
        }
    }

    private func title(for context: ActivityViewContext<MatchActivityAttributes>) -> String {
        context.state.hasStarted
            ? "Conversation avec \(context.attributes.displayName)"
            : "Vous avez matché avec \(context.attributes.displayName)"
    }

    private func subtitle(for context: ActivityViewContext<MatchActivityAttributes>) -> String {
        context.state.hasStarted ? "C'est parti." : "Le premier message part maintenant."
    }

    /// L'adresse qu'ouvre un appui sur l'activité.
    ///
    /// Vers la conversation quand elle existe, vers la liste des matchs
    /// sinon. Ouvrir l'application sur son écran d'accueil après avoir touché
    /// une carte qui nomme quelqu'un serait un geste perdu.
    private func url(for context: ActivityViewContext<MatchActivityAttributes>) -> URL? {
        if let conversation = context.attributes.conversationId {
            return URL(string: "plum://conversations/\(conversation.uuidString)")
        }
        return URL(string: "plum://matches")
    }
}

/// L'écran verrouillé : une bande, pas une carte.
private struct LockScreenView: View {
    let context: ActivityViewContext<MatchActivityAttributes>

    var body: some View {
        HStack(spacing: 14) {
            Mark(diameter: 40)

            VStack(alignment: .leading, spacing: 3) {
                Text(
                    context.state.hasStarted
                        ? "Conversation avec \(context.attributes.displayName)"
                        : "Vous avez matché avec \(context.attributes.displayName)"
                )
                .font(.headline)
                .foregroundStyle(PlumBrand.cream)
                .lineLimit(1)

                Text(
                    context.state.hasStarted
                        ? "C'est parti."
                        : "Le premier message part maintenant."
                )
                .font(.subheadline)
                .foregroundStyle(PlumBrand.cream.opacity(0.75))
                .lineLimit(1)
            }

            Spacer(minLength: 8)

            Elapsed(since: context.state.matchedAt)
                .font(.title3.monospacedDigit())
        }
        .padding(.horizontal, 18)
        .padding(.vertical, 14)
    }
}

/// Le temps écoulé, animé par le système.
private struct Elapsed: View {
    let since: Date

    var body: some View {
        Text(since, style: .timer)
            .monospacedDigit()
            .foregroundStyle(PlumBrand.cream)
            // Sans largeur minimale, la pilule de l'île se redimensionne à
            // chaque seconde qui change de chiffre, et l'ensemble tressaute.
            .frame(minWidth: 44, alignment: .trailing)
    }
}

/// La marque, redessinée ici parce qu'une extension ne voit pas les vues de
/// l'application. Les mêmes nombres que `PlumMark`, dans le même repère de 64.
private struct Mark: View {
    let diameter: CGFloat

    var body: some View {
        Canvas { context, size in
            let scale = size.width / 64
            context.fill(Path(ellipseIn: CGRect(origin: .zero, size: size)), with: .color(PlumBrand.plum))

            for (grand, petit) in [
                (CGPoint(x: 23.5, y: 32), CGPoint(x: 30, y: 32)),
                (CGPoint(x: 40.5, y: 32), CGPoint(x: 34, y: 32)),
            ] {
                var forme = Path()
                forme.addEllipse(in: cercle(grand, 17.5, scale))
                forme.addEllipse(in: cercle(petit, 15.2, scale))
                // `evenOdd` : appartenir à l'un *ou* à l'autre, jamais aux
                // deux. C'est ce que dit `fill-rule="evenodd"` dans le SVG,
                // et une soustraction ordinaire donnerait une autre forme.
                context.fill(forme, with: .color(PlumBrand.cream), style: FillStyle(eoFill: true))
            }
        }
        .frame(width: diameter, height: diameter)
        .accessibilityHidden(true)
    }

    private func cercle(_ centre: CGPoint, _ rayon: CGFloat, _ scale: CGFloat) -> CGRect {
        CGRect(
            x: (centre.x - rayon) * scale,
            y: (centre.y - rayon) * scale,
            width: rayon * 2 * scale,
            height: rayon * 2 * scale
        )
    }
}
