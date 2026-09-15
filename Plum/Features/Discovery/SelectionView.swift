import SwiftUI

/// Ce que l'écran peut poser par-dessus lui. Deux `.sheet` sur la même vue est
/// un piège SwiftUI — une seule des deux se présente — donc tout passe par une.
enum SelectionSheet: Identifiable {
    case profile(Profile)
    case report(Profile)
    case write(Profile)

    var id: String {
        switch self {
        case let .profile(profile): return "profile-\(profile.id)"
        case let .report(profile): return "report-\(profile.id)"
        case let .write(profile): return "write-\(profile.id)"
        }
    }
}

/// La sélection du jour.
///
/// Une liste qu'on fait défiler, pas une pile qu'on jette. Le défilement ne
/// décide de rien — il sert à lire — et chaque profil porte ses deux issues
/// écrites en toutes lettres. C'est tout l'écart avec ce qu'il y avait avant :
/// un paquet sans fond obligeait à un geste minuscule par carte, et un geste
/// minuscule tranche en un quart de seconde.
struct SelectionView: View {
    @Environment(\.services) private var services
    @Environment(SelectionRefreshSignal.self) private var deckRefresh
    @State private var viewModel: DiscoveryViewModel?
    @State private var sheet: SelectionSheet?
    @State private var locationSync: LocationSync?
    @State private var confirmingPass: Profile?

    var body: some View {
        PlumBackground {
            if let viewModel {
                content(for: viewModel)
            } else {
                ProgressView().tint(PlumTheme.Palette.plum)
            }
        }
        .task {
            // Construit ici plutôt que dans `init` pour recevoir les services
            // de l'environnement, que le mode démo peut avoir remplacés.
            if viewModel == nil {
                viewModel = DiscoveryViewModel(discovery: services.discovery)
            }
            if locationSync == nil {
                locationSync = LocationSync(provider: services.location, profiles: services.profiles)
            }
            await locationSync?.refreshAuthorization()
            await viewModel?.load()
        }
        .onChange(of: deckRefresh.token) { _, _ in
            Task { await viewModel?.load() }
        }
        .sheet(item: $sheet) { presented in
            switch presented {
            case let .profile(profile):
                ProfileDetailView(
                    profile: profile,
                    onWrite: { sheet = .write(profile) },
                    onPass: { confirmingPass = profile },
                    onReport: { reason in
                        Task { await viewModel?.report(profile, reason: reason) }
                    },
                    onBlock: {
                        Task { await viewModel?.block(profile) }
                    }
                )
            case let .report(profile):
                ReportSheet(profile: profile) { reason in
                    Task { await viewModel?.report(profile, reason: reason) }
                } onBlock: {
                    Task { await viewModel?.block(profile) }
                }
            case let .write(profile):
                WriteFirstView(profile: profile) { texte in
                    guard let viewModel else { return false }
                    return await viewModel.write(to: profile, body: texte)
                }
            }
        }
        // Laisser passer est définitif : la personne ne reviendra pas. Le dire
        // avant plutôt que de proposer une annulation après, qui était
        // justement la rustine du balayage.
        .confirmationDialog(
            confirmingPass.map { "Laisser passer \($0.displayName) ?" } ?? "",
            isPresented: Binding(
                get: { confirmingPass != nil },
                set: { if !$0 { confirmingPass = nil } }
            ),
            titleVisibility: .visible
        ) {
            Button("Laisser passer", role: .destructive) {
                if let profile = confirmingPass {
                    Task { await viewModel?.pass(profile) }
                }
                confirmingPass = nil
            }
        } message: {
            Text("Ce profil ne vous sera plus proposé.")
        }
        // Un geste de sécurité qui n'est pas parti doit se dire. Le silence
        // laisserait quelqu'un se croire protégé alors qu'il ne l'est pas.
        .alert(
            "Geste non enregistré",
            isPresented: Binding(
                get: { viewModel?.safetyFailure != nil },
                set: { if !$0 { viewModel?.dismissSafetyFailure() } }
            )
        ) {
            Button("D'accord", role: .cancel) { viewModel?.dismissSafetyFailure() }
        } message: {
            Text(viewModel?.safetyFailure ?? "")
        }
        .alert(
            "Message envoyé",
            isPresented: Binding(
                get: { viewModel?.justOpened != nil },
                set: { if !$0 { viewModel?.dismissOpenedThread() } }
            )
        ) {
            Button("D'accord") { viewModel?.dismissOpenedThread() }
        } message: {
            Text(
                viewModel?.justOpened.map {
                    "La conversation avec \($0.displayName) est ouverte. Vous la retrouverez dans vos messages."
                } ?? ""
            )
        }
    }

    // MARK: - Les morceaux

