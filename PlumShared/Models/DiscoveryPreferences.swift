import Foundation

/// The filters applied when the backend builds a deck.
struct DiscoveryPreferences: Codable, Hashable, Sendable {
    var interestedIn: GenderPreference
    var minAge: Int
    var maxAge: Int
    var maxDistanceKm: Int
    var showMeOnPlum: Bool

    static let `default` = DiscoveryPreferences(
        interestedIn: .everyone,
        minAge: 18,
        maxAge: 45,
        maxDistanceKm: 50,
        showMeOnPlum: true
    )

    /// The app never lets the range invert or dip under the legal minimum.
    var sanitized: DiscoveryPreferences {
        var copy = self
        copy.minAge = max(18, min(minAge, 99))
        copy.maxAge = max(copy.minAge, min(maxAge, 99))
        copy.maxDistanceKm = max(1, min(maxDistanceKm, 300))
        return copy
    }
}
