import Observation
import SwiftUI

/// Obtains a position and pushes it to the API.
///
/// The app displays distances and filters by them, but until this existed
/// nothing ever told the server where the person was — so the numbers came
/// from nowhere. One object owns that round trip, and every screen that needs
/// it asks here.
@MainActor
@Observable
final class LocationSync {
    private(set) var authorization: LocationAuthorization = .notDetermined
    private(set) var lastError: String?

    private var isSyncing = false
    private let provider: any LocationProviding
    private let profiles: any ProfileServicing

    init(provider: any LocationProviding, profiles: any ProfileServicing) {
        self.provider = provider
        self.profiles = profiles
    }

    var needsPermission: Bool {
        authorization == .notDetermined
    }

    var isBlocked: Bool {
        authorization == .denied
    }

    func refreshAuthorization() async {
        authorization = await provider.authorization()
    }

    /// Asks, then pushes straight away if granted — the permission prompt and
    /// the first useful result should feel like one action.
    @discardableResult
    func requestPermissionAndSync() async -> Bool {
        authorization = await provider.requestAuthorization()
        guard authorization.canLocate else { return false }
        return await sync()
    }

    /// Pushes a fresh position if we are allowed to. Silent on failure: a
    /// missing position degrades the selection, it does not break it.
    @discardableResult
    func sync() async -> Bool {
        guard !isSyncing else { return false }
        authorization = await provider.authorization()
        guard authorization.canLocate else { return false }

        isSyncing = true
        defer { isSyncing = false }

        do {
            let coordinate = try await provider.currentCoordinate()
            try await profiles.updateLocation(coordinate)
            lastError = nil
            return true
        } catch {
            lastError = error.asAPIError.userMessage
            return false
        }
    }
}
