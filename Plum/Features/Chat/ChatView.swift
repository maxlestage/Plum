import SwiftUI

/// The conversation screen: history above, composer pinned below, live updates
/// arriving from the socket in between.
struct ChatView: View {
    let conversation: Conversation
    /// Called when the conversation ends for good — a report, a block, an
    /// unmatch — so the list that pushed us can drop the row instead of
    /// showing a thread that no longer exists.
    var onLeave: (() -> Void)?

    @Environment(\.services) private var services
    @Environment(SessionStore.self) private var session
    @State private var viewModel: ChatViewModel?
    @State private var isReporting = false
    @State private var isConfirmingUnmatch = false
    @FocusState private var isComposerFocused: Bool
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        PlumBackground {
            VStack(spacing: 0) {
                if let viewModel {
                    if !viewModel.isConnected {
                        connectionNotice
                    }
                    thread(viewModel)
                    if viewModel.isClosed {
                        closedNotice
                    } else {
                        composer(viewModel)
                    }
                } else {
                    Spacer()
                    ProgressView().tint(PlumTheme.Palette.plum)
                    Spacer()
                }
            }
        }
        .navigationTitle(conversation.participant.displayName)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Menu {
                    Button {
                        isReporting = true
                    } label: {
                        Label("Signaler", systemImage: "flag")
                    }
                    Button(role: .destructive) {
                        isConfirmingUnmatch = true
                    } label: {
                        Label("Retirer le match", systemImage: "person.badge.minus")
                    }
                } label: {
                    Image(systemName: "ellipsis.circle")
                }
                .accessibilityLabel(Text("Options de la conversation"))
            }
            ToolbarItem(placement: .principal) {
                HStack(spacing: PlumTheme.Spacing.s) {
                    AvatarView(profile: conversation.participant, diameter: 30)
                    VStack(alignment: .leading, spacing: 0) {
                        Text(conversation.participant.displayName)
                            .font(.plumHeadline)
                        if let activity = conversation.participant.activityLine {
                            Text(activity)
                                .font(.plumCaption)
                                .foregroundStyle(PlumTheme.Palette.secondaryText)
                        }
                    }
                }
            }
        }
        .task {
            if viewModel == nil {
                viewModel = ChatViewModel(
                    conversation: conversation,
                    chat: services.chat,
                    discovery: services.discovery,
                    currentUserId: session.currentUserId ?? SampleData.currentUser.id
                )
            }
            await viewModel?.start()
        }
        .onChange(of: viewModel?.draft ?? "") { _, _ in
            Task { await viewModel?.draftChanged() }
        }
        .onDisappear { viewModel?.stop() }
        .sheet(isPresented: $isReporting) {
            // Harassment happens in the conversation, not on the card: the
            // same escape hatch has to be reachable from here.
            // On ne quitte la conversation que si le serveur a bien enregistré
            // le geste. Sortir quand même reviendrait à masquer un fil qui
            // reste ouvert : le message suivant arriverait sans prévenir.
            ReportSheet(profile: conversation.participant) { reason in
                Task { @MainActor in
                    if await viewModel?.report(reason: reason) == true {
                        leaveConversation()
                    }
                }
            } onBlock: {
                Task { @MainActor in
                    if await viewModel?.block() == true {
                        leaveConversation()
                    }
                }
            }
        }
        .alert(
            "Geste non enregistré",
            isPresented: Binding(
                get: { viewModel?.safetyFailure != nil },
                set: { if !$0 { viewModel?.dismissSafetyFailure() } }
            )
        ) {
            Button("D'accord", role: .cancel) { viewModel?.dismissSafetyFailure() }
        } message: {
            Text(viewModel?.safetyFailure ?? "")
        }
        .confirmationDialog(
            "Retirer ce match ?",
            isPresented: $isConfirmingUnmatch,
            titleVisibility: .visible
        ) {
            Button("Retirer", role: .destructive) {
                Task { @MainActor in
                    try? await services.matches.unmatch(matchId: conversation.matchId)
                    leaveConversation()
                }
            }
        } message: {
            Text("La conversation disparaît des deux côtés.")
        }
    }

    /// Closes the socket for this thread before backing out, so a conversation
    /// we just left does not keep pushing events.
    @MainActor
    private func leaveConversation() {
        viewModel?.stop()
        onLeave?()
        dismiss()
    }

    /// Le fil n'existe plus côté serveur. Sans raison affichée : blocage,
    /// match défait ou compte supprimé donnent la même réponse, et dire lequel
    /// dénoncerait le geste de l'autre.
    private var closedNotice: some View {
        Text("Cette conversation n'est plus disponible.")
            .font(.plumCaption)
            .foregroundStyle(PlumTheme.Palette.secondaryText)
            .multilineTextAlignment(.center)
            .frame(maxWidth: .infinity)
            .padding(.horizontal, PlumTheme.Spacing.m)
            .padding(.vertical, PlumTheme.Spacing.m)
            .background(.bar)
    }

    private var connectionNotice: some View {
        Text("Reconnexion…")
            .font(.plumCaption)
            .foregroundStyle(.white)
            .frame(maxWidth: .infinity)
            .padding(.vertical, 6)
            .background(PlumTheme.Palette.apricot)
    }

    private func thread(_ viewModel: ChatViewModel) -> some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(spacing: PlumTheme.Spacing.s) {
                    if viewModel.state.isLoading {
                        ProgressView().padding()
                    }

                    Color.clear
                        .frame(height: 1)
                        .onAppear {
                            Task { @MainActor in
                                // Put the reader back where they were: the new
                                // page went in above them.
                                guard let anchor = await viewModel.loadOlderMessages() else { return }
                                proxy.scrollTo(anchor, anchor: .top)
                            }
                        }

                    ForEach(Array(viewModel.items.enumerated()), id: \.element.id) { index, item in
                        MessageBubble(
                            item: item,
                            isMine: viewModel.isMine(item),
                            showsTimestamp: shouldShowTimestamp(at: index, in: viewModel.items),
                            onRetry: { Task { await viewModel.retry(item) } }
                        )
                        .id(item.id)
                    }

                    if viewModel.isParticipantTyping {
                        TypingIndicator()
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .id(Self.typingAnchor)
                    }

                    if let error = viewModel.state.error {
                        ErrorBanner(message: error.userMessage)
                    }
                }
                .padding(.horizontal, PlumTheme.Spacing.m)
                .padding(.vertical, PlumTheme.Spacing.m)
            }
            .scrollDismissesKeyboard(.interactively)
            .defaultScrollAnchor(.bottom)
            .onChange(of: viewModel.items.count) { _, _ in
                scrollToEnd(proxy, viewModel: viewModel)
            }
            .onChange(of: viewModel.isParticipantTyping) { _, isTyping in
                guard isTyping else { return }
                withAnimation { proxy.scrollTo(Self.typingAnchor, anchor: .bottom) }
            }
        }
    }

    private func composer(_ model: ChatViewModel) -> some View {
        @Bindable var viewModel = model

        return HStack(alignment: .bottom, spacing: PlumTheme.Spacing.s) {
            TextField("Un message…", text: $viewModel.draft, axis: .vertical)
                .accessibilityIdentifier("champ-message")
                .lineLimit(1...5)
                .font(.plumBody)
                .padding(.horizontal, PlumTheme.Spacing.m)
                .padding(.vertical, 10)
                .background(PlumTheme.Palette.surface, in: Capsule())
                .focused($isComposerFocused)

            Button {
                Task { await viewModel.send() }
            } label: {
                Image(systemName: "arrow.up")
                    .font(.system(size: 18, weight: .bold))
                    .foregroundStyle(.white)
                    .frame(width: 42, height: 42)
                    .background(PlumTheme.Palette.warmGradient, in: Circle())
            }
            .accessibilityIdentifier("bouton-envoyer")
            .accessibilityLabel(Text("Envoyer"))
            .disabled(!viewModel.canSend)
            .opacity(viewModel.canSend ? 1 : 0.4)
        }
        .padding(.horizontal, PlumTheme.Spacing.m)
        .padding(.vertical, PlumTheme.Spacing.s)
        .background(.bar)
    }

    private static let typingAnchor = "typing-indicator"

    /// One timestamp per burst: show it on the last message, and whenever
    /// there is a gap of more than five minutes.
    private func shouldShowTimestamp(at index: Int, in items: [ChatItem]) -> Bool {
        guard index < items.count else { return false }
        guard index < items.count - 1 else { return true }
        let gap = items[index + 1].message.sentAt.timeIntervalSince(items[index].message.sentAt)
        return gap > 300
    }

    private func scrollToEnd(_ proxy: ScrollViewProxy, viewModel: ChatViewModel) {
        guard let last = viewModel.items.last else { return }
        withAnimation(.easeOut(duration: 0.25)) {
            proxy.scrollTo(last.id, anchor: .bottom)
        }
    }
}

#Preview {
    NavigationStack {
        ChatView(conversation: SampleData.conversations[0])
            .environment(\.services, .preview)
            .environment(SessionStore(auth: AppEnvironment.preview.auth))
    }
}
