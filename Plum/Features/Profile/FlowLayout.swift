import SwiftUI

/// Wraps chips onto as many lines as they need. `Layout` rather than a stack of
/// `HStack`s so the wrapping follows the real available width, including when
/// Dynamic Type makes every chip wider.
struct FlowLayout: Layout {
    var spacing: CGFloat = 8
    var alignment: HorizontalAlignment = .center

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout Void) -> CGSize {
        let width = proposal.replacingUnspecifiedDimensions().width
        let rows = layout(subviews: subviews, availableWidth: width)
        let height = rows.reduce(0) { $0 + $1.height } + CGFloat(max(0, rows.count - 1)) * spacing
        return CGSize(width: width, height: height)
    }

    func placeSubviews(
        in bounds: CGRect,
        proposal: ProposedViewSize,
        subviews: Subviews,
        cache: inout Void
    ) {
        let rows = layout(subviews: subviews, availableWidth: bounds.width)
        var y = bounds.minY

        for row in rows {
            var x = bounds.minX + leadingOffset(for: row, in: bounds.width)
            for item in row.items {
                let size = subviews[item].sizeThatFits(.unspecified)
                subviews[item].place(
                    at: CGPoint(x: x, y: y + (row.height - size.height) / 2),
                    proposal: ProposedViewSize(size)
                )
                x += size.width + spacing
            }
            y += row.height + spacing
        }
    }

    private func leadingOffset(for row: Row, in width: CGFloat) -> CGFloat {
        switch alignment {
        case .center: return max(0, (width - row.width) / 2)
        case .trailing: return max(0, width - row.width)
        default: return 0
        }
    }

    private struct Row {
        var items: [Int] = []
        var width: CGFloat = 0
        var height: CGFloat = 0
    }

    private func layout(subviews: Subviews, availableWidth: CGFloat) -> [Row] {
        var rows: [Row] = []
        var current = Row()

        for index in subviews.indices {
            let size = subviews[index].sizeThatFits(.unspecified)
            let projected = current.items.isEmpty ? size.width : current.width + spacing + size.width

            if projected > availableWidth, !current.items.isEmpty {
                rows.append(current)
                current = Row()
                current.items = [index]
                current.width = size.width
                current.height = size.height
            } else {
                current.items.append(index)
                current.width = projected
                current.height = max(current.height, size.height)
            }
        }

        if !current.items.isEmpty { rows.append(current) }
        return rows
    }
}
