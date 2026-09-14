import SwiftUI

/// La liste des conversations, au poignet.
struct ConversationsView: View {
    let link: WatchSessionStore

    @State private var conversations: [Conversation] = []
    @State private var chargement = false
    @State private var souci: String?

    var body: some View {
        Group {
            switch link.state {
            case .waiting:
                Annonce(
                    titre: "Presque prêt",
                    corps: "Ouvrez Plum sur votre iPhone une fois : la montre récupère votre session et n'aura plus besoin de lui."
                )
            case .rejected:
                Annonce(
                    titre: "Session expirée",
                    corps: "Ouvrez Plum sur votre iPhone pour la renouveler."
                )
            case let .ready(credentials):
                liste(credentials)
            }
        }
        .navigationTitle("Plum")
    }

    @ViewBuilder
    private func liste(_ credentials: WatchCredentials) -> some View {
        List {
            if let souci {
                Text(souci)
                    .font(.footnote)
                    .foregroundStyle(.secondary)
            }

            if conversations.isEmpty && !chargement && souci == nil {
                Annonce(
                    titre: "Rien encore",
                    corps: "Les conversations démarrées sur le téléphone apparaîtront ici."
                )
            }

            ForEach(conversations) { conversation in
                NavigationLink {
                    ThreadView(conversation: conversation, link: link, credentials: credentials)
                } label: {
                    Ligne(conversation: conversation)
                }
            }
        }
        .overlay { if chargement && conversations.isEmpty { ProgressView() } }
        .task(id: credentials.accessToken) { await charger(credentials) }
        .refreshable { await charger(credentials) }
    }

    private func charger(_ credentials: WatchCredentials) async {
        chargement = true
        defer { chargement = false }
        do {
            conversations = try await WatchChatClient(credentials: credentials).conversations()
            souci = nil
        } catch WatchChatClient.Failure.expired {
            // Le seul échec que la montre sait traiter : elle oublie, et
            // l'écran dit quoi faire. Réessayer serait une boucle.
            link.forget()
        } catch {
            souci = "Pas de réseau."
        }
    }
}

private struct Ligne: View {
    let conversation: Conversation

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack {
                Text(conversation.participant.displayName)
                    .font(.headline)
                    .lineLimit(1)
                if conversation.unreadCount > 0 {
                    Spacer()
                    // Une pastille, pas un nombre : à cette taille, « 3 » et
                    // « 8 » se confondent, et savoir qu'il y a du nouveau
                    // suffit à décider d'ouvrir.
                    Circle()
                        .fill(PlumBrand.blush)
                        .frame(width: 8, height: 8)
                }
            }
            if let dernier = conversation.lastMessage {
                Text(dernier.body)
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                    .lineLimit(2)
            }
        }
        .padding(.vertical, 2)
    }
}

/// L'écran vide, avec sa raison.
///
/// Nommée `Annonce` et non `Message` : ce dernier est le modèle d'un message
/// de conversation, compilé dans la montre depuis que les modèles sont
/// partagés. Deux types du même nom dans un même module ne cohabitent pas.
private struct Annonce: View {
    let titre: String
    let corps: String

    var body: some View {
        VStack(spacing: 6) {
            Text(titre).font(.headline)
            Text(corps)
                .font(.caption2)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 8)
        .listRowBackground(Color.clear)
    }
}
