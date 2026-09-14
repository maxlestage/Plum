import Foundation

/// A verdict on a card. `superLike` is the one that costs you something.
enum SwipeDecision: String, Codable, Sendable {
    case like
    case pass
    case superLike
}

struct SwipeRequest: Codable, Sendable {
    let targetProfileId: UUID
    let decision: SwipeDecision
}

/// The answer to a swipe: the backend tells us straight away whether the like
/// was mutual, because that is the only moment the celebration makes sense.
struct SwipeOutcome: Codable, Sendable {
    let matched: Bool
    let match: Match?
    /// Remaining likes before the daily cap. `nil` means unlimited.
    let likesRemaining: Int?
}
