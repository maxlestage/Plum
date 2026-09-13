import SwiftUI

/// Named text styles. All of them build on Dynamic Type sizes, so the app
/// scales with the system setting instead of fighting it.
extension Font {
    static let plumDisplay = Font.system(.largeTitle, design: .serif).weight(.bold)
    static let plumTitle = Font.system(.title2, design: .rounded).weight(.bold)
    static let plumHeadline = Font.system(.headline, design: .rounded)
    static let plumBody = Font.system(.body)
    static let plumCallout = Font.system(.callout)
    static let plumCaption = Font.system(.caption).weight(.medium)
    static let plumButton = Font.system(.headline, design: .rounded).weight(.semibold)
}

extension Text {
    func plumSectionHeader() -> some View {
        self
            .font(.plumCaption)
            .textCase(.uppercase)
            .kerning(0.8)
            .foregroundStyle(PlumTheme.Palette.secondaryText)
    }
}
