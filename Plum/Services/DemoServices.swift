import Foundation

/// In-memory stand-ins for the whole service layer.
///
/// They are what previews use, and what the app falls back to when it is
/// launched with `PLUM_DEMO_MODE=1` — so the UI can be run and reviewed in the
/// simulator without the Rust API on the other end.
enum DemoMode {
    static var isEnabled: Bool {
        ProcessInfo.processInfo.environment["PLUM_DEMO_MODE"] == "1"
    }

    /// Enough latency to show spinners, not enough to be annoying.
    static let latency: Duration = .milliseconds(350)

    static func pause() async {
        try? await Task.sleep(for: latency)
    }
}

actor DemoAuthService: AuthServicing {
    private var isSignedIn = false

    func signIn(email: String, password: String) async throws -> AuthenticatedSession {
        await DemoMode.pause()
        guard password.count >= 8 else {
            throw APIError.server(status: 401, message: "Mot de passe incorrect.")
        }
        isSignedIn = true
        return session(email: email)
    }

    func signUp(_ request: SignUpRequest) async throws -> AuthenticatedSession {
        await DemoMode.pause()
        isSignedIn = true
        return session(email: request.email)
    }

    func currentUser() async throws -> User {
        guard isSignedIn else { throw APIError.unauthorized }
        return SampleData.currentUser
    }

    func signOut() async {
        isSignedIn = false
    }

    func deleteAccount() async throws {
        isSignedIn = false
    }

    func restoreSession() async -> User? {
        isSignedIn ? SampleData.currentUser : nil
    }

    private func session(email: String) -> AuthenticatedSession {
        var user = SampleData.currentUser
        user.email = email
        return AuthenticatedSession(
            user: user,
            tokens: AuthTokens(
                accessToken: "demo-access",
                refreshToken: "demo-refresh",
                expiresAt: .now.addingTimeInterval(3_600)
            )
        )
    }
}

actor DemoProfileService: ProfileServicing {
    private var profile = SampleData.myProfile
    private var prefs = DiscoveryPreferences.default

    func myProfile() async throws -> Profile {
        await DemoMode.pause()
        return profile
    }

    func update(_ update: ProfileUpdate) async throws -> Profile {
        await DemoMode.pause()
        if let name = update.displayName { profile.displayName = name }
        if let bio = update.bio { profile.bio = bio }
        if let city = update.city { profile.city = city }
        if let interests = update.interests { profile.interests = interests }
        return profile
    }

    func uploadPhoto(_ jpegData: Data) async throws -> Photo {
        await DemoMode.pause()
        let photo = Photo(
            id: UUID(),
            url: URL(string: "https://picsum.photos/seed/\(UUID().uuidString)/900/1200")!,
            position: profile.photos.count
        )
        profile.photos.append(photo)
        return photo
    }

    func deletePhoto(id: UUID) async throws {
        profile.photos.removeAll { $0.id == id }
    }

    func reorderPhotos(_ orderedIds: [UUID]) async throws -> [Photo] {
        var reordered: [Photo] = []
        for (index, id) in orderedIds.enumerated() {
            guard var photo = profile.photos.first(where: { $0.id == id }) else { continue }
            photo.position = index
            reordered.append(photo)
        }
        profile.photos = reordered
        return reordered
    }

    func preferences() async throws -> DiscoveryPreferences {
        prefs
    }

    func updatePreferences(_ preferences: DiscoveryPreferences) async throws -> DiscoveryPreferences {
        prefs = preferences.sanitized
        return prefs
    }
}

