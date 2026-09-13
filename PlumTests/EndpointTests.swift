import XCTest
@testable import Plum

final class EndpointTests: XCTestCase {
    private let configuration = APIConfiguration(
        baseURL: URL(string: "https://api.plum.app/api/v1")!,
        webSocketURL: URL(string: "wss://api.plum.app/ws")!
    )

    func testBuildsURLWithQueryAndBearerToken() throws {
        let endpoint = Endpoint.get(
            "discovery/deck",
            query: [URLQueryItem(name: "limit", value: "20")]
        )

        let request = try endpoint.urlRequest(configuration: configuration, accessToken: "abc123")

        XCTAssertEqual(request.url?.absoluteString, "https://api.plum.app/api/v1/discovery/deck?limit=20")
        XCTAssertEqual(request.httpMethod, "GET")
        XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer abc123")
    }

    func testLeadingSlashDoesNotDoubleUpInPath() throws {
        let request = try Endpoint.get("/matches").urlRequest(configuration: configuration)
        XCTAssertEqual(request.url?.absoluteString, "https://api.plum.app/api/v1/matches")
    }

    func testPublicEndpointsCarryNoAuthorizationHeader() throws {
        let endpoint = Endpoint.post(
            "auth/sign-in",
            body: SignInRequest(email: "moi@plum.app", password: "motdepasse"),
            requiresAuthentication: false
        )

        let request = try endpoint.urlRequest(configuration: configuration, accessToken: "abc123")

        XCTAssertNil(request.value(forHTTPHeaderField: "Authorization"))
        XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")
        XCTAssertNotNil(request.httpBody)
    }

    func testAccessTokenExpiryUsesLeewaySoRequestsNeverRaceIt() {
        let tokens = AuthTokens(
            accessToken: "a",
            refreshToken: "r",
            expiresAt: Date.now.addingTimeInterval(10)
        )
        // Ten seconds left, but inside the 30s leeway: treat it as expired.
        XCTAssertTrue(tokens.isExpired())
        XCTAssertFalse(tokens.isExpired(leeway: 0))
    }

    func testMultipartBodyWrapsPayloadInBoundaries() throws {
        let body = APIClient.multipartBody(
            data: Data("photo".utf8),
            filename: "selfie.jpg",
            boundary: "XYZ"
        )
        let text = try XCTUnwrap(String(data: body, encoding: .utf8))

        XCTAssertTrue(text.hasPrefix("--XYZ\r\n"))
        XCTAssertTrue(text.contains(#"filename="selfie.jpg""#))
        XCTAssertTrue(text.hasSuffix("\r\n--XYZ--\r\n"))
    }
}
