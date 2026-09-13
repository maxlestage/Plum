import Foundation

/// A single API call, described rather than executed, so it can be built in a
/// service and tested without a network.
struct Endpoint: Sendable {
    var path: String
    var method: HTTPMethod
    var query: [URLQueryItem]
    var body: AnyEncodable?
    var requiresAuthentication: Bool

    init(
        path: String,
        method: HTTPMethod = .get,
        query: [URLQueryItem] = [],
        body: AnyEncodable? = nil,
        requiresAuthentication: Bool = true
    ) {
        self.path = path
        self.method = method
        self.query = query
        self.body = body
        self.requiresAuthentication = requiresAuthentication
    }

    static func get(
        _ path: String,
        query: [URLQueryItem] = [],
        requiresAuthentication: Bool = true
    ) -> Endpoint {
        Endpoint(
            path: path,
            method: .get,
            query: query,
            requiresAuthentication: requiresAuthentication
        )
    }

    static func post(_ path: String, requiresAuthentication: Bool = true) -> Endpoint {
        Endpoint(path: path, method: .post, requiresAuthentication: requiresAuthentication)
    }

    static func post<Body: Encodable>(
        _ path: String,
        body: Body,
        requiresAuthentication: Bool = true
    ) -> Endpoint {
        Endpoint(
            path: path,
            method: .post,
            body: AnyEncodable(body),
            requiresAuthentication: requiresAuthentication
        )
    }

    static func patch<Body: Encodable>(
        _ path: String,
        body: Body,
        requiresAuthentication: Bool = true
    ) -> Endpoint {
        Endpoint(
            path: path,
            method: .patch,
            body: AnyEncodable(body),
            requiresAuthentication: requiresAuthentication
        )
    }

    static func delete(_ path: String, requiresAuthentication: Bool = true) -> Endpoint {
        Endpoint(path: path, method: .delete, requiresAuthentication: requiresAuthentication)
    }

    /// Builds the request. Kept separate from sending so tests can assert on
    /// the URL and headers without a server.
    func urlRequest(
        configuration: APIConfiguration,
        accessToken: String? = nil,
        encoder: JSONEncoder = .plum
    ) throws -> URLRequest {
        let trimmed = path.hasPrefix("/") ? String(path.dropFirst()) : path
        let resolved = configuration.baseURL.appendingPathComponent(trimmed)

        guard var components = URLComponents(url: resolved, resolvingAgainstBaseURL: false) else {
            throw APIError.invalidURL(path)
        }
        if !query.isEmpty {
            components.queryItems = query
        }
        guard let url = components.url else {
            throw APIError.invalidURL(path)
        }

        var request = URLRequest(url: url, timeoutInterval: configuration.requestTimeout)
        request.httpMethod = method.rawValue
        request.setValue("application/json", forHTTPHeaderField: "Accept")

        if let body {
            request.httpBody = try encoder.encode(body)
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        }
        if requiresAuthentication, let accessToken {
            request.setValue("Bearer \(accessToken)", forHTTPHeaderField: "Authorization")
        }
        return request
    }
}
