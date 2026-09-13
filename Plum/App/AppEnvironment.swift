import SwiftUI

/// The composition root: one value carrying every service a feature can reach
/// for. Views take it from the SwiftUI environment, so swapping the whole app
/// onto demo services is a one-line change in ``PlumApp``.
struct AppEnvironment: Sendable {
    var auth: any AuthServicing
    var profiles: any ProfileServicing
    var discovery: any DiscoveryServicing
    var matches: any MatchServicing
    var chat: any ChatServicing
    var location: any LocationProviding

    /// Wires the real API client, its socket and the keychain together.
    static func live(configuration: APIConfiguration = .fromEnvironment()) -> AppEnvironment {
        let client = APIClient(configuration: configuration)
        let socket = ChatSocket(configuration: configuration)
        return AppEnvironment(
            auth: AuthService(client: client),
            profiles: ProfileService(client: client),
            discovery: DiscoveryService(client: client),
            matches: MatchService(client: client),
            chat: ChatService(client: client, socket: socket),
            location: SystemLocationProvider()
        )
    }

    /// Everything in memory: used by previews, UI tests and `PLUM_DEMO_MODE=1`.
    static func demo() -> AppEnvironment {
        AppEnvironment(
            auth: DemoAuthService(),
            profiles: DemoProfileService(),
            discovery: DemoDiscoveryService(),
            matches: DemoMatchService(),
            chat: DemoChatService(),
            location: DemoLocationProvider()
        )
    }

    /// A demo environment that starts already signed in, which is what a
    /// preview of an inner screen wants.
    static let preview = AppEnvironment.demo()
}

private struct AppEnvironmentKey: EnvironmentKey {
    static let defaultValue = AppEnvironment.preview
}

extension EnvironmentValues {
    var services: AppEnvironment {
        get { self[AppEnvironmentKey.self] }
        set { self[AppEnvironmentKey.self] = newValue }
    }
}
