import Foundation

/// For screens that own their data in a separate property and only need to
/// know whether work is in flight and whether it went wrong.
enum ActivityState: Sendable, Equatable {
    case idle
    case loading
    case ready
    case failed(APIError)

    var error: APIError? {
        if case let .failed(error) = self { return error }
        return nil
    }

    var isLoading: Bool {
        if case .loading = self { return true }
        return false
    }
}

extension Error {
    /// Everything thrown at the view layer is shown as an ``APIError``, so an
    /// unexpected error still reads as something rather than a crash.
    var asAPIError: APIError {
        (self as? APIError) ?? .transport(localizedDescription)
    }
}
