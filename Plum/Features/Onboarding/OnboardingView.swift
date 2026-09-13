import PhotosUI
import SwiftUI

/// The three screens between signing up and being swipeable.
struct OnboardingView: View {
    @Environment(\.services) private var services
    @Environment(SessionStore.self) private var session
    @State private var viewModel: OnboardingViewModel?
    @State private var pickedPhoto: PhotosPickerItem?

    var body: some View {
        PlumBackground {
            if let viewModel {
                content(viewModel)
            } else {
                ProgressView().tint(PlumTheme.Palette.plum)
            }
        }
        .task {
            if viewModel == nil {
                viewModel = OnboardingViewModel(profiles: services.profiles, session: session)
            }
        }
        .onChange(of: pickedPhoto) { _, item in
            guard let item else { return }
            Task { @MainActor in
                if let data = try? await item.loadTransferable(type: Data.self) {
                    await viewModel?.addPhoto(data)
                }
                pickedPhoto = nil
            }
        }
    }

    private func content(_ viewModel: OnboardingViewModel) -> some View {
        VStack(spacing: 0) {
            progressBar(viewModel)

            ScrollView {
                VStack(alignment: .leading, spacing: PlumTheme.Spacing.l) {
                    VStack(alignment: .leading, spacing: PlumTheme.Spacing.s) {
                        Text(viewModel.step.title)
                            .font(.plumDisplay)
                        Text(viewModel.step.subtitle)
                            .font(.plumCallout)
                            .foregroundStyle(PlumTheme.Palette.secondaryText)
                    }

                    switch viewModel.step {
                    case .photos:
                        photosStep(viewModel)
                    case .about:
                        aboutStep(viewModel)
                    case .preferences:
                        preferencesStep(viewModel)
                    }

                    if let errorMessage = viewModel.errorMessage {
                        ErrorBanner(message: errorMessage)
                            .padding(.horizontal, -PlumTheme.Spacing.l)
                    }
                }
                .padding(PlumTheme.Spacing.l)
            }

            footer(viewModel)
        }
    }

    private func progressBar(_ viewModel: OnboardingViewModel) -> some View {
        GeometryReader { proxy in
            ZStack(alignment: .leading) {
                Capsule().fill(PlumTheme.Palette.plum.opacity(0.12))
                Capsule()
                    .fill(PlumTheme.Palette.warmGradient)
                    .frame(width: proxy.size.width * viewModel.progress)
            }
        }
        .frame(height: 6)
        .padding(.horizontal, PlumTheme.Spacing.l)
        .padding(.top, PlumTheme.Spacing.m)
        .animation(.easeInOut(duration: 0.25), value: viewModel.progress)
        .accessibilityLabel(Text("Étape \(viewModel.step.rawValue + 1) sur \(OnboardingStep.allCases.count)"))
    }

    // MARK: - Steps

    private func photosStep(_ viewModel: OnboardingViewModel) -> some View {
        LazyVGrid(
            columns: [GridItem(.flexible(), spacing: PlumTheme.Spacing.m),
                      GridItem(.flexible(), spacing: PlumTheme.Spacing.m)],
            spacing: PlumTheme.Spacing.m
        ) {
            ForEach(viewModel.photos) { photo in
                RemoteImage(url: photo.url, seed: photo.id.uuidString)
                    .frame(height: 200)
                    .clipShape(RoundedRectangle(cornerRadius: PlumTheme.Radius.medium, style: .continuous))
                    .overlay(alignment: .topTrailing) {
                        Button {
                            Task { await viewModel.removePhoto(photo) }
                        } label: {
                            Image(systemName: "xmark.circle.fill")
                                .font(.title3)
                                .foregroundStyle(.white, .black.opacity(0.45))
                        }
                        .padding(PlumTheme.Spacing.s)
                    }
            }

            if viewModel.canAddMorePhotos {
                PhotosPicker(selection: $pickedPhoto, matching: .images) {
                    VStack(spacing: PlumTheme.Spacing.s) {
                        Image(systemName: viewModel.isWorking ? "hourglass" : "plus")
                            .font(.title)
                        Text("Ajouter")
                            .font(.plumCaption)
                    }
                    .foregroundStyle(PlumTheme.Palette.plum)
                    .frame(maxWidth: .infinity)
                    .frame(height: 200)
                    .background(
                        RoundedRectangle(cornerRadius: PlumTheme.Radius.medium, style: .continuous)
                            .strokeBorder(
                                PlumTheme.Palette.plum.opacity(0.4),
                                style: StrokeStyle(lineWidth: 2, dash: [8])
                            )
                    )
                }
                .disabled(viewModel.isWorking)
            }
        }
    }

