import Foundation

/// Where the app points. The REST and socket hosts are kept separate because
/// they are usually the same machine in development and are not in production.
struct APIConfiguration: Sendable {
    var baseURL: URL
    var webSocketURL: URL
    var requestTimeout: TimeInterval

    init(baseURL: URL, webSocketURL: URL, requestTimeout: TimeInterval = 20) {
        self.baseURL = baseURL
        self.webSocketURL = webSocketURL
        self.requestTimeout = requestTimeout
    }

    /// Reads `PLUM_API_BASE_URL` / `PLUM_WS_BASE_URL` from the environment so a
    /// device build can be pointed at a laptop without touching code, and falls
    /// back to the simulator-friendly localhost defaults.
    static func fromEnvironment(
        _ environment: [String: String] = ProcessInfo.processInfo.environment
    ) -> APIConfiguration {
        let base = environment["PLUM_API_BASE_URL"]
            .flatMap(URL.init(string:)) ?? URL(string: "http://127.0.0.1:8080/api/v1")!
        let socket = environment["PLUM_WS_BASE_URL"]
            .flatMap(URL.init(string:)) ?? URL(string: "ws://127.0.0.1:8080/ws")!
        return APIConfiguration(baseURL: base, webSocketURL: socket)
    }
}
