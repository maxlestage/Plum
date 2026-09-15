import XCTest
@testable import Plum

/// The API is the contract; these lock the wire format the app expects from it.
final class ModelDecodingTests: XCTestCase {
    func testDecodesProfileFromSnakeCasePayload() throws {
        let json = """
        {
          "id": "8A1B0A2C-1111-4444-8888-0123456789AB",
          "display_name": "Inès",
          "birth_date": "1998-04-12T00:00:00Z",
          "gender": "woman",
          "bio": "Babyfoot ou rien.",
          "city": "Paris",
          "photos": [
            {
              "id": "8A1B0A2C-2222-4444-8888-0123456789AB",
              "url": "https://cdn.plum.app/p/1.jpg",
              "position": 0
            }
          ],
          "interests": ["Techno", "Ramen"],
          "distance_km": 3.4,
          "last_active_at": "2026-02-01T10:30:00.123Z"
        }
        """

        let profile = try JSONDecoder.plum.decode(Profile.self, from: Data(json.utf8))

        XCTAssertEqual(profile.displayName, "Inès")
        XCTAssertEqual(profile.gender, .woman)
        XCTAssertEqual(profile.photos.count, 1)
        XCTAssertEqual(profile.distanceKm, 3.4)
        XCTAssertNotNil(profile.lastActiveAt)
    }

    /// Postgres hands back timestamps with and without fractional seconds
    /// depending on the column; both have to parse.
    func testDecodesTimestampsWithAndWithoutFractionalSeconds() throws {
        XCTAssertNotNil(PlumDateFormat.date(from: "2026-02-01T10:30:00Z"))
        XCTAssertNotNil(PlumDateFormat.date(from: "2026-02-01T10:30:00.123Z"))
        XCTAssertNil(PlumDateFormat.date(from: "pas une date"))
    }

    /// La sélection arrive en snake_case ; les champs qui ne se décodent pas
    /// donnent un écran vide, pas une erreur.
    func testDecodesTheDailySelection() throws {
        let json = """
        {
          "items": [],
          "refreshes_at": "2026-02-02T00:00:00Z",
          "size": 3
        }
        """
        let selection = try JSONDecoder.plum.decode(DailySelection.self, from: Data(json.utf8))

        XCTAssertTrue(selection.items.isEmpty)
        XCTAssertEqual(selection.size, 3)
        XCTAssertEqual(selection.refreshesAt, PlumDateFormat.date(from: "2026-02-02T00:00:00Z"))
    }

    func testDecodesChatEventsByType() throws {
        let json = """
        {
          "type": "typing",
          "conversation_id": "8A1B0A2C-3333-4444-8888-0123456789AB",
          "profile_id": "8A1B0A2C-4444-4444-8888-0123456789AB"
        }
        """

        let event = try JSONDecoder.plum.decode(ChatEvent.self, from: Data(json.utf8))

        guard case let .typing(conversationId, _) = event else {
            return XCTFail("Attendu un évènement de saisie, reçu \(event)")
        }
        XCTAssertEqual(conversationId.uuidString, "8A1B0A2C-3333-4444-8888-0123456789AB")
    }

    func testUnknownEventTypeIsRejectedRatherThanIgnored() {
        let json = #"{"type": "quelque_chose_de_neuf"}"#
        XCTAssertThrowsError(
            try JSONDecoder.plum.decode(ChatEvent.self, from: Data(json.utf8))
        )
    }

    func testProfileAgeAndLocationLine() {
        let profile = Profile(
            id: UUID(),
            displayName: "Théo",
            birthDate: SampleData.birthDate(age: 31),
            gender: .man,
            bio: "",
            city: "Montreuil",
            photos: [],
            interests: [],
            distanceKm: 7.8
        )

        XCTAssertEqual(profile.age, 31)
        XCTAssertEqual(profile.locationLine, "Montreuil · 8 km")
    }

    /// Distances are deliberately vague past 10 km, and never precise enough to
    /// locate someone.
    func testDistanceFormattingRoundsAwayPrecision() {
        let formatter = DistanceFormatter()
        XCTAssertEqual(formatter.string(from: 0.4), "moins d'1 km")
        XCTAssertEqual(formatter.string(from: 4.2), "4 km")
        XCTAssertEqual(formatter.string(from: 23), "25 km")
    }
}