    @ViewBuilder
    private func content(for viewModel: DiscoveryViewModel) -> some View {
        ScrollView {
            VStack(alignment: .leading, spacing: PlumTheme.Spacing.l) {
                header(for: viewModel)
                locationPrompt

                if let error = viewModel.state.error {
                    ErrorBanner(message: error.userMessage) {
                        Task { await viewModel.load() }
                    }
                }

                if viewModel.state.isLoading && viewModel.profiles.isEmpty {
                    ProgressView()
                        .tint(PlumTheme.Palette.plum)
                        .frame(maxWidth: .infinity)
                        .padding(.top, PlumTheme.Spacing.xl)
                } else if viewModel.isEmpty {
                    done(for: viewModel)
                } else {
                    ForEach(viewModel.profiles) { profile in
                        ProfileCardView(
                            profile: profile,
                            onOpen: { sheet = .profile(profile) },
                            onWrite: { sheet = .write(profile) },
                            onPass: { confirmingPass = profile }
                        )
                        .contextMenu {
                            Button(role: .destructive) {
                                sheet = .report(profile)
                            } label: {
                                Label("Signaler ou bloquer", systemImage: "flag")
                            }
                        }
                        .accessibilityAction(named: Text("Signaler ou bloquer")) {
                            sheet = .report(profile)
                        }
                    }
                }
            }
            .padding(.horizontal, PlumTheme.Spacing.l)
            .padding(.bottom, PlumTheme.Spacing.xl)
        }
        .refreshable { await viewModel.load() }
    }

    private func header(for viewModel: DiscoveryViewModel) -> some View {
        VStack(alignment: .leading, spacing: PlumTheme.Spacing.xs) {
            HStack {
                PlumMark(size: 32)
                Text("Plum")
                    .font(.plumTitle)
                    .foregroundStyle(PlumTheme.Palette.plum)
                Spacer()
            }
            // Le compte restant, pas un quota de « j'aime » : il dit ce qu'il
            // reste à lire, pas ce qu'il reste à dépenser.
            if let size = viewModel.size, !viewModel.profiles.isEmpty {
                Text(
                    viewModel.remaining == size
                        ? "Vos \(size) profils du jour."
                        : "Il en reste \(viewModel.remaining) sur \(size)."
                )
                .font(.plumCallout)
                .foregroundStyle(PlumTheme.Palette.secondaryText)
            }
        }
        .padding(.top, PlumTheme.Spacing.s)
    }

    /// L'écran de fin de journée.
    ///
    /// Il dit *quand* revenir, parce que « revenez demain » sans heure n'est
    /// pas une information — et parce que ne rien dire donnerait envie de
    /// recharger, ce qui est exactement l'habitude qu'on ne veut pas créer.
    private func done(for viewModel: DiscoveryViewModel) -> some View {
        VStack(spacing: PlumTheme.Spacing.m) {
            Image(systemName: "moon.stars")
                .font(.system(size: 40))
                .foregroundStyle(PlumTheme.Palette.plum)
            Text("C'est tout pour aujourd'hui")
                .font(.plumTitle)
                .foregroundStyle(PlumTheme.Palette.primaryText)
            Text(nextTimeLine(viewModel))
                .font(.plumCallout)
                .foregroundStyle(PlumTheme.Palette.secondaryText)
                .multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity)
        .padding(.top, PlumTheme.Spacing.xl)
    }

    private func nextTimeLine(_ viewModel: DiscoveryViewModel) -> String {
        guard let next = viewModel.refreshesAt else {
            return "Trois autres profils demain."
        }
        let heure = next.formatted(date: .omitted, time: .shortened)
        return "Trois autres profils à partir de \(heure)."
    }

    /// Quelqu'un qui se connecte sur un nouvel appareil saute l'accueil, donc
    /// c'est le seul endroit où on le lui demanderait. Un bandeau plutôt
    /// qu'une modale : une sélection sans distances fonctionne quand même.
    @ViewBuilder
    private var locationPrompt: some View {
        if let locationSync, locationSync.needsPermission {
            Button {
                Task { await locationSync.requestPermissionAndSync() }
            } label: {
                HStack(spacing: PlumTheme.Spacing.s) {
                    Image(systemName: "location.circle.fill")
                    Text("Activez la localisation pour voir qui est près de vous.")
                        .font(.plumCaption)
                        .multilineTextAlignment(.leading)
                    Spacer(minLength: 0)
                }
                .foregroundStyle(PlumTheme.Palette.plum)
                .padding(PlumTheme.Spacing.s)
                .background(
                    RoundedRectangle(cornerRadius: PlumTheme.Radius.small, style: .continuous)
                        .fill(PlumTheme.Palette.plum.opacity(0.1))
                )
            }
            .buttonStyle(.plain)
        }
    }
}
