import Foundation

struct ProfileUpdate: Codable, Sendable {
    var displayName: String?
    var bio: String?
    var city: String?
    var interests: [String]?
}

protocol ProfileServicing: Sendable {
    func myProfile() async throws -> Profile
    func update(_ update: ProfileUpdate) async throws -> Profile
    func uploadPhoto(_ jpegData: Data) async throws -> Photo
    func deletePhoto(id: UUID) async throws
    func reorderPhotos(_ orderedIds: [UUID]) async throws -> [Photo]
    func preferences() async throws -> DiscoveryPreferences
    func updatePreferences(_ preferences: DiscoveryPreferences) async throws -> DiscoveryPreferences
    /// Marks onboarding as done. Returns the updated account so the session
    /// stops routing to the onboarding flow.
    func completeProfile() async throws -> User
    /// Pushes the current position. The server computes every distance from
    /// it; without this call the deck has nothing to sort by.
    func updateLocation(_ coordinate: Coordinate) async throws
}

struct ProfileService: ProfileServicing {
    let client: any APIClientProtocol

    init(client: any APIClientProtocol) {
        self.client = client
    }

    func myProfile() async throws -> Profile {
        try await client.send(.get("me/profile"), as: Profile.self)
    }

    func update(_ update: ProfileUpdate) async throws -> Profile {
        try await client.send(.patch("me/profile", body: update), as: Profile.self)
    }

    func uploadPhoto(_ jpegData: Data) async throws -> Photo {
        try await client.upload(jpegData, filename: "photo.jpg", to: "me/photos")
    }

    func deletePhoto(id: UUID) async throws {
        try await client.send(.delete("me/photos/\(id.uuidString)"))
    }

    func reorderPhotos(_ orderedIds: [UUID]) async throws -> [Photo] {
        struct Payload: Codable, Sendable { let photoIds: [UUID] }
        let endpoint = Endpoint.patch("me/photos/order", body: Payload(photoIds: orderedIds))
        return try await client.send(endpoint, as: [Photo].self)
    }

    func preferences() async throws -> DiscoveryPreferences {
        try await client.send(.get("me/preferences"), as: DiscoveryPreferences.self)
    }

    func updatePreferences(_ preferences: DiscoveryPreferences) async throws -> DiscoveryPreferences {
        let endpoint = Endpoint.patch("me/preferences", body: preferences.sanitized)
        return try await client.send(endpoint, as: DiscoveryPreferences.self)
    }

    func completeProfile() async throws -> User {
        try await client.send(.post("me/profile/complete"), as: User.self)
    }

    func updateLocation(_ coordinate: Coordinate) async throws {
        try await client.send(.patch("me/location", body: coordinate))
    }
}
