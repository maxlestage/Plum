import CoreLocation
import Foundation
import OSLog

struct Coordinate: Codable, Hashable, Sendable {
    var latitude: Double
    var longitude: Double

    init(latitude: Double, longitude: Double) {
        self.latitude = latitude
        self.longitude = longitude
    }
}

enum LocationAuthorization: Sendable, Equatable {
    case notDetermined
    case denied
    case authorized

    var canLocate: Bool { self == .authorized }
}

/// Where the app gets a position from. A protocol because CoreLocation is
/// untestable and because the demo mode has to work in a simulator with no
/// location set.
protocol LocationProviding: Sendable {
    func authorization() async -> LocationAuthorization
    /// Asks the person. Returns the resulting status, whatever they chose.
    func requestAuthorization() async -> LocationAuthorization
    /// A single fix. Throws rather than hanging if permission is missing.
    func currentCoordinate() async throws -> Coordinate
}

enum LocationError: Error, Equatable, Sendable {
    case notAuthorized
    case unavailable(String)
}

/// The value the app holds. It owns nothing: `CLLocationManager` lives on the
/// main actor, and building one from `AppEnvironment.live()` — which is not
/// isolated — would not compile. Every call hops instead.
struct SystemLocationProvider: LocationProviding {
    func authorization() async -> LocationAuthorization {
        await CoreLocationProvider.shared.authorization()
    }

    func requestAuthorization() async -> LocationAuthorization {
        await CoreLocationProvider.shared.requestAuthorization()
    }

    func currentCoordinate() async throws -> Coordinate {
        try await CoreLocationProvider.shared.currentCoordinate()
    }
}

/// The real thing. `@MainActor` because `CLLocationManager` wants a run loop,
/// and its delegate callbacks are turned into continuations here so callers
/// see a plain `await`.
@MainActor
final class CoreLocationProvider: NSObject, CLLocationManagerDelegate {
    static let shared = CoreLocationProvider()

    private let manager = CLLocationManager()
    private let logger = Logger(subsystem: "app.plum", category: "location")

    private var authorizationContinuation: CheckedContinuation<LocationAuthorization, Never>?
    private var locationContinuation: CheckedContinuation<Coordinate, Error>?

    private override init() {
        super.init()
        manager.delegate = self
        // Street-level precision would be both unnecessary and unkind: the UI
        // never shows anything finer than a kilometre.
        manager.desiredAccuracy = kCLLocationAccuracyKilometer
    }

    func authorization() async -> LocationAuthorization {
        Self.map(manager.authorizationStatus)
    }

    func requestAuthorization() async -> LocationAuthorization {
        let current = Self.map(manager.authorizationStatus)
        guard current == .notDetermined else { return current }

        return await withCheckedContinuation { continuation in
            authorizationContinuation = continuation
            manager.requestWhenInUseAuthorization()
        }
    }

    func currentCoordinate() async throws -> Coordinate {
        guard Self.map(manager.authorizationStatus).canLocate else {
            throw LocationError.notAuthorized
        }
        // Only one fix can be in flight; a second caller would strand the
        // first continuation.
        if locationContinuation != nil {
            throw LocationError.unavailable("Une localisation est déjà en cours.")
        }

        return try await withCheckedThrowingContinuation { continuation in
            locationContinuation = continuation
            manager.requestLocation()
            startLocationTimeout()
        }
    }

    /// `requestLocation()` is documented to report a failure on its own, but a
    /// continuation that is never resumed hangs the caller forever — and here
    /// that caller is a screen. This makes the worst case a slow refusal.
    private func startLocationTimeout() {
        Task { [weak self] in
            try? await Task.sleep(for: .seconds(15))
            self?.failLocation("Délai dépassé.")
        }
    }

    // MARK: - CLLocationManagerDelegate

    nonisolated func locationManagerDidChangeAuthorization(_ manager: CLLocationManager) {
        let status = manager.authorizationStatus
        Task { @MainActor in
            self.resumeAuthorization(with: Self.map(status))
        }
    }

    nonisolated func locationManager(
        _ manager: CLLocationManager,
        didUpdateLocations locations: [CLLocation]
    ) {
        let coordinate = locations.last.map {
            Coordinate(latitude: $0.coordinate.latitude, longitude: $0.coordinate.longitude)
        }
        Task { @MainActor in
            self.resumeLocation(with: coordinate)
        }
    }

    nonisolated func locationManager(_ manager: CLLocationManager, didFailWithError error: Error) {
        let description = error.localizedDescription
        Task { @MainActor in
            self.failLocation(description)
        }
    }

    // MARK: - Continuation plumbing

    private func resumeAuthorization(with status: LocationAuthorization) {
        guard status != .notDetermined, let continuation = authorizationContinuation else { return }
        authorizationContinuation = nil
        continuation.resume(returning: status)
    }

    private func resumeLocation(with coordinate: Coordinate?) {
        guard let continuation = locationContinuation else { return }
        locationContinuation = nil
        if let coordinate {
            continuation.resume(returning: coordinate)
        } else {
            continuation.resume(throwing: LocationError.unavailable("Position introuvable."))
        }
    }

    private func failLocation(_ description: String) {
        guard let continuation = locationContinuation else { return }
        locationContinuation = nil
        logger.notice("Localisation impossible : \(description, privacy: .public)")
        continuation.resume(throwing: LocationError.unavailable(description))
    }

    static func map(_ status: CLAuthorizationStatus) -> LocationAuthorization {
        switch status {
        case .notDetermined:
            return .notDetermined
        case .authorizedAlways, .authorizedWhenInUse:
            return .authorized
        default:
            // Denied and restricted are the same thing to us: no position.
            return .denied
        }
    }
}

/// A fixed position in Paris, for previews, the demo mode and the UI tests —
/// a simulator with no location set would otherwise never answer.
actor DemoLocationProvider: LocationProviding {
    private var status: LocationAuthorization
    private let coordinate: Coordinate

    init(
        status: LocationAuthorization = .authorized,
        coordinate: Coordinate = Coordinate(latitude: 48.8566, longitude: 2.3522)
    ) {
        self.status = status
        self.coordinate = coordinate
    }

    func authorization() async -> LocationAuthorization { status }

    func requestAuthorization() async -> LocationAuthorization {
        if status == .notDetermined { status = .authorized }
        return status
    }

    func currentCoordinate() async throws -> Coordinate {
        guard status.canLocate else { throw LocationError.notAuthorized }
        return coordinate
    }
}
