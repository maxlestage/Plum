import CoreLocation
import XCTest
@testable import Plum

@MainActor
final class LocationSyncTests: XCTestCase {
    private func makeSync(
        status: LocationAuthorization
    ) -> (LocationSync, DemoProfileService) {
        let profiles = DemoProfileService()
        let sync = LocationSync(
            provider: DemoLocationProvider(status: status),
            profiles: profiles
        )
        return (sync, profiles)
    }

    /// The whole point: the server computes every distance from the position
    /// we push, and before this existed nothing ever pushed one.
    func testAnAuthorisedSyncPushesTheCoordinate() async throws {
        let (sync, profiles) = makeSync(status: .authorized)

        let pushed = await sync.sync()

        XCTAssertTrue(pushed)
        let stored = try XCTUnwrap(await profiles.lastPushedLocation)
        XCTAssertEqual(stored.latitude, 48.8566, accuracy: 0.0001)
        XCTAssertEqual(stored.longitude, 2.3522, accuracy: 0.0001)
        XCTAssertNil(sync.lastError)
    }

    /// A refusal degrades the deck; it must not break it or nag.
    func testARefusalPushesNothingAndReportsNoError() async {
        let (sync, profiles) = makeSync(status: .denied)

        let pushed = await sync.sync()

        XCTAssertFalse(pushed)
        let stored = await profiles.lastPushedLocation
        XCTAssertNil(stored)
        XCTAssertTrue(sync.isBlocked)
        XCTAssertNil(sync.lastError, "Un refus n'est pas une erreur à afficher")
    }

    func testAskingThenSyncingIsOneAction() async {
        let (sync, profiles) = makeSync(status: .notDetermined)
        await sync.refreshAuthorization()
        XCTAssertTrue(sync.needsPermission)

        let granted = await sync.requestPermissionAndSync()

        XCTAssertTrue(granted)
        XCTAssertFalse(sync.needsPermission)
        let stored = await profiles.lastPushedLocation
        XCTAssertNotNil(stored, "Accepter doit envoyer la position dans la foulée")
    }

    func testAuthorizationMappingTreatsRestrictedAsRefused() {
        XCTAssertEqual(CoreLocationProvider.map(.notDetermined), .notDetermined)
        XCTAssertEqual(CoreLocationProvider.map(.authorizedWhenInUse), .authorized)
        XCTAssertEqual(CoreLocationProvider.map(.authorizedAlways), .authorized)
        XCTAssertEqual(CoreLocationProvider.map(.denied), .denied)
        XCTAssertEqual(CoreLocationProvider.map(.restricted), .denied)
    }

    func testCoordinateEncodesInSnakeCase() throws {
        let data = try JSONEncoder.plum.encode(Coordinate(latitude: 48.85, longitude: 2.35))
        let object = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
        XCTAssertEqual(object["latitude"] as? Double, 48.85)
        XCTAssertEqual(object["longitude"] as? Double, 2.35)
    }
}
