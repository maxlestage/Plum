import SwiftUI
#if canImport(UIKit)
import UIKit
#endif

/// Thin wrapper so feature code never touches UIKit directly and previews on
/// any platform stay silent.
@MainActor
enum Haptics {
    enum Kind {
        case light
        case success
        case warning
    }

    static func play(_ kind: Kind) {
        #if canImport(UIKit)
        switch kind {
        case .light:
            UIImpactFeedbackGenerator(style: .light).impactOccurred()
        case .success:
            UINotificationFeedbackGenerator().notificationOccurred(.success)
        case .warning:
            UINotificationFeedbackGenerator().notificationOccurred(.warning)
        }
        #endif
    }
}
