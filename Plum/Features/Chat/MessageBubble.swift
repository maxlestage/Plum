import SwiftUI

/// One message. Mine on the right in plum, theirs on the left on a neutral
/// surface — the arrangement everybody can already read without a legend.
struct MessageBubble: View {
    let item: ChatItem
    let isMine: Bool
    var showsTimestamp = false
    var onRetry: (() -> Void)?

    var body: some View {
        VStack(alignment: isMine ? .trailing : .leading, spacing: 2) {
            Text(item.message.body)
                .font(.plumBody)
                .foregroundStyle(isMine ? .white : PlumTheme.Palette.primaryText)
                .padding(.horizontal, PlumTheme.Spacing.m)
                .padding(.vertical, 10)
                .background {
                    BubbleShape(isMine: isMine)
                        .fill(isMine
                              ? AnyShapeStyle(PlumTheme.Palette.warmGradient)
                              : AnyShapeStyle(PlumTheme.Palette.surface))
                }
                .frame(maxWidth: 280, alignment: isMine ? .trailing : .leading)
                .opacity(item.deliveryState == .sending ? 0.65 : 1)

            footer
        }
        .frame(maxWidth: .infinity, alignment: isMine ? .trailing : .leading)
        .accessibilityElement(children: .combine)
        .accessibilityLabel(
            Text("\(isMine ? "Vous" : "Elle ou il") : \(item.message.body)")
        )
    }

    @ViewBuilder
    private var footer: some View {
        switch item.deliveryState {
        case .failed:
            Button {
                onRetry?()
            } label: {
                Label("Non envoyé — toucher pour réessayer", systemImage: "arrow.clockwise")
                    .font(.plumCaption)
                    .foregroundStyle(PlumTheme.Palette.pass)
            }
            .buttonStyle(.plain)
        case .sending:
            Text("Envoi…")
                .font(.plumCaption)
                .foregroundStyle(PlumTheme.Palette.secondaryText)
        case .sent:
            if showsTimestamp {
                HStack(spacing: 4) {
                    Text(RelativeDateFormatting.messageTimestamp(for: item.message.sentAt))
                    if isMine, item.message.readAt != nil {
                        Image(systemName: "checkmark.circle.fill")
                    }
                }
                .font(.plumCaption)
                .foregroundStyle(PlumTheme.Palette.secondaryText)
            }
        }
    }
}

/// A rounded rectangle with the corner nearest the sender squared off, so the
/// bubble points at whoever said it.
struct BubbleShape: Shape {
    let isMine: Bool

    func path(in rect: CGRect) -> Path {
        let radius: CGFloat = 18
        let small: CGFloat = 4
        return Path(
            roundedRect: rect,
            cornerRadii: RectangleCornerRadii(
                topLeading: radius,
                bottomLeading: isMine ? radius : small,
                bottomTrailing: isMine ? small : radius,
                topTrailing: radius
            ),
            style: .continuous
        )
    }
}

/// The three-dot indicator, animated with a phase timer rather than a
/// repeating animation so it stops cleanly when it disappears.
struct TypingIndicator: View {
    @State private var phase = 0

    private let timer = Timer.publish(every: 0.35, on: .main, in: .common).autoconnect()

    var body: some View {
        HStack(spacing: 5) {
            ForEach(0..<3, id: \.self) { index in
                Circle()
                    .fill(PlumTheme.Palette.secondaryText)
                    .frame(width: 7, height: 7)
                    .scaleEffect(phase == index ? 1.3 : 0.8)
                    .opacity(phase == index ? 1 : 0.5)
            }
        }
        .padding(.horizontal, PlumTheme.Spacing.m)
        .padding(.vertical, 12)
        .background(PlumTheme.Palette.surface, in: Capsule())
        .onReceive(timer) { _ in
            withAnimation(.easeInOut(duration: 0.3)) {
                phase = (phase + 1) % 3
            }
        }
        .accessibilityLabel(Text("En train d'écrire"))
    }
}

#Preview {
    VStack(alignment: .leading, spacing: 12) {
        MessageBubble(
            item: ChatItem(message: SampleData.thread(for: SampleData.conversations[0])[0]),
            isMine: false,
            showsTimestamp: true
        )
        MessageBubble(
            item: ChatItem(message: SampleData.thread(for: SampleData.conversations[0])[1]),
            isMine: true,
            showsTimestamp: true
        )
        TypingIndicator()
    }
    .padding()
}