    private func aboutStep(_ viewModel: OnboardingViewModel) -> some View {
        @Bindable var model = viewModel

        return VStack(alignment: .leading, spacing: PlumTheme.Spacing.l) {
            VStack(alignment: .leading, spacing: PlumTheme.Spacing.xs) {
                Text("Ville").plumSectionHeader()
                TextField("Où êtes-vous ?", text: $model.city)
                    .textContentType(.addressCity)
                    .font(.plumBody)
                    .padding(PlumTheme.Spacing.m)
                    .background(
                        RoundedRectangle(cornerRadius: PlumTheme.Radius.small, style: .continuous)
                            .fill(PlumTheme.Palette.surface)
                    )
            }

            VStack(alignment: .leading, spacing: PlumTheme.Spacing.xs) {
                Text("Bio").plumSectionHeader()
                TextField("Deux phrases, pas un CV.", text: $model.bio, axis: .vertical)
                    .lineLimit(3...6)
                    .font(.plumBody)
                    .padding(PlumTheme.Spacing.m)
                    .background(
                        RoundedRectangle(cornerRadius: PlumTheme.Radius.small, style: .continuous)
                            .fill(PlumTheme.Palette.surface)
                    )
            }

            VStack(alignment: .leading, spacing: PlumTheme.Spacing.s) {
                Text("Centres d'intérêt").plumSectionHeader()
                FlowLayout(spacing: PlumTheme.Spacing.s, alignment: .leading) {
                    ForEach(ProfileViewModel.suggestedInterests, id: \.self) { interest in
                        Button {
                            viewModel.toggleInterest(interest)
                        } label: {
                            InterestChip(
                                title: interest,
                                isSelected: viewModel.interests.contains(interest)
                            )
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
        }
    }

    private func preferencesStep(_ viewModel: OnboardingViewModel) -> some View {
        @Bindable var model = viewModel

        return VStack(alignment: .leading, spacing: PlumTheme.Spacing.l) {
            VStack(alignment: .leading, spacing: PlumTheme.Spacing.s) {
                Text("Me montrer").plumSectionHeader()
                Picker("Me montrer", selection: $model.preferences.interestedIn) {
                    ForEach(GenderPreference.allCases) { preference in
                        Text(preference.label).tag(preference)
                    }
                }
                .pickerStyle(.segmented)
            }

            VStack(alignment: .leading, spacing: PlumTheme.Spacing.s) {
                Text("Âge").plumSectionHeader()
                Stepper("À partir de \(model.preferences.minAge) ans",
                        value: $model.preferences.minAge, in: 18...99)
                Stepper("Jusqu'à \(model.preferences.maxAge) ans",
                        value: $model.preferences.maxAge, in: max(18, model.preferences.minAge)...99)
            }

            VStack(alignment: .leading, spacing: PlumTheme.Spacing.s) {
                Text("Distance").plumSectionHeader()
                Text("Jusqu'à \(model.preferences.maxDistanceKm) km")
                    .font(.plumCallout)
                Slider(
                    value: Binding(
                        get: { Double(model.preferences.maxDistanceKm) },
                        set: { model.preferences.maxDistanceKm = Int($0) }
                    ),
                    in: 1...300,
                    step: 1
                )
            }
        }
    }

    // MARK: - Footer

    private func footer(_ viewModel: OnboardingViewModel) -> some View {
        VStack(spacing: PlumTheme.Spacing.s) {
            if let reason = viewModel.blockedReason {
                Text(reason)
                    .font(.plumCaption)
                    .foregroundStyle(PlumTheme.Palette.secondaryText)
            }

            HStack(spacing: PlumTheme.Spacing.m) {
                if viewModel.step != .photos {
                    Button("Retour") { viewModel.goBack() }
                        .buttonStyle(PlumSecondaryButtonStyle())
                        .frame(maxWidth: 120)
                }

                Button(viewModel.isLastStep ? "Commencer" : "Continuer") {
                    Task { @MainActor in _ = await viewModel.advance() }
                }
                .buttonStyle(PlumPrimaryButtonStyle(isLoading: viewModel.isWorking))
                .disabled(!viewModel.canAdvance)
                .opacity(viewModel.canAdvance ? 1 : 0.5)
            }
        }
        .padding(PlumTheme.Spacing.l)
        .background(.bar)
    }
}

#Preview {
    OnboardingView()
        .environment(\.services, .preview)
        .environment(SessionStore(auth: AppEnvironment.preview.auth))
}
