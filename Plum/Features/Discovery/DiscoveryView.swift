import SwiftUI

/// The deck. Owns the drag of the top card and turns it into a decision; the
/// view model owns everything that outlives the gesture.
struct DiscoveryView: View {
    @Environment(\.services) private var services
    @Environment(DeckRefreshSignal.self) private var deckRefresh
    @State private var viewModel: DiscoveryViewModel?
    @State private var drag: CGSize = .zero
    @State private var isCommitting = false
    @State private var reportTarget: Profile?
    @State private var detailProfile: Profile?
    @State private var locationSync: LocationSync?

    var body: some View {
        PlumBackground {
            VStack(spacing: 0) {
                header
                locationPrompt

                if let viewModel {
                    deck(for: viewModel)

                    if let error = viewModel.state.error {
                        ErrorBanner(message: error.userMessage) {
                            Task { await viewModel.refresh() }
                        }
                        .padding(.bottom, PlumTheme.Spacing.s)
                    }

                    actionBar(for: viewModel)
                } else {
                    Spacer()
                    ProgressView().tint(PlumTheme.Palette.plum)
                    Spacer()
                }
            }
        }
        .task {
            // Built here rather than in `init` so it gets the environment's
            // services, which the demo mode may have swapped out.
            if viewModel == nil {
                viewModel = DiscoveryViewModel(discovery: services.discovery)
            }
            if locationSync == nil {
                locationSync = LocationSync(provider: services.location, profiles: services.profiles)
            }
            await locationSync?.refreshAuthorization()
            await viewModel?.loadInitialDeck()
        }
        .onChange(of: deckRefresh.token) { _, _ in
            Task { await viewModel?.refresh() }
        }
        .sheet(item: $detailProfile) { profile in
            ProfileDetailView(
                profile: profile,
                onDecision: { decision in
                    guard let viewModel else { return }
                    commit(decision, viewModel: viewModel)
                },
                onReport: { reportTarget = profile }
            )
        }
        .sheet(item: $reportTarget) { profile in
            ReportSheet(profile: profile) { reason in
                Task { await viewModel?.report(profile, reason: reason) }
            } onBlock: {
                Task { await viewModel?.block(profile) }
            }
        }
        .fullScreenCover(
            isPresented: Binding(
                get: { viewModel?.newMatch != nil },
                set: { if !$0 { viewModel?.dismissMatch() } }
            )
        ) {
            if let match = viewModel?.newMatch {
                MatchCelebrationView(match: match) {
                    viewModel?.dismissMatch()
                }
            }
        }
    }

    // MARK: - Pieces

    private var header: some View {
        HStack {
            PlumMark(size: 32)
            Text("Plum")
                .font(.plumTitle)
                .foregroundStyle(PlumTheme.Palette.plum)
            Spacer()
            if let remaining = viewModel?.likesRemaining {
                Label("\(remaining)", systemImage: "heart")
                    .font(.plumCaption)
                    .foregroundStyle(PlumTheme.Palette.secondaryText)
            }
        }
        .padding(.horizontal, PlumTheme.Spacing.l)
        .padding(.bottom, PlumTheme.Spacing.s)
    }

    /// Someone signing in on a new device skips onboarding entirely, so this
    /// is the only place they would ever be asked. It stays a strip rather
    /// than a modal: a deck without distances still works.
    @ViewBuilder
    private var locationPrompt: some View {
        if let locationSync, locationSync.needsPermission {
            Button {
                Task { await locationSync.requestPermissionAndSync() }
            } label: {
                HStack(spacing: PlumTheme.Spacing.s) {
                    Image(systemName: "location.circle.fill")
                    Text("Activez la localisation pour voir qui est près de vous.")
                        .font(.plumCaption)
                        .multilineTextAlignment(.leading)
                    Spacer(minLength: 0)
                }
                .foregroundStyle(PlumTheme.Palette.plum)
                .padding(PlumTheme.Spacing.s)
                .background(
                    RoundedRectangle(cornerRadius: PlumTheme.Radius.small, style: .continuous)
                        .fill(PlumTheme.Palette.plum.opacity(0.1))
                )
            }
            .buttonStyle(.plain)
            .padding(.horizontal, PlumTheme.Spacing.l)
            .padding(.bottom, PlumTheme.Spacing.s)
        }
    }

