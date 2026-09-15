import SwiftUI

/// The visual vocabulary of the app. Every colour, radius and spacing value in
/// the UI comes from here, so a restyle is one file.
enum PlumTheme {
    enum Palette {
        /// Deep plum — the brand anchor.
        static let plum = Color(hex: 0x6B2D5C)
        /// Le bas du dégradé de l'icône d'application, repris par
        /// `Scripts/generate_appicon.py` — d'où l'absence d'usage en Swift.
        static let plumDeep = Color(hex: 0x40183A)
        static let blush = Color(hex: 0xE8608C)
        static let apricot = Color(hex: 0xF6A56B)
        static let mint = Color(hex: 0x4FC3A1)
        static let ink = Color(hex: 0x1F1320)

        static let canvas = Color("Canvas", bundle: .main)
        static let surface = Color("Surface", bundle: .main)
        static let primaryText = Color("PrimaryText", bundle: .main)
        static let secondaryText = Color("SecondaryText", bundle: .main)

        /// The like / pass verdict colours, used on the card overlays and the
        /// action bar alike so the gesture and the buttons mean the same thing.
        static let like = mint
        static let pass = Color(hex: 0xE2574C)
        static let superLike = Color(hex: 0x4A9BE8)

        static let warmGradient = LinearGradient(
            colors: [blush, plum],
            startPoint: .topLeading,
            endPoint: .bottomTrailing
        )

        static let cardScrim = LinearGradient(
            colors: [.clear, .black.opacity(0.15), .black.opacity(0.75)],
            startPoint: .center,
            endPoint: .bottom
        )
    }

    enum Spacing {
        static let xs: CGFloat = 4
        static let s: CGFloat = 8
        static let m: CGFloat = 16
        static let l: CGFloat = 24
        static let xl: CGFloat = 32
    }

    enum Radius {
        static let small: CGFloat = 10
        static let medium: CGFloat = 18
        static let card: CGFloat = 28
    }
}

extension Color {
    /// `Color(hex: 0x6B2D5C)` reads better in a palette than four decimals.
    init(hex: UInt32, opacity: Double = 1) {
        self.init(
            .sRGB,
            red: Double((hex >> 16) & 0xFF) / 255,
            green: Double((hex >> 8) & 0xFF) / 255,
            blue: Double(hex & 0xFF) / 255,
            opacity: opacity
        )
    }
}
