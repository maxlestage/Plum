import SwiftUI

/// Matches and conversations in one screen, because they are the same thing at
/// two stages of the same story.
struct MatchesView: View {
    @Environment(\.services) private var services
    @State private var viewModel: MatchesViewModel?
    @State private var openedConversation: Conversation?

    var body: some View {
        NavigationStack {
            PlumBackground {
                content
            }
            .navigationTitle("Vos conversations")
            .navigationDestination(item: $openedConversation) { conversation in
                ChatView(conversation: conversation) {
                    // Reported, blocked or unmatched: the row has to go.
                    viewModel?.drop(conversationId: conversation.id)
                }
                .onDisappear { viewModel?.markRead(conversation.id) }
            }
        }
        .task {
            if viewModel == nil {
                viewModel = MatchesViewModel(
                    matchService: services.matches,
                    chatService: services.chat
                )
            }
            await viewModel?.load()
            await watchForIncomingMessages()
        }
    }

    /// A message arriving while this tab is open has to move its row: the
    /// preview, the unread count and the order all come from it. The socket
    /// is multicast, so listening here costs nothing that the badge is not
    /// already paying.
    private func watchForIncomingMessages() async {
        guard let stream = try? await services.chat.eventStream() else { return }
        for await event in stream {
            guard case let .messageReceived(message) = event else { continue }
            viewModel?.apply(message)
        }
    }

    @ViewBuilder
    private var content: some View {
        if let viewModel {
            VStack(spacing: 0) {
                // Every other screen shows its failures; this one swallowed
                // them, so a match that would not open just did nothing.
                if let error = viewModel.state.error {
                    ErrorBanner(message: error.userMessage) {
                        Task { await viewModel.load() }
                    }
                    .padding(.vertical, PlumTheme.Spacing.s)
                }
                list(viewModel)
            }
        } else {
            ProgressView().tint(PlumTheme.Palette.plum)
        }
    }

    @ViewBuilder
    private func list(_ viewModel: MatchesViewModel) -> some View {
        if viewModel.isEmpty {
            EmptyStateView(
                systemImage: "heart.slash",
                title: "Rien encore",
                message: "Une conversation s'ouvre quand quelqu'un écrit — vous, ou l'autre. Vos trois profils du jour sont dans l'onglet Aujourd'hui."
            )
        } else {
            List {
                Section {
                    ForEach(viewModel.conversations) { conversation in
                        Button {
                            openedConversation = conversation
                        } label: {
                            ConversationRow(conversation: conversation)
                        }
                        .buttonStyle(.plain)
                        .accessibilityIdentifier("ligne-conversation")
                        .listRowBackground(PlumTheme.Palette.surface)
                        .swipeActions(edge: .trailing) {
                            Button("Retirer", role: .destructive) {
                                Task { await unmatch(conversation, viewModel: viewModel) }
                            }
                        }
                        .onAppear {
                            // Reaching the last row is the signal to page.
                            guard conversation.id == viewModel.conversations.last?.id else { return }
                            Task { await viewModel.loadNextPage() }
                        }
                    }

                    if viewModel.hasMoreToLoad {
                        HStack {
                            Spacer()
                            ProgressView().tint(PlumTheme.Palette.plum)
                            Spacer()
                        }
                        .listRowBackground(Color.clear)
                    }
                }
            }
            .listStyle(.insetGrouped)
            .scrollContentBackground(.hidden)
            .refreshable { await viewModel.load() }
        }
    }

    private func unmatch(_ conversation: Conversation, viewModel: MatchesViewModel) async {
        guard let match = viewModel.matches.first(where: { $0.id == conversation.matchId }) else {
            return
        }
        await viewModel.unmatch(match)
    }
}

/// One row of the inbox.
struct ConversationRow: View {
    let conversation: Conversation

    var body: some View {
        HStack(spacing: PlumTheme.Spacing.m) {
            AvatarView(profile: conversation.participant, diameter: 56, showsActivityDot: true)

            VStack(alignment: .leading, spacing: 3) {
                Text(conversation.participant.displayName)
                    .font(.plumHeadline)
                    .foregroundStyle(PlumTheme.Palette.primaryText)
                Text(conversation.preview)
                    .font(.plumCallout)
                    .foregroundStyle(PlumTheme.Palette.secondaryText)
                    .lineLimit(1)
            }

            Spacer(minLength: PlumTheme.Spacing.s)

            VStack(alignment: .trailing, spacing: PlumTheme.Spacing.xs) {
                Text(RelativeDateFormatting.inboxTimestamp(for: conversation.updatedAt))
                    .font(.plumCaption)
                    .foregroundStyle(PlumTheme.Palette.secondaryText)
                if conversation.unreadCount > 0 {
                    Text("\(conversation.unreadCount)")
                        .font(.plumCaption)
                        .foregroundStyle(.white)
                        .padding(.horizontal, 7)
                        .padding(.vertical, 3)
                        .background(PlumTheme.Palette.blush, in: Capsule())
                }
            }
        }
        .padding(.vertical, PlumTheme.Spacing.xs)
        .contentShape(Rectangle())
    }
}

#Preview {
    MatchesView()
        .environment(\.services, .preview)
        .environment(SessionStore(auth: AppEnvironment.preview.auth))
}
