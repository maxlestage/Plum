import Foundation

/// Le strict nécessaire pour qu'une montre lise et réponde.
///
/// Trois appels, pas un de plus. Le client de l'application sait rafraîchir
/// un jeton, réessayer, gérer un socket ; rien de tout cela n'a sa place sur
/// une montre, où l'on ouvre l'écran dix secondes. Un jeton périmé s'y traite
/// d'une seule façon utile : demander au téléphone d'en renvoyer un.
///
/// Ce fichier vit dans le code partagé pour être testable depuis les tests
/// iOS. C'est la seule façon de le vérifier ici : la CI construit un
/// simulateur iPhone, pas une montre.
struct WatchChatClient: Sendable {
    enum Failure: Error, Equatable, Sendable {
        /// Le jeton n'est plus valable : la montre doit en redemander un.
        case expired
        case transport(String)
        case decoding(String)
    }

    private let credentials: WatchCredentials
    private let session: URLSession

    init(credentials: WatchCredentials, session: URLSession = .shared) {
        self.credentials = credentials
        self.session = session
    }

    var currentUserId: UUID { credentials.currentUserId }

    func conversations() async throws(Failure) -> [Conversation] {
        let page: Page<Conversation> = try await get("conversations")
        return page.items
    }

    func messages(in conversation: UUID) async throws(Failure) -> [Message] {
        let page: Page<Message> = try await get("conversations/\(conversation)/messages")
        // L'API rend du plus récent au plus ancien ; une conversation se lit
        // dans l'autre sens.
        return page.items.sorted { $0.sentAt < $1.sentAt }
    }

    func send(_ body: String, to conversation: UUID) async throws(Failure) -> Message {
        var request = URLRequest(url: url("conversations/\(conversation)/messages"))
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        // Le même `client_id` que l'application : c'est lui qui rend un
        // renvoi inoffensif. Sur une montre, où la connexion se perd en
        // levant le poignet, il compte plus qu'ailleurs.
        request.httpBody = try? JSONEncoder.plum.encode(
            Outgoing(clientId: UUID(), body: body)
        )
        return try await perform(request)
    }

    // MARK: - Le socle

    private struct Outgoing: Encodable {
        let clientId: UUID
        let body: String
    }

    private func url(_ path: String) -> URL {
        credentials.baseURL.appendingPathComponent(path)
    }

    private func get<T: Decodable>(_ path: String) async throws(Failure) -> T {
        var request = URLRequest(url: url(path))
        request.httpMethod = "GET"
        return try await perform(request)
    }

    private func perform<T: Decodable>(_ request: URLRequest) async throws(Failure) -> T {
        var request = request
        request.setValue(
            "Bearer \(credentials.accessToken)",
            forHTTPHeaderField: "Authorization"
        )

        let data: Data
        let response: URLResponse
        do {
            (data, response) = try await session.data(for: request)
        } catch {
            throw Failure.transport(error.localizedDescription)
        }

        if let http = response as? HTTPURLResponse, http.statusCode == 401 {
            throw Failure.expired
        }
        if let http = response as? HTTPURLResponse, !(200..<300).contains(http.statusCode) {
            throw Failure.transport("HTTP \(http.statusCode)")
        }

        do {
            return try JSONDecoder.plum.decode(T.self, from: data)
        } catch {
            throw Failure.decoding(error.localizedDescription)
        }
    }
}

/// Les réponses toutes faites de la montre.
///
/// Dicter marche, mais pas dans un métro ni devant des collègues. Trois
/// réponses couvrent ce qu'on envoie vraiment depuis un poignet : accuser
/// réception, gagner du temps, relancer. Au-delà, c'est une liste qu'on
/// parcourt au lieu de sortir son téléphone — et sortir son téléphone
/// devient alors plus rapide, ce qui rend la montre inutile.
enum QuickReply {
    static let all = ["Salut !", "Je te réponds vite", "Ça marche 👍"]
}
