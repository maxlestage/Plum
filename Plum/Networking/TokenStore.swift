import Foundation
import Security

/// Somewhere to keep the token pair between launches.
protocol TokenStoring: Sendable {
    func read() -> AuthTokens?
    func write(_ tokens: AuthTokens)
    func clear()
}

/// Keychain-backed storage. Tokens are the one thing in this app that must not
/// land in `UserDefaults`.
struct KeychainTokenStore: TokenStoring {
    private let service: String
    private let account: String

    init(service: String = "app.plum.tokens", account: String = "session") {
        self.service = service
        self.account = account
    }

    private var baseQuery: [String: Any] {
        [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account
        ]
    }

    func read() -> AuthTokens? {
        var query = baseQuery
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne

        var item: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &item)
        guard status == errSecSuccess, let data = item as? Data else { return nil }
        return try? JSONDecoder.plum.decode(AuthTokens.self, from: data)
    }

    func write(_ tokens: AuthTokens) {
        guard let data = try? JSONEncoder.plum.encode(tokens) else { return }

        let attributes: [String: Any] = [
            kSecValueData as String: data,
            kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlock
        ]
        let status = SecItemUpdate(baseQuery as CFDictionary, attributes as CFDictionary)
        if status == errSecItemNotFound {
            var insert = baseQuery
            insert.merge(attributes) { current, _ in current }
            _ = SecItemAdd(insert as CFDictionary, nil)
        }
    }

    func clear() {
        _ = SecItemDelete(baseQuery as CFDictionary)
    }
}

/// In-memory storage for tests and previews, where touching the real keychain
/// would be both slow and rude.
final class InMemoryTokenStore: TokenStoring, @unchecked Sendable {
    private let lock = NSLock()
    private var tokens: AuthTokens?

    init(tokens: AuthTokens? = nil) {
        self.tokens = tokens
    }

    func read() -> AuthTokens? {
        lock.lock()
        defer { lock.unlock() }
        return tokens
    }

    func write(_ tokens: AuthTokens) {
        lock.lock()
        defer { lock.unlock() }
        self.tokens = tokens
    }

    func clear() {
        lock.lock()
        defer { lock.unlock() }
        tokens = nil
    }
}
