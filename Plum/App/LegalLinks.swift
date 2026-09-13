import Foundation

/// The documents an app that handles personal data has to make reachable —
/// and that the welcome screen already claimed people were accepting.
///
/// Overridable from the environment so a staging build can point at drafts
/// rather than the live pages.
enum LegalLinks {
    static var terms: URL { url(for: "PLUM_TERMS_URL", fallback: "https://plum.app/conditions") }
    static var privacy: URL { url(for: "PLUM_PRIVACY_URL", fallback: "https://plum.app/confidentialite") }
    static var support: URL { url(for: "PLUM_SUPPORT_URL", fallback: "https://plum.app/aide") }

    private static func url(for key: String, fallback: String) -> URL {
        ProcessInfo.processInfo.environment[key]
            .flatMap(URL.init(string:)) ?? URL(string: fallback)!
    }
}
