import Observation
import SwiftUI

/// Your own profile: the one screen where every field is editable and every
/// edit has to survive a round trip.
@MainActor
@Observable
final class ProfileViewModel {
    private(set) var profile: Profile?
    private(set) var preferences = DiscoveryPreferences.default
    private(set) var state: ActivityState = .idle
    private(set) var isSaving = false

    /// Edited copies, so a cancelled edit changes nothing.
    var draftName = ""
    var draftBio = ""
    var draftCity = ""
    var draftInterests: [String] = []

    static let bioLimit = 300
    static let interestLimit = 6

    /// The list offered when adding an interest. Free text is allowed too, but
    /// suggestions keep the data tidy enough to match on.
    static let suggestedInterests = [
        "Cinéma", "Concerts", "Randonnée", "Cuisine", "Vinyles", "Escalade",
        "Jeux vidéo", "Expos", "Café", "Vélo", "Voyages", "Karaoké",
        "Techno", "Lecture", "Yoga", "Brunch", "Skate", "Photographie"
    ]

    private let profiles: any ProfileServicing
    private let session: SessionStore

    init(profiles: any ProfileServicing, session: SessionStore) {
        self.profiles = profiles
        self.session = session
    }

    var remainingBioCharacters: Int {
        Self.bioLimit - draftBio.count
    }

    var canSave: Bool {
        !isSaving && !draftName.trimmingCharacters(in: .whitespaces).isEmpty
            && draftBio.count <= Self.bioLimit
    }

    func load() async {
        if profile == nil { state = .loading }
        do {
            async let profileRequest = profiles.myProfile()
            async let preferencesRequest = profiles.preferences()
            let (loaded, prefs) = try await (profileRequest, preferencesRequest)
            apply(loaded)
            preferences = prefs
            state = .ready
        } catch {
            state = .failed(error.asAPIError)
        }
    }

    private func apply(_ profile: Profile) {
        self.profile = profile
        session.currentProfile = profile
        draftName = profile.displayName
        draftBio = profile.bio
        draftCity = profile.city
        draftInterests = profile.interests
    }

    func resetDraft() {
        guard let profile else { return }
        draftName = profile.displayName
        draftBio = profile.bio
        draftCity = profile.city
        draftInterests = profile.interests
    }

    func save() async -> Bool {
        guard canSave else { return false }
        isSaving = true
        defer { isSaving = false }

        do {
            let updated = try await profiles.update(
                ProfileUpdate(
                    displayName: draftName.trimmingCharacters(in: .whitespaces),
                    bio: draftBio.trimmingCharacters(in: .whitespacesAndNewlines),
                    city: draftCity.trimmingCharacters(in: .whitespaces),
                    interests: draftInterests
                )
            )
            apply(updated)
            Haptics.play(.success)
            return true
        } catch {
            state = .failed(error.asAPIError)
            Haptics.play(.warning)
            return false
        }
    }

    func toggleInterest(_ interest: String) {
        if let index = draftInterests.firstIndex(of: interest) {
            draftInterests.remove(at: index)
        } else if draftInterests.count < Self.interestLimit {
            draftInterests.append(interest)
        }
    }

    func addPhoto(_ jpegData: Data) async {
        do {
            let photo = try await profiles.uploadPhoto(jpegData)
            profile?.photos.append(photo)
            if let profile { session.currentProfile = profile }
        } catch {
            state = .failed(error.asAPIError)
        }
    }

    /// Promotes a photo to the cover slot. It is the one picture that decides
    /// whether anyone reads the rest of the profile, so it needs to be one
    /// gesture away.
    func makeCover(_ photo: Photo) async {
        guard let profile, profile.coverPhoto?.id != photo.id else { return }

        let reordered = [photo] + profile.orderedPhotos.filter { $0.id != photo.id }
        // Optimistic: the grid reorders under the finger, not after a round
        // trip.
        self.profile?.photos = reordered.enumerated().map { index, photo in
            var copy = photo
            copy.position = index
            return copy
        }
        Haptics.play(.light)

        do {
            let confirmed = try await profiles.reorderPhotos(reordered.map(\.id))
            self.profile?.photos = confirmed
            if let updated = self.profile { session.currentProfile = updated }
        } catch {
            state = .failed(error.asAPIError)
        }
    }

    func deletePhoto(_ photo: Photo) async {
        profile?.photos.removeAll { $0.id == photo.id }
        try? await profiles.deletePhoto(id: photo.id)
    }

    func updatePreferences(_ new: DiscoveryPreferences) async {
        let sanitized = new.sanitized
        preferences = sanitized
        do {
            preferences = try await profiles.updatePreferences(sanitized)
        } catch {
            state = .failed(error.asAPIError)
        }
    }
}
