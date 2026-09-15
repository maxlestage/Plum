import SwiftUI

/// Un profil de la sélection, et les deux choses qu'on peut en faire.
///
/// Ce n'est plus une carte qu'on balaie : elle ne bouge pas, elle ne
/// s'empile pas, et elle porte ses deux issues écrites en toutes lettres.
/// Trois par jour se lisent ; c'est ce qui permet de les écrire au lieu de
/// les jeter d'un revers de pouce.
struct ProfileCardView: View {
    let profile: Profile
    let onOpen: () -> Void
    let onWrite: () -> Void
    let onPass: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Button(action: onOpen) {
                ZStack(alignment: .bottomLeading) {
                    RemoteImage(url: profile.coverPhoto?.url, seed: profile.displayName)
                        .aspectRatio(3.0 / 4.0, contentMode: .fill)
                        .frame(maxWidth: .infinity)
                        .clipped()
                        .overlay(PlumTheme.Palette.cardScrim)

                    identity
                        .padding(PlumTheme.Spacing.m)
                }
            }
            .buttonStyle(.plain)
            .accessibilityLabel("\(profile.displayName), \(profile.age) ans")
            .accessibilityHint("Ouvre le profil complet")

            actions
                .padding(PlumTheme.Spacing.m)
        }
        .background(PlumTheme.Palette.surface)
        .clipShape(RoundedRectangle(cornerRadius: PlumTheme.Radius.card, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: PlumTheme.Radius.card, style: .continuous)
                .strokeBorder(PlumTheme.Palette.secondaryText.opacity(0.12))
        )
    }

    private var identity: some View {
        VStack(alignment: .leading, spacing: PlumTheme.Spacing.xs) {
            HStack(alignment: .firstTextBaseline, spacing: PlumTheme.Spacing.s) {
                Text(profile.displayName)
                    .font(.plumTitle)
                Text("\(profile.age)")
                    .font(.plumHeadline)
                    .opacity(0.9)
            }
            Text(profile.locationLine)
                .font(.plumCallout)
                .opacity(0.9)
        }
        .foregroundStyle(.white)
    }

    private var actions: some View {
        VStack(alignment: .leading, spacing: PlumTheme.Spacing.m) {
            if !profile.bio.isEmpty {
                // Les deux phrases, en entier. C'est ce sur quoi on décide,
                // donc c'est le seul endroit où tronquer serait absurde.
                Text(profile.bio)
                    .font(.plumBody)
                    .foregroundStyle(PlumTheme.Palette.primaryText)
                    .fixedSize(horizontal: false, vertical: true)
            }

            HStack(spacing: PlumTheme.Spacing.s) {
                Button(action: onPass) {
                    Text("Passer")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(PlumSecondaryButtonStyle())

                Button(action: onWrite) {
                    Label("Écrire", systemImage: "square.and.pencil")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(PlumPrimaryButtonStyle())
            }
        }
    }
}
