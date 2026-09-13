import SwiftUI

/// The full profile behind a card.
///
/// The deck only ever showed three lines of bio and a strip of interests,
/// which is not enough to decide anything — so people swiped on photos alone.
/// This is the screen that makes a considered decision possible, with the same
/// three verdicts available from inside it.
struct ProfileDetailView: View {
    let profile: Profile
    let onDecision: (SwipeDecision) -> Void
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

    /// The same three verdicts as the deck, so a decision made here does not
    /// require backing out and finding the card again.
    private var actions: some View {
        HStack(spacing: PlumTheme.Spacing.l) {
            CircularActionButton(
                systemImage: "xmark",
                tint: PlumTheme.Palette.pass,
                accessibilityTitle: "Passer"
            ) {
                decide(.pass)
            }

            CircularActionButton(
                systemImage: "star.fill",
                tint: PlumTheme.Palette.superLike,
                diameter: 48,
                accessibilityTitle: "Coup de cœur"
            ) {
                decide(.superLike)
            }

            CircularActionButton(
                systemImage: "heart.fill",
                tint: PlumTheme.Palette.like,
                isProminent: true,
                accessibilityTitle: "J'aime"
            ) {
                decide(.like)
            }
        }
        .padding(.vertical, PlumTheme.Spacing.m)
        .frame(maxWidth: .infinity)
        .background(.bar)
    }

    private func decide(_ decision: SwipeDecision) {
        onDecision(decision)
        dismiss()
    }
}

#Preview {
    ProfileDetailView(profile: SampleData.deck[0], onDecision: { _ in })
}
