import SwiftUI

/// A single profile card. It draws itself and reports the drag; deciding what
/// a drag *means* is the deck's job.
struct SwipeCardView: View {
    let profile: Profile
    var dragTranslation: CGSize = .zero
    var isTopCard = false

    @State private var photoIndex = 0

    private var photos: [Photo] { profile.orderedPhotos }

    var body: some View {
        GeometryReader { proxy in
            ZStack(alignment: .bottom) {
                photo
                PlumTheme.Palette.cardScrim
                details
                if isTopCard {
                    verdictOverlays
                }
                if photos.count > 1 {
                    photoIndicators
                        .frame(maxHeight: .infinity, alignment: .top)
                }
            }
            .frame(width: proxy.size.width, height: proxy.size.height)
            .clipShape(RoundedRectangle(cornerRadius: PlumTheme.Radius.card, style: .continuous))
            .shadow(color: .black.opacity(0.18), radius: 18, y: 10)
            .contentShape(Rectangle())
            // Tapping the right half advances the photos, the left half goes
            // back — the gesture everybody already knows.
            .onTapGesture { location in
                advancePhoto(forward: location.x > proxy.size.width / 2)
            }
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel(Text("\(profile.nameAndAge), \(profile.locationLine)"))
    }

    private var photo: some View {
        RemoteImage(
            url: photos.indices.contains(photoIndex) ? photos[photoIndex].url : photos.first?.url,
            seed: profile.displayName + String(photoIndex)
        )
        .id(photoIndex)
        .transition(.opacity)
    }

    private var photoIndicators: some View {
        HStack(spacing: 4) {
            ForEach(photos.indices, id: \.self) { index in
                Capsule()
                    .fill(index == photoIndex ? Color.white : Color.white.opacity(0.35))
                    .frame(height: 3)
            }
        }
        .padding(.horizontal, PlumTheme.Spacing.m)
        .padding(.top, PlumTheme.Spacing.m)
    }

    private var details: some View {
        VStack(alignment: .leading, spacing: PlumTheme.Spacing.s) {
            HStack(alignment: .firstTextBaseline, spacing: PlumTheme.Spacing.s) {
                Text(profile.nameAndAge)
                    .font(.plumTitle)
                if profile.isRecentlyActive {
                    Circle()
                        .fill(PlumTheme.Palette.mint)
                        .frame(width: 9, height: 9)
                }
            }

            Text(profile.locationLine)
                .font(.plumCallout)
                .opacity(0.9)

            if !profile.bio.isEmpty {
                Text(profile.bio)
                    .font(.plumCallout)
                    .lineLimit(3)
                    .opacity(0.95)
            }

            if !profile.interests.isEmpty {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: PlumTheme.Spacing.s) {
                        ForEach(profile.interests, id: \.self) { interest in
                            Text(interest)
                                .font(.plumCaption)
                                .padding(.horizontal, 12)
                                .padding(.vertical, 6)
                                .background(.ultraThinMaterial, in: Capsule())
                        }
                    }
                }
                .scrollClipDisabled()
            }
        }
        .foregroundStyle(.white)
        .padding(PlumTheme.Spacing.l)
    }

    /// LIKE / NOPE / SUPER stamps whose opacity tracks the drag, so the
    /// gesture tells you what it is about to do before you let go.
    private var verdictOverlays: some View {
        ZStack {
            stamp("J'AIME", color: PlumTheme.Palette.like, rotation: -18)
                .opacity(opacity(for: dragTranslation.width, threshold: PlumTheme.Layout.swipeCommitThreshold))
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)

            stamp("NON", color: PlumTheme.Palette.pass, rotation: 18)
                .opacity(opacity(for: -dragTranslation.width, threshold: PlumTheme.Layout.swipeCommitThreshold))
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topTrailing)

            stamp("COUP DE CŒUR", color: PlumTheme.Palette.superLike, rotation: 0)
                .opacity(opacity(for: -dragTranslation.height, threshold: PlumTheme.Layout.swipeUpCommitThreshold))
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottom)
                .padding(.bottom, 120)
        }
        .padding(PlumTheme.Spacing.l)
        .allowsHitTesting(false)
    }

    private func stamp(_ title: String, color: Color, rotation: Double) -> some View {
        Text(title)
            .font(.system(.title, design: .rounded).weight(.heavy))
            .foregroundStyle(color)
            .padding(.horizontal, PlumTheme.Spacing.m)
            .padding(.vertical, PlumTheme.Spacing.s)
            .overlay(
                RoundedRectangle(cornerRadius: PlumTheme.Radius.small, style: .continuous)
                    .stroke(color, lineWidth: 4)
            )
            .rotationEffect(.degrees(rotation))
    }

    private func opacity(for value: CGFloat, threshold: CGFloat) -> Double {
        guard value > 0 else { return 0 }
        return Double(min(1, value / threshold))
    }

    private func advancePhoto(forward: Bool) {
        guard photos.count > 1 else { return }
        withAnimation(.easeInOut(duration: 0.15)) {
            if forward {
                photoIndex = min(photos.count - 1, photoIndex + 1)
            } else {
                photoIndex = max(0, photoIndex - 1)
            }
        }
    }
}

#Preview {
    SwipeCardView(profile: SampleData.deck[0], isTopCard: true)
        .padding()
        .frame(height: 560)
}