actor DemoDiscoveryService: DiscoveryServicing {
    private var remaining = SampleData.deck
    private var lastPassed: Profile?

    func deck(cursor: String?, limit: Int) async throws -> Page<Profile> {
        await DemoMode.pause()
        // The demo deck refills so swiping never dead-ends during a review.
        if remaining.isEmpty {
            remaining = SampleData.deck
        }
        return Page(items: remaining, nextCursor: nil)
    }

    func swipe(profileId: UUID, decision: SwipeDecision) async throws -> SwipeOutcome {
        if decision == .pass {
            lastPassed = remaining.first { $0.id == profileId }
        }
        remaining.removeAll { $0.id == profileId }

        // Every other like is a match, which is roughly the fantasy the demo
        // is meant to sell.
        let matched = decision != .pass && remaining.count % 2 == 0
        guard matched, let profile = SampleData.deck.first(where: { $0.id == profileId }) else {
            return SwipeOutcome(matched: false, match: nil, likesRemaining: nil)
        }
        let match = Match(id: UUID(), profile: profile, matchedAt: .now)
        return SwipeOutcome(matched: true, match: match, likesRemaining: nil)
    }

    func rewind() async throws -> Profile? {
        guard let profile = lastPassed else { return nil }
        lastPassed = nil
        remaining.insert(profile, at: 0)
        return profile
    }

    func report(profileId: UUID, reason: String) async throws {
        remaining.removeAll { $0.id == profileId }
    }

    func block(profileId: UUID) async throws {
        remaining.removeAll { $0.id == profileId }
    }
}

actor DemoMatchService: MatchServicing {
    private var stored = SampleData.matches

    func matches(cursor: String?) async throws -> Page<Match> {
        await DemoMode.pause()
        return Page(items: stored, nextCursor: nil)
    }

    func unmatch(matchId: UUID) async throws {
        stored.removeAll { $0.id == matchId }
    }

    func conversation(forMatch matchId: UUID) async throws -> Conversation {
        guard let conversation = SampleData.conversations.first(where: { $0.matchId == matchId }) else {
            throw APIError.notFound
        }
        return conversation
    }
}

actor DemoChatService: ChatServicing {
    private var threads: [UUID: [Message]] = [:]
    /// Broadcast, like the real socket: the tab badge and the open
    /// conversation both listen.
    private var subscribers: [UUID: AsyncStream<ChatEvent>.Continuation] = [:]

    func conversations(cursor: String?) async throws -> Page<Conversation> {
        await DemoMode.pause()
        return Page(items: SampleData.conversations, nextCursor: nil)
    }

    func messages(conversationId: UUID, before: String?) async throws -> Page<Message> {
        await DemoMode.pause()
        if threads[conversationId] == nil,
           let conversation = SampleData.conversations.first(where: { $0.id == conversationId }) {
            threads[conversationId] = SampleData.thread(for: conversation)
        }
        return Page(items: threads[conversationId] ?? [], nextCursor: nil)
    }

    func send(conversationId: UUID, clientId: UUID, body: String) async throws -> Message {
        await DemoMode.pause()
        let message = Message(
            id: clientId,
            conversationId: conversationId,
            senderId: SampleData.currentUser.id,
            body: body,
            sentAt: .now
        )
        threads[conversationId, default: []].append(message)
        scheduleReply(to: conversationId)
        return message
    }

    func markRead(conversationId: UUID) async throws {}

    func eventStream() async throws -> AsyncStream<ChatEvent> {
        let id = UUID()
        let (stream, continuation) = AsyncStream<ChatEvent>.makeStream(of: ChatEvent.self)
        continuation.onTermination = { [weak self] _ in
            Task { await self?.removeSubscriber(id) }
        }
        subscribers[id] = continuation
        continuation.yield(.connectionChanged(isConnected: true))
        return stream
    }

    func closeStream() async {
        for continuation in subscribers.values {
            continuation.finish()
        }
        subscribers.removeAll()
    }

    private func removeSubscriber(_ id: UUID) {
        subscribers[id] = nil
    }

    /// Fakes someone on the other end typing, so the live path is exercised.
    private func scheduleReply(to conversationId: UUID) {
        guard let conversation = SampleData.conversations.first(where: { $0.id == conversationId }) else {
            return
        }
        Task { [weak self] in
            try? await Task.sleep(for: .seconds(2))
            await self?.deliverReply(to: conversation)
        }
    }

    private func deliverReply(to conversation: Conversation) {
        let reply = Message(
            id: UUID(),
            conversationId: conversation.id,
            senderId: conversation.participant.id,
            body: "Hmm. Continue.",
            sentAt: .now
        )
        threads[conversation.id, default: []].append(reply)
        for continuation in subscribers.values {
            continuation.yield(.messageReceived(reply))
        }
    }
}
