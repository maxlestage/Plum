import SwiftUI

/// Reporting is one tap away from every profile — on a card and inside a
/// conversation alike — because a dating app that buries it is doing it wrong.
struct ReportSheet: View {
    let profile: Profile
    let onReport: (String) -> Void
    let onBlock: () -> Void

    @Environment(\.dismiss) private var dismiss

    private let reasons = [
        "Faux profil",
        "Photos inappropriées",
        "Harcèlement",
        "Profil de mineur",
        "Autre"
    ]

    var body: some View {
        NavigationStack {
            List {
                Section("Signaler \(profile.displayName)") {
                    ForEach(reasons, id: \.self) { reason in
                        Button(reason) {
                            onReport(reason)
                            dismiss()
                        }
                    }
                }
                Section {
                    Button("Bloquer \(profile.displayName)", role: .destructive) {
                        onBlock()
                        dismiss()
                    }
                } footer: {
                    Text("Vous ne verrez plus ce profil et cette personne ne verra plus le vôtre.")
                }
            }
            .navigationTitle("Signalement")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Annuler") { dismiss() }
                }
            }
        }
        .presentationDetents([.medium])
    }
}
