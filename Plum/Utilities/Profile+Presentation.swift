import Foundation

extension Profile {
    /// "En ligne" is a promise; this is the honest version of it.
    var isRecentlyActive: Bool {
        guard let lastActiveAt else { return false }
        return Date.now.timeIntervalSince(lastActiveAt) < 15 * 60
    }

    var activityLine: String? {
        guard let lastActiveAt else { return nil }
        if isRecentlyActive { return "En ligne" }
        return "Vu·e " + RelativeDateFormatting.phrase(for: lastActiveAt)
    }

    var nameAndAge: String {
        "\(displayName), \(age)"
    }
}
