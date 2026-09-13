import SwiftUI

/// Editing works on a draft: nothing changes until Enregistrer, and Annuler
/// restores what was there.
struct EditProfileView: View {
    @Bindable var viewModel: ProfileViewModel
    @Environment(\.dismiss) private var dismiss
    @State private var newInterest = ""

    var body: some View {
        NavigationStack {
            Form {
                Section("Prénom") {
                    TextField("Prénom", text: $viewModel.draftName)
                        .textContentType(.givenName)
                }

                Section {
                    TextField("Dites quelque chose de vrai.", text: $viewModel.draftBio, axis: .vertical)
                        .lineLimit(4...10)
                } header: {
                    Text("Bio")
                } footer: {
                    HStack {
                        Text("Ce qui vous rend supportable en terrasse.")
                        Spacer()
                        Text("\(viewModel.remainingBioCharacters)")
                            .foregroundStyle(
                                viewModel.remainingBioCharacters < 0
                                    ? PlumTheme.Palette.pass
                                    : PlumTheme.Palette.secondaryText
                            )
                            .monospacedDigit()
                    }
                }

                Section("Ville") {
                    TextField("Ville", text: $viewModel.draftCity)
                        .textContentType(.addressCity)
                }

                Section {
                    FlowLayout(spacing: PlumTheme.Spacing.s, alignment: .leading) {
                        ForEach(ProfileViewModel.suggestedInterests, id: \.self) { interest in
                            Button {
                                viewModel.toggleInterest(interest)
                            } label: {
                                InterestChip(
                                    title: interest,
                                    isSelected: viewModel.draftInterests.contains(interest)
                                )
                            }
                            .buttonStyle(.plain)
                        }
                    }
                    .padding(.vertical, PlumTheme.Spacing.xs)

                    HStack {
                        TextField("Autre chose…", text: $newInterest)
                            .onSubmit(addCustomInterest)
                        Button("Ajouter", action: addCustomInterest)
                            .disabled(newInterest.trimmingCharacters(in: .whitespaces).isEmpty)
                    }
                } header: {
                    Text("Centres d'intérêt")
                } footer: {
                    Text("\(viewModel.draftInterests.count) sur \(ProfileViewModel.interestLimit).")
                }
            }
            .navigationTitle("Modifier")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Annuler") {
                        viewModel.resetDraft()
                        dismiss()
                    }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Enregistrer") {
                        Task { @MainActor in
                            if await viewModel.save() { dismiss() }
                        }
                    }
                    .disabled(!viewModel.canSave)
                }
            }
        }
    }

    private func addCustomInterest() {
        let trimmed = newInterest.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty else { return }
        viewModel.toggleInterest(trimmed)
        newInterest = ""
    }
}

#Preview {
    EditProfileView(
        viewModel: ProfileViewModel(
            profiles: AppEnvironment.preview.profiles,
            session: SessionStore(auth: AppEnvironment.preview.auth)
        )
    )
}
