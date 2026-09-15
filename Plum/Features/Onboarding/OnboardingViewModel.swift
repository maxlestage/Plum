import Observation
import SwiftUI

/// Hoisted out of the view model so it carries no actor isolation: it is a
/// plain value used for navigation and for conformances a `@MainActor` type
/// could not satisfy.
enum OnboardingStep: Int, CaseIterable, Identifiable, Sendable {
    case photos
    case about
    case preferences

    var id: Int { rawValue }

    var title: String {
        switch self {
        case .photos: return "Montrez-vous"
        case .about: return "Dites-en un peu"
        case .preferences: return "Qui voulez-vous voir ?"
        }
    }

    var subtitle: String {
        switch self {
        case .photos: return "Une photo au minimum. Deux, c'est mieux."
        case .about: return "Deux phrases suffisent. Vraiment."
        case .preferences: return "Ça se change à tout moment dans les réglages."
        }
    }
}

/// Walks a brand-new account through the minimum it needs before it can
/// appear in anyone's selection.
///
/// The gate exists because the alternative is worse for everyone: a profile
/// with no photo and no bio wastes the swipe of every person it reaches.
@MainActor
@Observable
final class OnboardingViewModel {
    var step: OnboardingStep = .photos
    var bio = ""
    var city = ""
    var interests: [String] = []
    var preferences = DiscoveryPreferences.default

    private(set) var photos: [Photo] = []
    private(set) var isWorking = false
    private(set) var errorMessage: String?

    static let minimumPhotos = 1
    static let maximumPhotos = 6

    private let profiles: any ProfileServicing
    private let session: SessionStore

    init(profiles: any ProfileServicing, session: SessionStore) {
        self.profiles = profiles
        self.session = session
    }

    var progress: Double {
        Double(step.rawValue + 1) / Double(OnboardingStep.allCases.count)
    }

    var isLastStep: Bool {
        step == OnboardingStep.allCases.last
    }

    var canAdvance: Bool {
        guard !isWorking else { return false }
        switch step {
        case .photos:
            return photos.count >= Self.minimumPhotos
        case .about:
            return !city.trimmingCharacters(in: .whitespaces).isEmpty
        case .preferences:
            return true
        }
    }

    /// Why the button is dim, said out loud rather than left to guess.
    var blockedReason: String? {
        guard !canAdvance, !isWorking else { return nil }
        switch step {
        case .photos: return "Ajoutez au moins une photo pour continuer."
        case .about: return "Indiquez votre ville."
        case .preferences: return nil
        }
    }

    var canAddMorePhotos: Bool {
        photos.count < Self.maximumPhotos
    }

    // MARK: - Steps

    /// Advances, or finishes. Returns true once onboarding is done.
    func advance() async -> Bool {
        guard canAdvance else { return false }
        errorMessage = nil

        guard isLastStep else {
            let next = OnboardingStep(rawValue: step.rawValue + 1) ?? step
            withAnimation(.easeInOut(duration: 0.25)) { step = next }
            return false
        }
        return await finish()
    }

    func goBack() {
        guard let previous = OnboardingStep(rawValue: step.rawValue - 1) else { return }
        withAnimation(.easeInOut(duration: 0.25)) { step = previous }
    }

    // MARK: - Actions

    func addPhoto(_ jpegData: Data) async {
        guard canAddMorePhotos else { return }
        isWorking = true
        defer { isWorking = false }
        do {
            photos.append(try await profiles.uploadPhoto(jpegData))
            Haptics.play(.light)
        } catch {
            errorMessage = error.asAPIError.userMessage
        }
    }

    func removePhoto(_ photo: Photo) async {
        photos.removeAll { $0.id == photo.id }
        try? await profiles.deletePhoto(id: photo.id)
    }

    func toggleInterest(_ interest: String) {
        if let index = interests.firstIndex(of: interest) {
            interests.remove(at: index)
        } else if interests.count < ProfileViewModel.interestLimit {
            interests.append(interest)
        }
    }

    private func finish() async -> Bool {
        isWorking = true
        defer { isWorking = false }

        do {
            let profile = try await profiles.update(
                ProfileUpdate(
                    displayName: nil,
                    bio: bio.trimmingCharacters(in: .whitespacesAndNewlines),
                    city: city.trimmingCharacters(in: .whitespaces),
                    interests: interests
                )
            )
            _ = try await profiles.updatePreferences(preferences)
            let user = try await profiles.completeProfile()

            session.currentProfile = profile
            session.adopt(user)
            Haptics.play(.success)
            return true
        } catch {
            errorMessage = error.asAPIError.userMessage
            Haptics.play(.warning)
            return false
        }
    }
}
