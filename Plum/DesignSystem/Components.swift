import SwiftUI

/// Remote image with a branded placeholder, used everywhere a photo appears.
/// The gradient fallback means the app still looks deliberate when a photo
/// fails to load or the demo mode is running offline.
struct RemoteImage: View {
    let url: URL?
    var seed: String = ""

    var body: some View {
        AsyncImage(url: url, transaction: Transaction(animation: .easeOut(duration: 0.25))) { phase in
            switch phase {
            case let .success(image):
                image
                    .resizable()
                    .scaledToFill()
            case .failure:
                placeholder
            case .empty:
                placeholder.overlay(ProgressView().tint(.white))
            @unknown default:
                placeholder
            }
        }
    }

    private var placeholder: some View {
        LinearGradient(
            colors: Self.gradientColors(for: seed),
            startPoint: .topLeading,
            endPoint: .bottomTrailing
        )
    }

    /// Derives a stable pair of brand-adjacent colours from the seed, so two
    /// profiles never share a placeholder by accident.
    ///
    /// `hashValue` would be wrong twice over here: it is seeded per process,
    /// so the same profile would change colour between launches, and
    /// `abs(Int.min)` traps. This is a plain FNV-1a instead.
    static func gradientColors(for seed: String) -> [Color] {
        var hash: UInt64 = 0xcbf2_9ce4_8422_2325
        for byte in seed.utf8 {
            hash ^= UInt64(byte)
            hash &*= 0x1000_0000_01b3
        }
        let hue = Double(hash % 360) / 360
        return [
            Color(hue: hue, saturation: 0.35, brightness: 0.75),
            PlumTheme.Palette.plum
        ]
    }
}

struct InterestChip: View {
    let title: String
    var isSelected = false

    var body: some View {
        Text(title)
            .font(.plumCaption)
            .padding(.horizontal, PlumTheme.Spacing.m)
            .padding(.vertical, PlumTheme.Spacing.s)
            .background {
                Capsule()
                    .fill(isSelected
                          ? AnyShapeStyle(PlumTheme.Palette.plum)
                          : AnyShapeStyle(PlumTheme.Palette.plum.opacity(0.1)))
            }
            .foregroundStyle(isSelected ? .white : PlumTheme.Palette.plum)
    }
}

struct AvatarView: View {
    let profile: Profile
    var diameter: CGFloat = 56
    var showsActivityDot = false

    var body: some View {
        RemoteImage(url: profile.coverPhoto?.url, seed: profile.displayName)
            .frame(width: diameter, height: diameter)
            .clipShape(Circle())
            .overlay(alignment: .bottomTrailing) {
                if showsActivityDot, profile.isRecentlyActive {
                    Circle()
                        .fill(PlumTheme.Palette.mint)
                        .frame(width: diameter * 0.24, height: diameter * 0.24)
                        .overlay(Circle().stroke(PlumTheme.Palette.surface, lineWidth: 2))
                }
            }
            .accessibilityLabel(Text(profile.displayName))
    }
}

/// The empty state used by the deck, the matches list and the inbox.
struct EmptyStateView: View {
    let systemImage: String
    let title: String
    let message: String
    var actionTitle: String?
    var action: (() -> Void)?

    var body: some View {
        VStack(spacing: PlumTheme.Spacing.m) {
            Image(systemName: systemImage)
                .font(.system(size: 46))
                .foregroundStyle(PlumTheme.Palette.plum.opacity(0.5))
            Text(title)
                .font(.plumTitle)
                .multilineTextAlignment(.center)
            Text(message)
                .font(.plumCallout)
                .foregroundStyle(PlumTheme.Palette.secondaryText)
                .multilineTextAlignment(.center)
            if let actionTitle, let action {
                Button(actionTitle, action: action)
                    .buttonStyle(PlumSecondaryButtonStyle())
                    .padding(.top, PlumTheme.Spacing.s)
                    .frame(maxWidth: 260)
            }
        }
        .padding(PlumTheme.Spacing.xl)
        .frame(maxWidth: .infinity)
    }
}

/// A dismissible error strip. Errors in this app are never fatal: there is
/// always something else to swipe.
struct ErrorBanner: View {
    let message: String
    var retry: (() -> Void)?

    var body: some View {
        HStack(spacing: PlumTheme.Spacing.s) {
            Image(systemName: "exclamationmark.triangle.fill")
            Text(message)
                .font(.plumCallout)
                .fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: 0)
            if let retry {
                Button("Réessayer", action: retry)
                    .font(.plumCaption)
            }
        }
        .padding(PlumTheme.Spacing.m)
        .foregroundStyle(.white)
        .background(PlumTheme.Palette.pass, in: RoundedRectangle(cornerRadius: PlumTheme.Radius.small, style: .continuous))
        .padding(.horizontal, PlumTheme.Spacing.m)
        .transition(.move(edge: .top).combined(with: .opacity))
    }
}

/// Wraps a screen so every feature gets the same background treatment.
struct PlumBackground<Content: View>: View {
    @ViewBuilder var content: Content

    var body: some View {
        ZStack {
            PlumTheme.Palette.canvas
                .ignoresSafeArea()
            content
        }
    }
}
