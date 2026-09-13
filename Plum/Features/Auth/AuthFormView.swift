import SwiftUI

/// One form for both sign-in and sign-up: the fields differ, the plumbing does
/// not.
struct AuthFormView: View {
    @State var viewModel: AuthViewModel
    @Environment(\.dismiss) private var dismiss
    @FocusState private var focusedField: Field?

    private enum Field: Hashable {
        case email, password, displayName
    }

    var body: some View {
        PlumBackground {
            ScrollView {
                VStack(alignment: .leading, spacing: PlumTheme.Spacing.l) {
                    header

                    VStack(spacing: PlumTheme.Spacing.m) {
                        if viewModel.mode == .signUp {
                            labelledField("Prénom") {
                                TextField("Comment on vous appelle ?", text: $viewModel.displayName)
                                    .textContentType(.givenName)
                                    .focused($focusedField, equals: .displayName)
                                    .submitLabel(.next)
                                    .onSubmit { focusedField = .email }
                            }
                        }

                        labelledField("Email") {
                            TextField("vous@exemple.fr", text: $viewModel.email)
                                .textContentType(.emailAddress)
                                .keyboardType(.emailAddress)
                                .textInputAutocapitalization(.never)
                                .autocorrectionDisabled()
                                .focused($focusedField, equals: .email)
                                .submitLabel(.next)
                                .onSubmit { focusedField = .password }
                        }

                        labelledField("Mot de passe", hint: viewModel.passwordHint) {
                            SecureField("8 caractères minimum", text: $viewModel.password)
                                .textContentType(viewModel.mode == .signUp ? .newPassword : .password)
                                .focused($focusedField, equals: .password)
                                .submitLabel(.go)
                                .onSubmit { Task { await viewModel.submit() } }
                        }

                        if viewModel.mode == .signUp {
                            birthDateField
                            genderField
                        }
                    }

                    if let errorMessage = viewModel.errorMessage {
                        ErrorBanner(message: errorMessage)
                            .padding(.horizontal, -PlumTheme.Spacing.m)
                    }

                    Button(viewModel.mode.callToAction) {
                        Task { await viewModel.submit() }
                    }
                    .buttonStyle(PlumPrimaryButtonStyle(isLoading: viewModel.isSubmitting))
                    .disabled(!viewModel.canSubmit)
                    .opacity(viewModel.canSubmit ? 1 : 0.5)

                    Spacer(minLength: PlumTheme.Spacing.xl)
                }
                .padding(PlumTheme.Spacing.l)
            }
            .scrollDismissesKeyboard(.interactively)
        }
        .navigationBarTitleDisplayMode(.inline)
        .animation(.easeInOut(duration: 0.2), value: viewModel.errorMessage)
        .onChange(of: viewModel.email) { _, _ in viewModel.clearError() }
        .onChange(of: viewModel.password) { _, _ in viewModel.clearError() }
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: PlumTheme.Spacing.s) {
            Text(viewModel.mode.title)
                .font(.plumDisplay)
                .foregroundStyle(PlumTheme.Palette.primaryText)
            Text(viewModel.mode == .signUp
                 ? "Trois champs et vous êtes dans le bain."
                 : "Vos matchs vous attendent, probablement.")
                .font(.plumCallout)
                .foregroundStyle(PlumTheme.Palette.secondaryText)
        }
    }

    private var birthDateField: some View {
        VStack(alignment: .leading, spacing: PlumTheme.Spacing.xs) {
            Text("Date de naissance").plumSectionHeader()
            DatePicker(
                "Date de naissance",
                selection: $viewModel.birthDate,
                in: ...Date.now,
                displayedComponents: .date
            )
            .labelsHidden()
            .datePickerStyle(.compact)

            if !AuthViewModel.isOldEnough(viewModel.birthDate) {
                Text("Plum est réservé aux majeurs.")
                    .font(.plumCaption)
                    .foregroundStyle(PlumTheme.Palette.pass)
            }
        }
    }

    private var genderField: some View {
        VStack(alignment: .leading, spacing: PlumTheme.Spacing.xs) {
            Text("Je suis").plumSectionHeader()
            Picker("Je suis", selection: $viewModel.gender) {
                ForEach(Gender.allCases) { gender in
                    Text(gender.label).tag(gender)
                }
            }
            .pickerStyle(.segmented)
        }
    }

    /// Keeps the label / field / hint stack identical for every input.
    private func labelledField<Content: View>(
        _ title: String,
        hint: String? = nil,
        @ViewBuilder content: () -> Content
    ) -> some View {
        VStack(alignment: .leading, spacing: PlumTheme.Spacing.xs) {
            Text(title).plumSectionHeader()
            content()
                .font(.plumBody)
                .padding(PlumTheme.Spacing.m)
                .background(
                    RoundedRectangle(cornerRadius: PlumTheme.Radius.small, style: .continuous)
                        .fill(PlumTheme.Palette.surface)
                )
            if let hint {
                Text(hint)
                    .font(.plumCaption)
                    .foregroundStyle(PlumTheme.Palette.pass)
            }
        }
    }
}

#Preview {
    NavigationStack {
        AuthFormView(
            viewModel: AuthViewModel(
                mode: .signUp,
                auth: AppEnvironment.preview.auth,
                session: SessionStore(auth: AppEnvironment.preview.auth)
            )
        )
    }
}
