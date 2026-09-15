import Observation
import SwiftUI

/// The matches tab: fresh matches on top as a rail, conversations below.
@MainActor
@Observable
final class MatchesViewModel {
    private(set) var matches: [Match] = []
    private(set) var conversations: [Conversation] = []
    private(set) var state: ActivityState = .idle

    private var matchesCursor: String?
    private var conversationsCursor: String?
    private var isPaginating = false
    private let matchService: any MatchServicing
    private let chatService: any ChatServicing

    init(matchService: any MatchServicing, chatService: any ChatServicing) {
        self.matchService = matchService
        self.chatService = chatService
    }

    // `unstartedMatches` vivait ici : les matchs à qui personne n'avait encore
    // écrit, montrés dans une rangée en haut de la boîte pour débloquer
    // exactement ce cas. Il ne peut plus s'en produire — un fil naît *avec*
    // son premier message, dans la même transaction — donc la rangée était
    // vide en toutes circonstances. Une interface qui ne peut rien afficher
    // est pire qu'absente : on la croit en panne.

    var isEmpty: Bool {
        matches.isEmpty && conversations.isEmpty && !state.isLoading
    }

    func load() async {
        if matches.isEmpty { state = .loading }
        do {
            // Two independent reads: run them together rather than in series.
            async let matchPage = matchService.matches(cursor: nil)
            async let conversationPage = chatService.conversations(cursor: nil)

            let (loadedMatches, loadedConversations) = try await (matchPage, conversationPage)
            matches = loadedMatches.items.sorted { $0.matchedAt > $1.matchedAt }
            matchesCursor = loadedMatches.nextCursor
            conversations = loadedConversations.items.sorted { $0.updatedAt > $1.updatedAt }
            conversationsCursor = loadedConversations.nextCursor
            state = .ready
        } catch {
            state = .failed(error.asAPIError)
        }
    }

    var hasMoreToLoad: Bool {
        matchesCursor != nil || conversationsCursor != nil
    }

    /// Called when the last row scrolls into view. Both lists page
    /// independently, so whichever still has a cursor is extended.
    func loadNextPage() async {
        guard !isPaginating, hasMoreToLoad else { return }
        isPaginating = true
        defer { isPaginating = false }

        do {
            if let cursor = matchesCursor {
                let page = try await matchService.matches(cursor: cursor)
                let known = Set(matches.map(\.id))
                matches += page.items.filter { !known.contains($0.id) }
                matchesCursor = page.nextCursor
            }
            if let cursor = conversationsCursor {
                let page = try await chatService.conversations(cursor: cursor)
                let known = Set(conversations.map(\.id))
                conversations += page.items.filter { !known.contains($0.id) }
                conversations.sort { $0.updatedAt > $1.updatedAt }
                conversationsCursor = page.nextCursor
            }
        } catch {
            state = .failed(error.asAPIError)
        }
    }

    func conversation(for match: Match) async -> Conversation? {
        if let id = match.conversationId,
           let existing = conversations.first(where: { $0.id == id }) {
            return existing
        }
        do {
            let conversation = try await matchService.conversation(forMatch: match.id)
            if !conversations.contains(where: { $0.id == conversation.id }) {
                conversations.insert(conversation, at: 0)
            }
            return conversation
        } catch {
            state = .failed(error.asAPIError)
            return nil
        }
    }

    func unmatch(_ match: Match) async {
        matches.removeAll { $0.id == match.id }
        conversations.removeAll { $0.matchId == match.id }
        try? await matchService.unmatch(matchId: match.id)
    }

    /// Applied when the socket pushes a message while this tab is on screen.
    func apply(_ message: Message) {
        guard let index = conversations.firstIndex(where: { $0.id == message.conversationId }) else {
            return
        }
        conversations[index].lastMessage = message
        conversations[index].updatedAt = message.sentAt
        if message.senderId != conversations[index].participant.id {
            conversations[index].unreadCount = 0
        } else {
            conversations[index].unreadCount += 1
        }
        conversations.sort { $0.updatedAt > $1.updatedAt }
    }

    /// Removes a conversation and its match after a report, a block or an
    /// unmatch, without waiting for a reload to notice.
    func drop(conversationId: UUID) {
        guard let conversation = conversations.first(where: { $0.id == conversationId }) else {
            return
        }
        conversations.removeAll { $0.id == conversationId }
        matches.removeAll { $0.id == conversation.matchId }
    }

    func markRead(_ conversationId: UUID) {
        guard let index = conversations.firstIndex(where: { $0.id == conversationId }) else { return }
        conversations[index].unreadCount = 0
    }
}
