import Observation
import SwiftUI

/// Qui on a bloqué, et comment revenir en arrière.
@MainActor
@Observable
final class BlockedListViewModel {
    private(set) var people: [BlockedPerson] = []
    private(set) var state: ActivityState = .idle
    /// Ce qu'il faut dire quand un déblocage n'a pas abouti. L'écran ne doit
    /// pas retirer la ligne d'une personne qui est restée bloquée.
    private(set) var failure: String?

    private let discovery: any DiscoveryServicing

    init(discovery: any DiscoveryServicing) {
        self.discovery = discovery
    }

    func load() async {
        if people.isEmpty { state = .loading }
        do {
            people = try await discovery.blocks()
            state = .ready
        } catch {
            state = .failed(error.asAPIError)
        }
    }

    func unblock(_ person: BlockedPerson) async {
        do {
            try await discovery.unblock(profileId: person.id)
            people.removeAll { $0.id == person.id }
        } catch {
            failure = "\(person.label) est toujours bloqué : \(error.asAPIError.userMessage)"
        }
    }

    func dismissFailure() { failure = nil }
}

/// L'écran du retour en arrière.
///
/// Il dit aussi ce que débloquer ne rend pas. Bloquer avait supprimé la
/// conversation et ses messages des deux côtés ; rouvrir la porte ne
/// ressuscite rien, et mieux vaut l'écrire que de laisser espérer.
struct BlockedListView: View {
    @Environment(\.services) private var services
    @State private var viewModel: BlockedListViewModel?
    @State private var aboutToRelease: BlockedPerson?

    var body: some View {
        Group {
            if let viewModel {
                content(viewModel)
            } else {
                ProgressView()
            }
        }
        .navigationTitle("Personnes bloquées")
        .navigationBarTitleDisplayMode(.inline)
        .task {
            if viewModel == nil {
                viewModel = BlockedListViewModel(discovery: services.discovery)
            }
            await viewModel?.load()
        }
    }

    @ViewBuilder
    private func content(_ viewModel: BlockedListViewModel) -> some View {
        List {
            if let error = viewModel.state.error {
                Text(error.userMessage)
                    .font(.plumCallout)
                    .foregroundStyle(PlumTheme.Palette.pass)
            } else if viewModel.people.isEmpty, viewModel.state.isReady {
                Text("Vous n'avez bloqué personne.")
                    .font(.plumCallout)
                    .foregroundStyle(PlumTheme.Palette.secondaryText)
            } else {
                Section {
                    ForEach(viewModel.people) { person in
                        row(person)
                    }
                } footer: {
                    Text("Débloquer laisse la personne réapparaître dans vos sélections. La conversation, elle, ne revient pas : elle a été supprimée.")
                }
            }
        }
        .overlay {
            if viewModel.state.isLoading, viewModel.people.isEmpty {
                ProgressView()
            }
        }
        .refreshable { await viewModel.load() }
        .confirmationDialog(
            aboutToRelease.map { "Débloquer \($0.label) ?" } ?? "",
            isPresented: Binding(
                get: { aboutToRelease != nil },
                set: { if !$0 { aboutToRelease = nil } }
            ),
            titleVisibility: .visible
        ) {
            Button("Débloquer") {
                guard let person = aboutToRelease else { return }
                aboutToRelease = nil
                Task { await viewModel.unblock(person) }
            }
            Button("Annuler", role: .cancel) { aboutToRelease = nil }
        } message: {
            Text("Vous pourrez la revoir dans vos sélections, et elle pourra vous écrire.")
        }
        .alert(
            "Déblocage impossible",
            isPresented: Binding(
                get: { viewModel.failure != nil },
                set: { if !$0 { viewModel.dismissFailure() } }
            )
        ) {
            Button("OK", role: .cancel) { viewModel.dismissFailure() }
        } message: {
            Text(viewModel.failure ?? "")
        }
    }

    private func row(_ person: BlockedPerson) -> some View {
        HStack {
            VStack(alignment: .leading, spacing: PlumTheme.Spacing.xs) {
                Text(person.label)
                    .font(.plumCallout)
                    .foregroundStyle(
                        person.displayName == nil
                            ? PlumTheme.Palette.secondaryText
                            : PlumTheme.Palette.primaryText
                    )
                Text("Bloqué le \(person.blockedAt.formatted(date: .abbreviated, time: .omitted))")
                    .font(.plumCaption)
                    .foregroundStyle(PlumTheme.Palette.secondaryText)
            }
            Spacer()
            Button("Débloquer") { aboutToRelease = person }
                .font(.plumCallout)
                .buttonStyle(.borderless)
                .accessibilityIdentifier("debloquer-\(person.id.uuidString)")
        }
    }
}

#Preview {
    NavigationStack {
        BlockedListView()
    }
    .environment(\.services, .preview)
}
