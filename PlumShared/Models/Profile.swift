import Foundation

/// The public card of a member: what shows up in someone else's daily selection.
struct Profile: Identifiable, Codable, Hashable, Sendable {
    let id: UUID
    var displayName: String
    var birthDate: Date
    var gender: Gender
    var bio: String
    var city: String
    var photos: [Photo]
    var interests: [String]
    /// Straight-line distance from the viewer, in kilometres. Absent on your
    /// own profile.
    var distanceKm: Double?
    var lastActiveAt: Date?

    init(
        id: UUID,
        displayName: String,
        birthDate: Date,
        gender: Gender,
        bio: String,
        city: String,
        photos: [Photo],
        interests: [String],
        distanceKm: Double? = nil,
        lastActiveAt: Date? = nil
    ) {
        self.id = id
        self.displayName = displayName
        self.birthDate = birthDate
        self.gender = gender
        self.bio = bio
        self.city = city
        self.photos = photos
        self.interests = interests
        self.distanceKm = distanceKm
        self.lastActiveAt = lastActiveAt
    }

    var age: Int {
        let years = Calendar.current.dateComponents([.year], from: birthDate, to: .now).year
        return years ?? 0
    }

    var coverPhoto: Photo? {
        photos.min { $0.position < $1.position }
    }

    var orderedPhotos: [Photo] {
        photos.sorted { $0.position < $1.position }
    }

    /// "Paris · 4 km" — the subtitle under the name on a card.
    var locationLine: String {
        guard let distanceKm else { return city }
        return "\(city) · \(Self.distanceFormatter.string(from: distanceKm))"
    }

    private static let distanceFormatter = DistanceFormatter()
}

/// Rounds distances the way a dating app should: precise when you are close,
/// vague when you are not.
struct DistanceFormatter: Sendable {
    func string(from kilometres: Double) -> String {
        if kilometres < 1 { return "moins d'1 km" }
        if kilometres < 10 { return "\(Int(kilometres.rounded())) km" }
        return "\(Int((kilometres / 5).rounded() * 5)) km"
    }
}
