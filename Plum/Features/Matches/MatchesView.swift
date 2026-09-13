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
            .navigationTitle("Vos matchs")
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
        }
    }

    @ViewBuilder
    private var content: some View {
        if let viewModel {
            if viewModel.isEmpty {
                EmptyStateView(
                    systemImage: "heart.slash",
                    title: "Rien encore",
                    message: "Les matchs arrivent quand deux personnes se disent oui. Retournez swiper."
                )
            } else {
                List {
                    if !viewModel.unstartedMatches.isEmpty {
                        Section {
                            newMatchesRail(viewModel)
                                .listRowInsets(EdgeInsets())
                                .listRowBackground(Color.clear)
                        } header: {
                            Text("Nouveaux matchs").plumSectionHeader()
                        }
                    }

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
                    } header: {
                        if !viewModel.conversations.isEmpty {
                            Text("Messages").plumSectionHeader()
                        }
                    }
                }
                .listStyle(.insetGrouped)
                .scrollContentBackground(.hidden)
                .refreshable { await viewModel.load() }
            }
        } else {
            ProgressView().tint(PlumTheme.Palette.plum)
        }
    }

    private func newMatchesRail(_ viewModel: MatchesViewModel) -> some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: PlumTheme.Spacing.m) {
                ForEach(viewModel.unstartedMatches) { match in
                    Button {
                        Task { @MainActor in
                            openedConversation = await viewModel.conversation(for: match)
                        }
                    } label: {
                        VStack(spacing: PlumTheme.Spacing.xs) {
                            AvatarView(profile: match.profile, diameter: 72, showsActivityDot: true)
                                .overlay {
                                    if match.isFresh {
                                        Circle()
                                            .stroke(PlumTheme.Palette.warmGradient, lineWidth: 3)
                                    }
                                }
                            Text(match.profile.displayName)
                                .font(.plumCaption)
                                .lineLimit(1)
                                .foregroundStyle(PlumTheme.Palette.primaryText)
                        }
                        .frame(width: 78)
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.horizontal, PlumTheme.Spacing.m)
            .padding(.vertical, PlumTheme.Spacing.s)
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