    @ViewBuilder
    private func deck(for viewModel: DiscoveryViewModel) -> some View {
        ZStack {
            if viewModel.isEmpty {
                EmptyStateView(
                    systemImage: "sparkles",
                    title: "Plus personne pour l'instant",
                    message: "Revenez tout à l'heure, ou élargissez vos critères dans les réglages.",
                    actionTitle: "Recharger"
                ) {
                    Task { await viewModel.refresh() }
                }
            }

            // Drawn back-to-front so the top card is last and receives touches.
            ForEach(Array(viewModel.visibleProfiles.enumerated()).reversed(), id: \.element.id) { index, profile in
                let isTop = index == 0
                SwipeCardView(
                    profile: profile,
                    dragTranslation: isTop ? drag : .zero,
                    isTopCard: isTop,
                    onOpenDetail: isTop ? { detailProfile = profile } : nil
                )
                .scaleEffect(scale(forDepth: index))
                .offset(y: CGFloat(index) * 12)
                .offset(isTop ? drag : .zero)
                .rotationEffect(.degrees(isTop ? rotation : 0), anchor: .bottom)
                .gesture(
                    dragGesture(for: profile, viewModel: viewModel),
                    including: isTop ? .all : .subviews
                )
                .contextMenu {
                    if isTop {
                        Button(role: .destructive) {
                            reportTarget = profile
                        } label: {
                            Label("Signaler ou bloquer", systemImage: "flag")
                        }
                    }
                }
                .zIndex(Double(PlumTheme.Layout.cardStackDepth - index))
                .animation(.spring(response: 0.35, dampingFraction: 0.8), value: viewModel.visibleProfiles.map(\.id))
                // The deck is a drag gesture, which VoiceOver cannot perform.
                // These are the same three verdicts as rotor actions.
                .accessibilityHidden(!isTop)
                .accessibilityAction(named: Text("J'aime")) {
                    commit(.like, viewModel: viewModel)
                }
                .accessibilityAction(named: Text("Passer")) {
                    commit(.pass, viewModel: viewModel)
                }
                .accessibilityAction(named: Text("Coup de cœur")) {
                    commit(.superLike, viewModel: viewModel)
                }
                .accessibilityAction(named: Text("Voir le profil complet")) {
                    detailProfile = profile
                }
                .accessibilityAction(named: Text("Signaler ou bloquer")) {
                    reportTarget = profile
                }
            }
        }
        .padding(.horizontal, PlumTheme.Spacing.l)
        .padding(.bottom, PlumTheme.Spacing.m)
    }

    private func actionBar(for viewModel: DiscoveryViewModel) -> some View {
        HStack(spacing: PlumTheme.Spacing.l) {
            CircularActionButton(
                systemImage: "arrow.uturn.backward",
                tint: PlumTheme.Palette.apricot,
                diameter: 48,
                accessibilityTitle: "Annuler le dernier passe"
            ) {
                Task { await viewModel.rewind() }
            }
            .disabled(!viewModel.canRewind)
            .opacity(viewModel.canRewind ? 1 : 0.4)

            CircularActionButton(
                systemImage: "xmark",
                tint: PlumTheme.Palette.pass,
                accessibilityTitle: "Passer"
            ) {
                commit(.pass, viewModel: viewModel)
            }

            CircularActionButton(
                systemImage: "star.fill",
                tint: PlumTheme.Palette.superLike,
                diameter: 48,
                accessibilityTitle: "Coup de cœur"
            ) {
                commit(.superLike, viewModel: viewModel)
            }

            CircularActionButton(
                systemImage: "heart.fill",
                tint: PlumTheme.Palette.like,
                isProminent: true,
                accessibilityTitle: "J'aime"
            ) {
                commit(.like, viewModel: viewModel)
            }
        }
        .padding(.bottom, PlumTheme.Spacing.m)
        .disabled(viewModel.topProfile == nil || isCommitting)
    }

    // MARK: - Gesture

    private var rotation: Double {
        let progress = Double(drag.width / 220)
        return max(-1, min(1, progress)) * PlumTheme.Layout.maxCardRotation
    }

    private func scale(forDepth index: Int) -> CGFloat {
        1 - (CGFloat(index) * 0.04)
    }

    private func dragGesture(for profile: Profile, viewModel: DiscoveryViewModel) -> some Gesture {
        DragGesture(minimumDistance: 8)
            .onChanged { value in
                guard !isCommitting else { return }
                drag = value.translation
            }
            .onEnded { value in
                guard !isCommitting else { return }
                if let decision = Self.decision(for: value.translation) {
                    commit(decision, viewModel: viewModel, startingFrom: value.translation)
                } else {
                    withAnimation(.spring(response: 0.35, dampingFraction: 0.7)) {
                        drag = .zero
                    }
                }
            }
    }

    /// The gesture's meaning, extracted so it can be tested without a view.
    static func decision(for translation: CGSize) -> SwipeDecision? {
        if -translation.height > PlumTheme.Layout.swipeUpCommitThreshold,
           abs(translation.width) < PlumTheme.Layout.swipeCommitThreshold {
            return .superLike
        }
        if translation.width > PlumTheme.Layout.swipeCommitThreshold { return .like }
        if translation.width < -PlumTheme.Layout.swipeCommitThreshold { return .pass }
        return nil
    }

    /// Flies the card off in the direction of the verdict, then tells the view
    /// model. The animation is the feedback, so it runs first.
    private func commit(
        _ decision: SwipeDecision,
        viewModel: DiscoveryViewModel,
        startingFrom translation: CGSize = .zero
    ) {
        guard let profile = viewModel.topProfile, !isCommitting else { return }
        isCommitting = true
        drag = translation

        withAnimation(.easeOut(duration: 0.28)) {
            drag = Self.exitTranslation(for: decision)
        }

        Task { @MainActor in
            try? await Task.sleep(for: .milliseconds(220))
            await viewModel.swipe(profile, decision: decision)
            drag = .zero
            isCommitting = false
        }
    }

    static func exitTranslation(for decision: SwipeDecision) -> CGSize {
        switch decision {
        case .like: return CGSize(width: 700, height: -80)
        case .pass: return CGSize(width: -700, height: -80)
        case .superLike: return CGSize(width: 0, height: -900)
        }
    }
}

#Preview {
    DiscoveryView()
        .environment(\.services, .preview)
        .environment(SessionStore(auth: AppEnvironment.preview.auth))
        .environment(DeckRefreshSignal())
}
