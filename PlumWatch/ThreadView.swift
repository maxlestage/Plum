import SwiftUI

/// Un fil, et de quoi y répondre sans sortir son téléphone.
struct ThreadView: View {
    let conversation: Conversation
    let link: WatchSessionStore
    let credentials: WatchCredentials

    @State private var messages: [Message] = []
    @State private var chargement = false
    @State private var envoi = false
    @State private var souci: String?

    private var client: WatchChatClient { WatchChatClient(credentials: credentials) }

    var body: some View {
        ScrollViewReader { proxy in
            List {
                if let souci {
                    Text(souci).font(.footnote).foregroundStyle(.secondary)
                }

                ForEach(messages) { message in
                    Bulle(message: message, mienne: message.senderId == credentials.currentUserId)
                        .id(message.id)
                }

                Section("Répondre") {
                    ForEach(QuickReply.all, id: \.self) { texte in
                        Button(texte) { Task { await envoyer(texte) } }
                            .disabled(envoi)
                    }
                    // La dictée en dernier, sous les réponses toutes faites :
                    // c'est celle qui demande de parler à voix haute, donc
                    // celle qu'on choisit le moins souvent.
                    TextFieldLink("Dicter…") { texte in
                        Task { await envoyer(texte) }
                    }
                    .disabled(envoi)
                }
            }
            .overlay { if chargement && messages.isEmpty { ProgressView() } }
            .task { await charger(proxy) }
            .navigationTitle(conversation.participant.displayName)
        }
    }

    private func charger(_ proxy: ScrollViewProxy) async {
        chargement = true
        defer { chargement = false }
        do {
            messages = try await client.messages(in: conversation.id)
            souci = nil
            // Un fil s'ouvre sur son dernier message, pas sur le premier :
            // personne n'ouvre une conversation pour relire le début.
            if let dernier = messages.last { proxy.scrollTo(dernier.id, anchor: .bottom) }
        } catch WatchChatClient.Failure.expired {
            link.forget()
        } catch {
            souci = "Pas de réseau."
        }
    }

    private func envoyer(_ texte: String) async {
        let propre = texte.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !propre.isEmpty, !envoi else { return }
        envoi = true
        defer { envoi = false }
        do {
            let envoye = try await client.send(propre, to: conversation.id)
            messages.append(envoye)
            souci = nil
        } catch WatchChatClient.Failure.expired {
            link.forget()
        } catch {
            // Rien n'est ajouté à l'écran : montrer une bulle qui n'est pas
            // partie ferait croire à un message envoyé. Sur une montre, on ne
            // revient pas vérifier.
            souci = "Message non envoyé."
        }
    }
}

private struct Bulle: View {
    let message: Message
    let mienne: Bool

    var body: some View {
        HStack {
            if mienne { Spacer(minLength: 20) }
            Text(message.body)
                .font(.caption)
                .padding(.horizontal, 8)
                .padding(.vertical, 5)
                .background(mienne ? PlumBrand.plum : Color.gray.opacity(0.25))
                .foregroundStyle(mienne ? PlumBrand.cream : Color.primary)
                .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
            if !mienne { Spacer(minLength: 20) }
        }
        .listRowBackground(Color.clear)
    }
}
