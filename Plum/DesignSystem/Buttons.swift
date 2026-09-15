import SwiftUI

/// The filled, full-width call to action.
struct PlumPrimaryButtonStyle: ButtonStyle {
    var isLoading = false

    func makeBody(configuration: Configuration) -> some View {
        ZStack {
            configuration.label
                .opacity(isLoading ? 0 : 1)
            if isLoading {
                ProgressView()
                    .tint(.white)
            }
        }
        .font(.plumButton)
        .foregroundStyle(.white)
        .frame(maxWidth: .infinity, minHeight: 54)
        .background(PlumTheme.Palette.warmGradient)
        .clipShape(RoundedRectangle(cornerRadius: PlumTheme.Radius.medium, style: .continuous))
        .opacity(configuration.isPressed ? 0.85 : 1)
        .scaleEffect(configuration.isPressed ? 0.98 : 1)
        .animation(.spring(response: 0.3, dampingFraction: 0.7), value: configuration.isPressed)
    }
}

/// The quieter sibling: same footprint, no fill.
struct PlumSecondaryButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.plumButton)
            .foregroundStyle(PlumTheme.Palette.plum)
            .frame(maxWidth: .infinity, minHeight: 54)
            .background(
                RoundedRectangle(cornerRadius: PlumTheme.Radius.medium, style: .continuous)
                    .stroke(PlumTheme.Palette.plum.opacity(0.35), lineWidth: 1.5)
            )
            .opacity(configuration.isPressed ? 0.6 : 1)
    }
}
