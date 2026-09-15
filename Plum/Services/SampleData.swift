import Foundation

/// Fixtures for previews, the offline demo mode and tests. Not compiled out of
/// release builds on purpose: the demo mode is how the app is shown without a
/// server running.
enum SampleData {
    static func date(_ iso: String) -> Date {
        PlumDateFormat.date(from: iso) ?? Date(timeIntervalSince1970: 0)
    }

    static func birthDate(age: Int) -> Date {
        Calendar.current.date(byAdding: .year, value: -age, to: .now) ?? .now
    }

    static let currentUser = User(
        id: UUID(uuidString: "00000000-0000-0000-0000-0000000000A1")!,
        email: "moi@plum.app",
        createdAt: date("2025-11-02T10:15:00Z"),
        profileCompleted: true
    )

    static let myProfile = Profile(
        id: currentUser.id,
        displayName: "Camille",
        birthDate: birthDate(age: 29),
        gender: .nonBinary,
        bio: "Ici pour les terrasses, les mauvais films et les discussions qui finissent à 3 h.",
        city: "Paris",
        photos: [photo(seed: "camille-1", position: 0), photo(seed: "camille-2", position: 1)],
        interests: ["Cinéma", "Rooftops", "Vinyles", "Randonnée"]
    )

    static let selection: [Profile] = [
        Profile(
            id: UUID(uuidString: "00000000-0000-0000-0000-0000000000B1")!,
            displayName: "Inès",
            birthDate: birthDate(age: 27),
            gender: .woman,
            bio: "Je gagne toujours au babyfoot. Prouvez-moi le contraire.",
            city: "Paris 11e",
            photos: [photo(seed: "ines-1", position: 0), photo(seed: "ines-2", position: 1)],
            interests: ["Babyfoot", "Ramen", "Techno"],
            distanceKm: 2.4,
            lastActiveAt: .now.addingTimeInterval(-600)
        ),
        Profile(
            id: UUID(uuidString: "00000000-0000-0000-0000-0000000000B2")!,
            displayName: "Théo",
            birthDate: birthDate(age: 31),
            gender: .man,
            bio: "Cuisinier la nuit, cinéphile le reste du temps. Zéro plan sérieux.",
            city: "Montreuil",
            photos: [photo(seed: "theo-1", position: 0)],
            interests: ["Cuisine", "Vélo", "Kurosawa"],
            distanceKm: 7.8,
            lastActiveAt: .now.addingTimeInterval(-3_600)
        ),
        Profile(
            id: UUID(uuidString: "00000000-0000-0000-0000-0000000000B3")!,
            displayName: "Sacha",
            birthDate: birthDate(age: 25),
            gender: .nonBinary,
            bio: "Playlists trop longues, messages trop courts.",
            city: "Pantin",
            photos: [photo(seed: "sacha-1", position: 0), photo(seed: "sacha-2", position: 1)],
            interests: ["Musique", "Skate", "Expos"],
            distanceKm: 12.1,
            lastActiveAt: .now.addingTimeInterval(-86_400)
        ),
        Profile(
            id: UUID(uuidString: "00000000-0000-0000-0000-0000000000B4")!,
            displayName: "Léa",
            birthDate: birthDate(age: 33),
            gender: .woman,
            bio: "Deux chats, aucun projet à cinq ans.",
            city: "Saint-Ouen",
            photos: [photo(seed: "lea-1", position: 0)],
            interests: ["Chats", "Escalade", "Karaoké"],
            distanceKm: 21.6,
            lastActiveAt: .now.addingTimeInterval(-7_200)
        )
    ]

    static let matches: [Match] = [
        Match(
            id: UUID(uuidString: "00000000-0000-0000-0000-0000000000C1")!,
            profile: deck[0],
            matchedAt: .now.addingTimeInterval(-4_000),
            conversationId: conversations[0].id
        ),
        Match(
            id: UUID(uuidString: "00000000-0000-0000-0000-0000000000C2")!,
            profile: deck[2],
            matchedAt: .now.addingTimeInterval(-100_000),
            conversationId: conversations[1].id
        )
    ]

    static let conversations: [Conversation] = [
        Conversation(
            id: UUID(uuidString: "00000000-0000-0000-0000-0000000000D1")!,
            matchId: UUID(uuidString: "00000000-0000-0000-0000-0000000000C1")!,
            participant: deck[0],
            lastMessage: Message(
                id: UUID(uuidString: "00000000-0000-0000-0000-0000000000E1")!,
                conversationId: UUID(uuidString: "00000000-0000-0000-0000-0000000000D1")!,
                senderId: deck[0].id,
                body: "Babyfoot jeudi, tu perds d'avance.",
                sentAt: .now.addingTimeInterval(-1_200)
            ),
            unreadCount: 2,
            updatedAt: .now.addingTimeInterval(-1_200)
        ),
        Conversation(
            id: UUID(uuidString: "00000000-0000-0000-0000-0000000000D2")!,
            matchId: UUID(uuidString: "00000000-0000-0000-0000-0000000000C2")!,
            participant: deck[2],
            lastMessage: Message(
                id: UUID(uuidString: "00000000-0000-0000-0000-0000000000E2")!,
                conversationId: UUID(uuidString: "00000000-0000-0000-0000-0000000000D2")!,
                senderId: currentUser.id,
                body: "Envoie la playlist alors",
                sentAt: .now.addingTimeInterval(-90_000)
            ),
            updatedAt: .now.addingTimeInterval(-90_000)
        )
    ]

    static func thread(for conversation: Conversation) -> [Message] {
        let other = conversation.participant.id
        let me = currentUser.id
        let lines: [(UUID, String, TimeInterval)] = [
            (other, "Alors, ce profil, c'est toi qui l'as écrit ou un ami ?", -9_000),
            (me, "Moi. Mon ami aurait fait pire.", -8_400),
            (other, "Rassurant.", -8_100),
            (me, "On boit un truc cette semaine ?", -3_000),
            (other, conversation.preview, -1_200)
        ]
        return lines.enumerated().map { index, line in
            Message(
                id: UUID(),
                conversationId: conversation.id,
                senderId: line.0,
                body: line.1,
                sentAt: .now.addingTimeInterval(line.2),
                readAt: index < 3 ? .now.addingTimeInterval(line.2 + 60) : nil
            )
        }
    }

    private static func photo(seed: String, position: Int) -> Photo {
        Photo(
            id: UUID(),
            url: URL(string: "https://picsum.photos/seed/\(seed)/900/1200")!,
            position: position
        )
    }
}
