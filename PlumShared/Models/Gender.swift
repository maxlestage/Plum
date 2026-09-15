import Foundation

enum Gender: String, Codable, CaseIterable, Identifiable, Sendable {
    case woman
    case man
    case nonBinary
    case other

    var id: String { rawValue }

    var label: String {
        switch self {
        case .woman: return "Femme"
        case .man: return "Homme"
        case .nonBinary: return "Non-binaire"
        case .other: return "Autre"
        }
    }
}

/// Who a member wants to see in their selection.
enum GenderPreference: String, Codable, CaseIterable, Identifiable, Sendable {
    case women
    case men
    case everyone

    var id: String { rawValue }

    var label: String {
        switch self {
        case .women: return "Des femmes"
        case .men: return "Des hommes"
        case .everyone: return "Tout le monde"
        }
    }
}
