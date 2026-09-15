import SwiftUI

/// Le profil entier, derrière la carte.
///
/// La carte montre la photo, le nom et les deux phrases. Ici il y a les autres
/// photos et les centres d'intérêt, pour qui veut regarder avant d'écrire —
/// et les deux mêmes issues, pour ne pas avoir à refermer et retrouver la
/// carte.
struct ProfileDetailView: View {
    let profile: Profile
    let onWrite: () -> Void
    let onPass: () -> Void
    var onReport: ((String) -> Void)?
    var onBlock: (() -> Void)?

    @Environment(\.dismiss) private var dismiss
    @State private var photoIndex = 0
    @State private var isReporting = false

    var body: some View {
        NavigationStack {
            PlumBackground {
                ScrollView {
                    VStack(alignment: .leading, spacing: PlumTheme.Spacing.l) {
                        carousel
                        identity
                        if !profile.bio.isEmpty { bio }
                        if !profile.interests.isEmpty { interests }
                        Spacer(minLength: PlumTheme.Spacing.xl)
                    }
                }
                .scrollIndicators(.hidden)
                .accessibilityIdentifier("feuille-profil")
            }
            .safeAreaInset(edge: .bottom) { actions }
            .navigationBarTitleDisplayMode(.inline)
            // Presented from here rather than bounced back to the deck:
            // handing the deck a new sheet while this one dismisses cancels
            // it, because both would ride the same binding.
            .sheet(isPresented: $isReporting) {
                ReportSheet(profile: profile) { reason in
                    onReport?(reason)
                    dismiss()
                } onBlock: {
                    onBlock?()
                    dismiss()
                }
            }
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Fermer") { dismiss() }
                }
                ToolbarItem(placement: .topBarTrailing) {
                    Button(role: .destructive) {
                        isReporting = true
                    } label: {
                        Label("Signaler", systemImage: "flag")
                    }
                    .accessibilityLabel(Text("Signaler ce profil"))
                }
            }
        }
    }

    private var photos: [Photo] { profile.orderedPhotos }

    private var carousel: some View {
        TabView(selection: $photoIndex) {
            ForEach(Array(photos.enumerated()), id: \.element.id) { index, photo in
                RemoteImage(url: photo.url, seed: photo.id.uuidString)
                    .tag(index)
            }
        }
        .tabViewStyle(.page(indexDisplayMode: photos.count > 1 ? .automatic : .never))
        .frame(height: 460)
        .clipShape(RoundedRectangle(cornerRadius: PlumTheme.Radius.card, style: .continuous))
        .padding(.horizontal, PlumTheme.Spacing.m)
        .accessibilityLabel(Text("Photos de \(profile.displayName)"))
    }

    private var identity: some View {
        VStack(alignment: .leading, spacing: PlumTheme.Spacing.xs) {
            HStack(alignment: .firstTextBaseline, spacing: PlumTheme.Spacing.s) {
                Text(profile.nameAndAge)
                    .font(.plumDisplay)
                if profile.isRecentlyActive {
                    Circle()
                        .fill(PlumTheme.Palette.mint)
                        .frame(width: 10, height: 10)
                }
            }
            Text(profile.locationLine)
                .font(.plumCallout)
                .foregroundStyle(PlumTheme.Palette.secondaryText)
            if let activity = profile.activityLine {
                Text(activity)
                    .font(.plumCaption)
                    .foregroundStyle(PlumTheme.Palette.secondaryText)
            }
        }
        .padding(.horizontal, PlumTheme.Spacing.l)
    }

    private var bio: some View {
        VStack(alignment: .leading, spacing: PlumTheme.Spacing.xs) {
            Text("À propos").plumSectionHeader()
            Text(profile.bio)
                .font(.plumBody)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(.horizontal, PlumTheme.Spacing.l)
    }

    private var interests: some View {
        VStack(alignment: .leading, spacing: PlumTheme.Spacing.s) {
            Text("Centres d'intérêt").plumSectionHeader()
            FlowLayout(spacing: PlumTheme.Spacing.s, alignment: .leading) {
                ForEach(profile.interests, id: \.self) { interest in
                    InterestChip(title: interest)
                }
            }
        }
        .padding(.horizontal, PlumTheme.Spacing.l)
    }

    /// Les deux mêmes issues que sur la carte.
    ///
    /// Des boutons nommés, pas des ronds à icône : « Écrire » et « Passer »
    /// disent ce qu'ils font. Les trois ronds d'avant — croix, étoile, cœur —
    /// se devinaient par habitude d'une autre application, ce qui est
    /// précisément ce dont on sort.
    private var actions: some View {
        HStack(spacing: PlumTheme.Spacing.s) {
            Button {
                onPass()
                dismiss()
            } label: {
                Text("Passer").frame(maxWidth: .infinity)
            }
            .buttonStyle(PlumSecondaryButtonStyle())

            Button {
                onWrite()
                dismiss()
            } label: {
                Label("Écrire", systemImage: "square.and.pencil")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(PlumPrimaryButtonStyle())
        }
        .padding(PlumTheme.Spacing.m)
        .frame(maxWidth: .infinity)
        .background(.bar)
    }
}

#Preview {
    ProfileDetailView(profile: SampleData.selection[0], onWrite: {}, onPass: {})
}
