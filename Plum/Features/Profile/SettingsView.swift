import SwiftUI

/// Discovery filters, account actions, and the two destructive ones behind
/// confirmations.
struct SettingsView: View {
    @Bindable var viewModel: ProfileViewModel
    @Environment(\.dismiss) private var dismiss
    @Environment(\.services) private var services
    @Environment(SessionStore.self) private var session
    @Environment(DeckRefreshSignal.self) private var deckRefresh

    @State private var draft = DiscoveryPreferences.default
    @State private var locationSync: LocationSync?
    @State private var isConfirmingSignOut = false
    @State private var isConfirmingDeletion = false

    var body: some View {
        NavigationStack {
            Form {
                Section("Je veux voir") {
                    Picker("Me montrer", selection: $draft.interestedIn) {
                        ForEach(GenderPreference.allCases) { preference in
                            Text(preference.label).tag(preference)
                        }
                    }
                    .pickerStyle(.inline)
                    .labelsHidden()
                }

                Section {
                    Stepper(
                        "À partir de \(draft.minAge) ans",
                        value: $draft.minAge,
                        in: 18...99
                    )
                    Stepper(
                        "Jusqu'à \(draft.maxAge) ans",
                        value: $draft.maxAge,
                        in: max(18, draft.minAge)...99
                    )
                } header: {
                    Text("Âge")
                } footer: {
                    Text("Plum est réservé aux majeurs, ici comme ailleurs.")
                }

                Section("Distance") {
                    locationRow
                    VStack(alignment: .leading) {
                        Text("Jusqu'à \(draft.maxDistanceKm) km")
                            .font(.plumCallout)
                        Slider(
                            value: Binding(
                                get: { Double(draft.maxDistanceKm) },
                                set: { draft.maxDistanceKm = Int($0) }
                            ),
                            in: 1...300,
                            step: 1
                        )
                    }
                }

                Section {
                    Toggle("Me montrer sur Plum", isOn: $draft.showMeOnPlum)
                } footer: {
                    Text("Désactivé, votre profil n'apparaît plus dans les decks. Vos conversations restent.")
                }

                Section("À propos") {
                    Link("Conditions d'utilisation", destination: LegalLinks.terms)
                    Link("Confidentialité", destination: LegalLinks.privacy)
                    Link("Aide et contact", destination: LegalLinks.support)
                }

                Section {
                    Button("Se déconnecter") { isConfirmingSignOut = true }
                    Button("Supprimer mon compte", role: .destructive) {
                        isConfirmingDeletion = true
                    }
                }
            }
            .navigationTitle("Réglages")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("OK") {
                        Task { @MainActor in
                            await viewModel.updatePreferences(draft)
                            // The deck lives in another tab and stays alive:
                            // without this the new criteria wait for a relaunch.
                            deckRefresh.invalidate()
                        }
                        dismiss()
                    }
                }
            }
            .onAppear { draft = viewModel.preferences }
            .task {
                if locationSync == nil {
                    locationSync = LocationSync(provider: services.location, profiles: services.profiles)
                }
                await locationSync?.refreshAuthorization()
            }
            .onChange(of: draft.minAge) { _, newValue in
                // Keep the range coherent while it is being dragged.
                if draft.maxAge < newValue { draft.maxAge = newValue }
            }
            .confirmationDialog(
                "Se déconnecter de Plum ?",
                isPresented: $isConfirmingSignOut,
                titleVisibility: .visible
            ) {
                Button("Se déconnecter", role: .destructive) {
                    Task { @MainActor in
                        // Drop the socket first: a live stream outliving the
                        // session would keep pushing someone else's messages.
                        await services.chat.closeStream()
                        await session.signOut()
                        dismiss()
                    }
                }
            }
            .confirmationDialog(
                "Supprimer définitivement votre compte ?",
                isPresented: $isConfirmingDeletion,
                titleVisibility: .visible
            ) {
                Button("Tout supprimer", role: .destructive) {
                    Task { @MainActor in
                        await services.chat.closeStream()
                        try? await session.deleteAccount()
                        dismiss()
                    }
                }
            } message: {
                Text("Vos matchs, vos messages et vos photos partent avec. Il n'y a pas de retour en arrière.")
            }
        }
    }
}

extension SettingsView {
    /// Without a position the distance filter is decorative; say so where the
    /// slider is, rather than letting someone wonder why nobody is nearby.
    @ViewBuilder
    fileprivate var locationRow: some View {
        if let locationSync {
            if locationSync.isBlocked {
                Text("Localisation refusée : le filtre de distance ne s'applique pas. Réactivez-la dans Réglages ▸ Plum.")
                    .font(.plumCaption)
                    .foregroundStyle(PlumTheme.Palette.pass)
                    .fixedSize(horizontal: false, vertical: true)
            } else if locationSync.needsPermission {
                Button("Activer la localisation") {
                    Task { await locationSync.requestPermissionAndSync() }
                }
                .font(.plumCallout)
            } else if let lastError = locationSync.lastError {
                VStack(alignment: .leading, spacing: PlumTheme.Spacing.xs) {
                    Text("Dernière position non transmise : \(lastError)")
                        .font(.plumCaption)
                        .foregroundStyle(PlumTheme.Palette.secondaryText)
                        .fixedSize(horizontal: false, vertical: true)
                    Button("Réessayer") {
                        Task { await locationSync.sync() }
                    }
                    .font(.plumCallout)
                }
            }
        }
    }
}

#Preview {
    SettingsView(
        viewModel: ProfileViewModel(
            profiles: AppEnvironment.preview.profiles,
            session: SessionStore(auth: AppEnvironment.preview.auth)
        )
    )
    .environment(SessionStore(auth: AppEnvironment.preview.auth))
    .environment(\.services, .preview)
    .environment(DeckRefreshSignal())
}
