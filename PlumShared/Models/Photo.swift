import Foundation

struct Photo: Identifiable, Codable, Hashable, Sendable {
    let id: UUID
    var url: URL
    /// Position in the profile carousel, 0 being the cover photo.
    var position: Int

    init(id: UUID, url: URL, position: Int) {
        self.id = id
        self.url = url
        self.position = position
    }
}
