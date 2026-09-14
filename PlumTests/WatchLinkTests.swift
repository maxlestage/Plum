import XCTest
@testable import Plum

/// Ce que la CI peut réellement vérifier de la montre.
///
/// Elle construit un simulateur iPhone, pas une montre : les vues watchOS ne
/// sont donc que compilées. C'est précisément pour ça que la logique vit dans
/// le code partagé — le passage de jeton et le client HTTP s'exécutent ici,
/// dans les tests iOS, et ce sont eux qui se cassent en silence.
final class WatchLinkTests: XCTestCase {
    private let credentials = WatchCredentials(
        accessToken: "un-jeton-qui-ressemble-a-un-jwt",
        currentUserId: UUID(),
        baseURL: URL(string: "https://plum.test/api/v1")!
    )

    /// Le seul contrat entre les deux appareils : ce qui part doit revenir
    /// identique. `WCSession` ne transporte pas de `Codable`, donc tout passe
    /// par un dictionnaire — l'endroit où un champ se perd sans bruit.
    func testCredentialsSurviveTheCrossing() throws {
        let charge = WatchLink.encode(credentials)
        let recu = try XCTUnwrap(WatchLink.decode(charge))

        XCTAssertEqual(recu, credentials)
    }

    /// Un dictionnaire qui n'est pas le nôtre ne doit pas être décodé à
    /// moitié. WatchConnectivity délivre aussi les messages du système ;
    /// accepter n'importe quoi donnerait une montre connectée à rien.
    func testSomethingElseIsRefusedRatherThanGuessed() {
        for étranger in [
            [:] as [String: Any],
            ["autre.cle": Data()],
            [WatchLink.payloadKey: "pas des données"],
            [WatchLink.payloadKey: Data("{}".utf8)],
        ] {
            XCTAssertNil(WatchLink.decode(étranger), "accepté à tort : \(étranger)")
        }
    }

    /// Le dictionnaire ne doit contenir que des types que `WCSession`
    /// transporte. Un type non conforme fait échouer l'envoi à l'exécution,
    /// jamais à la compilation.
    func testThePayloadIsPropertyListSafe() {
        let charge = WatchLink.encode(credentials)
        XCTAssertTrue(
            PropertyListSerialization.propertyList(charge, isValidFor: .binary),
            "WCSession refuserait cette charge"
        )
    }
}

/// Un serveur en toc, pour éprouver le client de la montre sans réseau.
private final class StubProtocol: URLProtocol {
    nonisolated(unsafe) static var reply: (Int, Data) = (200, Data())
    nonisolated(unsafe) static var lastBody: Data?

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        // `httpBody` est vidé par URLSession ; le corps se relit dans le flux.
        if let flux = request.httpBodyStream {
            flux.open()
            var data = Data()
            var tampon = [UInt8](repeating: 0, count: 1024)
            while flux.hasBytesAvailable {
                let lus = flux.read(&tampon, maxLength: tampon.count)
                if lus <= 0 { break }
                data.append(contentsOf: tampon[0..<lus])
            }
            flux.close()
            Self.lastBody = data
        }

        let (code, corps) = Self.reply
        let response = HTTPURLResponse(
            url: request.url!,
            statusCode: code,
            httpVersion: nil,
            headerFields: ["Content-Type": "application/json"]
        )!
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: corps)
        client?.urlProtocolDidFinishLoading(self)
    }

    override func stopLoading() {}
}

final class WatchChatClientTests: XCTestCase {
    private var client: WatchChatClient!

    override func setUp() {
        super.setUp()
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [StubProtocol.self]
        client = WatchChatClient(
            credentials: WatchCredentials(
                accessToken: "jeton",
                currentUserId: UUID(),
                baseURL: URL(string: "https://plum.test/api/v1")!
            ),
            session: URLSession(configuration: configuration)
        )
    }

    /// L'API rend les messages du plus récent au plus ancien ; un fil se lit
    /// dans l'autre sens. Sans ce tri, la conversation s'affiche à l'envers —
    /// et c'est le genre de chose qu'on ne voit qu'avec trois messages.
    func testAThreadIsReadOldestFirst() async throws {
        let base = Date(timeIntervalSince1970: 1_757_800_000)
        let conversation = UUID()
        let messages = [2, 0, 1].map { décalage in
            Message(
                id: UUID(),
                conversationId: conversation,
                senderId: UUID(),
                body: "message \(décalage)",
                sentAt: base.addingTimeInterval(TimeInterval(décalage) * 60)
            )
        }
        StubProtocol.reply = (200, try JSONEncoder.plum.encode(Page(items: messages)))

        let reçus = try await client.messages(in: conversation)

        XCTAssertEqual(
            reçus.map(\.body),
            ["message 0", "message 1", "message 2"],
            "le fil doit remonter du plus ancien au plus récent"
        )
    }

    /// Un 401 doit se distinguer du reste : c'est le seul échec que la montre
    /// sait traiter — elle oublie son jeton et attend le téléphone. Confondu
    /// avec une panne réseau, il donnerait une montre qui réessaie en boucle
    /// avec un jeton mort.
    func testAnExpiredTokenIsItsOwnFailure() async {
        StubProtocol.reply = (401, Data("{}".utf8))

        do {
            _ = try await client.conversations()
            XCTFail("un 401 doit échouer")
        } catch {
            XCTAssertEqual(error, .expired)
        }
    }

    func testAnyOtherStatusIsATransportFailure() async {
        StubProtocol.reply = (503, Data("{}".utf8))

        do {
            _ = try await client.conversations()
            XCTFail("un 503 doit échouer")
        } catch {
            XCTAssertEqual(error, .transport("HTTP 503"))
        }
    }

    /// Chaque envoi porte un `client_id` neuf. C'est lui qui rend un renvoi
    /// inoffensif côté serveur ; sur une montre, où la connexion se perd en
    /// baissant le poignet, il compte plus qu'ailleurs.
    func testEverySendCarriesAFreshClientId() async throws {
        let conversation = UUID()
        let réponse = Message(
            id: UUID(),
            conversationId: conversation,
            senderId: UUID(),
            body: "Salut !",
            sentAt: .now
        )
        StubProtocol.reply = (200, try JSONEncoder.plum.encode(réponse))

        var vus = Set<String>()
        for _ in 0..<3 {
            _ = try await client.send("Salut !", to: conversation)
            let corps = try XCTUnwrap(StubProtocol.lastBody)
            let json = try XCTUnwrap(
                JSONSerialization.jsonObject(with: corps) as? [String: Any]
            )
            vus.insert(try XCTUnwrap(json["client_id"] as? String))
        }

        XCTAssertEqual(vus.count, 3, "trois envois, trois identifiants")
    }
}
