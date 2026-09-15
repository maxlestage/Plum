import SwiftUI

/// Écrire la première phrase.
///
/// Les deux phrases de la personne restent affichées au-dessus du champ, et ce
/// n'est pas de la décoration : c'est ce à quoi on répond. Les faire
/// disparaître au moment d'écrire, c'est demander un premier message à partir
/// de rien — et un premier message écrit à partir de rien est un « salut ».
struct WriteFirstView: View {
    let profile: Profile
    let onSend: (String) async -> Bool

    @Environment(\.dismiss) private var dismiss
    @State private var texte = ""
    @State private var envoi = false
    @FocusState private var clavier: Bool

    private var propre: String {
        texte.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    var body: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: PlumTheme.Spacing.l) {
                if !profile.bio.isEmpty {
                    VStack(alignment: .leading, spacing: PlumTheme.Spacing.s) {
                        Text(profile.displayName)
                            .font(.plumHeadline)
                        Text(profile.bio)
                            .font(.plumBody)
                            .foregroundStyle(PlumTheme.Palette.secondaryText)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(PlumTheme.Spacing.m)
                    .background(PlumTheme.Palette.surface)
                    .clipShape(
                        RoundedRectangle(cornerRadius: PlumTheme.Radius.medium, style: .continuous)
                    )
                }

                TextField("Votre première phrase", text: $texte, axis: .vertical)
                    .font(.plumBody)
                    .lineLimit(3...8)
                    .focused($clavier)
                    .textFieldStyle(.plain)
                    .padding(PlumTheme.Spacing.m)
                    .background(PlumTheme.Palette.surface)
                    .clipShape(
                        RoundedRectangle(cornerRadius: PlumTheme.Radius.medium, style: .continuous)
                    )
                    .accessibilityLabel("Votre première phrase à \(profile.displayName)")

                Text("Il n'y a pas de « match » à attendre : votre message part maintenant, et \(profile.displayName) répond ou ne répond pas.")
                    .font(.plumCaption)
                    .foregroundStyle(PlumTheme.Palette.secondaryText)
                    .fixedSize(horizontal: false, vertical: true)

                Spacer(minLength: 0)
            }
            .padding(PlumTheme.Spacing.l)
            .background(PlumTheme.Palette.canvas)
            .navigationTitle("Écrire")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Annuler") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Envoyer") {
                        Task {
                            envoi = true
                            let parti = await onSend(propre)
                            envoi = false
                            if parti { dismiss() }
                        }
                    }
                    .disabled(propre.isEmpty || envoi)
                }
            }
            .onAppear { clavier = true }
        }
    }
}
