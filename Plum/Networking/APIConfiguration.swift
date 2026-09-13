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

    /// Where a shipped build points. Change this line, and nothing else, if
    /// the deployment moves.
    ///
    /// A compiled constant rather than an environment variable, because the
    /// scheme's variables exist only when Xcode launches the app: a TestFlight
    /// build has none, and would otherwise aim at localhost — which is to say
    /// at itself, which is to say nowhere.
    static let productionHost = "plum-a5f3c7189761.herokuapp.com"

    /// Reads `PLUM_API_BASE_URL` / `PLUM_WS_BASE_URL` from the environment so a
    /// device build can be pointed at a laptop without touching code, and falls
    /// back to localhost in Debug and to the deployment everywhere else.
    static func fromEnvironment(
        _ environment: [String: String] = ProcessInfo.processInfo.environment
    ) -> APIConfiguration {
        let base = environment["PLUM_API_BASE_URL"]
            .flatMap(URL.init(string:)) ?? URL(string: defaultBaseURL)!
        let socket = environment["PLUM_WS_BASE_URL"]
            .flatMap(URL.init(string:)) ?? URL(string: defaultWebSocketURL)!
        return APIConfiguration(baseURL: base, webSocketURL: socket)
    }

    static var defaultBaseURL: String {
        #if DEBUG
        "http://127.0.0.1:8080/api/v1"
        #else
        "https://\(productionHost)/api/v1"
        #endif
    }

    static var defaultWebSocketURL: String {
        #if DEBUG
        "ws://127.0.0.1:8080/ws"
        #else
        "wss://\(productionHost)/ws"
        #endif
    }
}
