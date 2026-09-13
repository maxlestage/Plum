import Foundation

enum APIError: Error, Equatable, Sendable {
    case invalidURL(String)
    case invalidResponse
    case unauthorized
    case notFound
    case rateLimited(retryAfter: TimeInterval?)
    /// A 4xx the server explained, e.g. "cette adresse est déjà prise".
    case server(status: Int, message: String)
    case decoding(String)
    case transport(String)
    case offline

    /// What we are willing to put in front of a user.
    var userMessage: String {
        switch self {
        case .invalidURL, .invalidResponse, .decoding:
            return "Quelque chose s'est mal passé de notre côté. Réessayez."
        case .unauthorized:
            return "Votre session a expiré. Reconnectez-vous."
        case .notFound:
            return "Introuvable."
        case .rateLimited:
            return "Doucement. Réessayez dans un instant."
        case let .server(_, message):
            return message
        case .transport:
            return "Connexion impossible. Vérifiez votre réseau."
        case .offline:
            return "Vous êtes hors ligne."
        }
    }

    /// Retrying a 500 or a dropped connection is reasonable; retrying a 400 is
    /// just noise.
    var isRetryable: Bool {
        switch self {
        case .transport, .offline, .rateLimited:
            return true
        case let .server(status, _):
            return status >= 500
        default:
            return false
        }
    }
}

/// The error envelope an axum handler returns on failure.
struct APIErrorBody: Decodable, Sendable {
    let message: String
    let code: String?
}
