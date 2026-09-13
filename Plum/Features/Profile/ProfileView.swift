import PhotosUI
import SwiftUI

/// Your own card, as others see it, with everything editable in place.
struct ProfileView: View {
    @Environment(\.services) private var services
    @Environment(SessionStore.self) private var session
    @State private var viewModel: ProfileViewModel?
    @State private var isEditing = false
    @State private var isShowingSettings = false
    @State private var pickedPhoto: PhotosPickerItem?

    var body: some View {
        NavigationStack {
            PlumBackground {
                content
            }
            .navigationTitle("Profil")
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button {
                        isShowingSettings = true
                    } label: {
                        Image(systemName: "gearshape")
                    }
                }
            }
            .sheet(isPresented: $isEditing) {
                if let viewModel {
                    EditProfileView(viewModel: viewModel)
                }
            }
            .sheet(isPresented: $isShowingSettings) {
                if let viewModel {
                    SettingsView(viewModel: viewModel)
                }
            }
        }
        .task {
            if viewModel == nil {
                viewModel = ProfileViewModel(profiles: services.profiles, session: session)
            }
            await viewModel?.load()
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

    @ViewBuilder
    private var content: some View {
        if let viewModel, let profile = viewModel.profile {
            ScrollView {
                VStack(spacing: PlumTheme.Spacing.l) {
                    photoGrid(profile, viewModel: viewModel)

                    VStack(spacing: PlumTheme.Spacing.xs) {
                        Text(profile.nameAndAge)
                            .font(.plumDisplay)
                        Text(profile.city)
                            .font(.plumCallout)
                            .foregroundStyle(PlumTheme.Palette.secondaryText)
                    }

                    if !profile.bio.isEmpty {
                        Text(profile.bio)
                            .font(.plumBody)
                            .multilineTextAlignment(.center)
                            .padding(.horizontal, PlumTheme.Spacing.l)
                    }

                    if !profile.interests.isEmpty {
                        FlowLayout(spacing: PlumTheme.Spacing.s) {
                            ForEach(profile.interests, id: \.self) { interest in
                                InterestChip(title: interest)
                            }
                        }
                        .padding(.horizontal, PlumTheme.Spacing.l)
                    }

                    Button("Modifier mon profil") { isEditing = true }
                        .buttonStyle(PlumSecondaryButtonStyle())
                        .padding(.horizontal, PlumTheme.Spacing.l)

                    if let error = viewModel.state.error {
                        ErrorBanner(message: error.userMessage) {
                            Task { await viewModel.load() }
                        }
                    }
                }
                .padding(.vertical, PlumTheme.Spacing.l)
            }
            .refreshable { await viewModel.load() }
        } else if viewModel?.state.isLoading ?? true {
            ProgressView().tint(PlumTheme.Palette.plum)
        } else {
            EmptyStateView(
                systemImage: "person.crop.circle.badge.exclamationmark",
                title: "Profil indisponible",
                message: "On n'a pas réussi à charger votre profil.",
                actionTitle: "Réessayer"
            ) {
                Task { await viewModel?.load() }
            }
        }
    }

    private func photoGrid(_ profile: Profile, viewModel: ProfileViewModel) -> some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: PlumTheme.Spacing.m) {
                ForEach(profile.orderedPhotos) { photo in
                    RemoteImage(url: photo.url, seed: photo.id.uuidString)
                        .frame(width: 180, height: 240)
                        .clipShape(RoundedRectangle(cornerRadius: PlumTheme.Radius.medium, style: .continuous))
                        .overlay(alignment: .topLeading) {
                            if photo.id == profile.coverPhoto?.id {
                                Text("Couverture")
                                    .font(.plumCaption)
                                    .padding(.horizontal, PlumTheme.Spacing.s)
                                    .padding(.vertical, 4)
                                    .background(.ultraThinMaterial, in: Capsule())
                                    .padding(PlumTheme.Spacing.s)
                            }
                        }
                        .contextMenu {
                            if photo.id != profile.coverPhoto?.id {
                                Button {
                                    Task { await viewModel.makeCover(photo) }
                                } label: {
                                    Label("Mettre en couverture", systemImage: "star")
                                }
                            }
                            Button(role: .destructive) {
                                Task { await viewModel.deletePhoto(photo) }
                            } label: {
                                Label("Supprimer", systemImage: "trash")
                            }
                        }
                        .overlay(alignment: .topTrailing) {
                            Button {
                                Task { await viewModel.deletePhoto(photo) }
                            } label: {
                                Image(systemName: "xmark.circle.fill")
                                    .font(.title3)
                                    .foregroundStyle(.white, .black.opacity(0.45))
                            }
                            .padding(PlumTheme.Spacing.s)
                        }
                }

                if profile.photos.count < 6 {
                    PhotosPicker(selection: $pickedPhoto, matching: .images) {
                        VStack(spacing: PlumTheme.Spacing.s) {
                            Image(systemName: "plus")
                                .font(.title)
                            Text("Ajouter")
                                .font(.plumCaption)
                        }
                        .foregroundStyle(PlumTheme.Palette.plum)
                        .frame(width: 180, height: 240)
                        .background(
                            RoundedRectangle(cornerRadius: PlumTheme.Radius.medium, style: .continuous)
                                .strokeBorder(
                                    PlumTheme.Palette.plum.opacity(0.4),
                                    style: StrokeStyle(lineWidth: 2, dash: [8])
                                )
                        )
                    }
                }
            }
            .padding(.horizontal, PlumTheme.Spacing.l)
        }
    }
}

#Preview {
    ProfileView()
        .environment(\.services, .preview)
        .environment(SessionStore(auth: AppEnvironment.preview.auth))
        .environment(DeckRefreshSignal())
}
