import Observation
import SwiftUI

/// Kept out of the view model so it stays free of actor isolation: it is used
/// as a navigation value, which must be usable from anywhere.
enum AuthMode: Identifiable, Hashable, Sendable {
    case signIn
    case signUp

    var id: Self { self }

    var title: String {
        switch self {
        case .signIn: return "Content de vous revoir"
        case .signUp: return "Bienvenue sur Plum"
        }
    }

    var callToAction: String {
        switch self {
        case .signIn: return "Se connecter"
        case .signUp: return "Créer mon compte"
        }
    }
}

/// Drives sign-in and sign-up. Validation happens here rather than in the
/// views so the rules can be tested without a UI.
@MainActor
@Observable
final class AuthViewModel {
    var mode: AuthMode
    var email = ""
    var password = ""
    var displayName = ""
    var gender: Gender = .woman
    var birthDate = Calendar.current.date(byAdding: .year, value: -25, to: .now) ?? .now

    private(set) var isSubmitting = false
    private(set) var errorMessage: String?

    private let auth: any AuthServicing
    private let session: SessionStore

    init(mode: AuthMode, auth: any AuthServicing, session: SessionStore) {
        self.mode = mode
        self.auth = auth
        self.session = session
    }

    // MARK: - Validation

    static func isValidEmail(_ email: String) -> Bool {
        let trimmed = email.trimmingCharacters(in: .whitespaces)
        // Deliberately loose: the server is the authority, this only catches
        // the obvious typo before a round trip.
        guard let at = trimmed.firstIndex(of: "@"), at != trimmed.startIndex else { return false }
        let domain = trimmed[trimmed.index(after: at)...]
        return domain.contains(".")
            && !domain.hasPrefix(".")
            && !domain.hasSuffix(".")
            && !trimmed.contains(" ")
    }

    static func isValidPassword(_ password: String) -> Bool {
        password.count >= 8
    }

    /// Plum is 18+. The picker enforces it too, but never trust the picker.
    static func isOldEnough(_ birthDate: Date, now: Date = .now) -> Bool {
        guard let age = Calendar.current.dateComponents([.year], from: birthDate, to: now).year else {
            return false
        }
        return age >= 18
    }

    var canSubmit: Bool {
        guard !isSubmitting,
              Self.isValidEmail(email),
              Self.isValidPassword(password) else { return false }
        guard mode == .signUp else { return true }
        return !displayName.trimmingCharacters(in: .whitespaces).isEmpty
            && Self.isOldEnough(birthDate)
    }

    var passwordHint: String? {
        guard !password.isEmpty, !Self.isValidPassword(password) else { return nil }
        return "8 caractères minimum."
    }

    // MARK: - Actions

    func submit() async {
        guard canSubmit else { return }
        isSubmitting = true
        errorMessage = nil
        defer { isSubmitting = false }

        do {
            let authenticated: AuthenticatedSession
            switch mode {
            case .signIn:
                authenticated = try await auth.signIn(
                    email: email.trimmingCharacters(in: .whitespaces).lowercased(),
                    password: password
                )
            case .signUp:
                authenticated = try await auth.signUp(
                    SignUpRequest(
                        email: email.trimmingCharacters(in: .whitespaces).lowercased(),
                        password: password,
                        displayName: displayName.trimmingCharacters(in: .whitespaces),
                        birthDate: birthDate,
                        gender: gender
                    )
                )
            }
            Haptics.play(.success)
            session.adopt(authenticated)
        } catch {
            Haptics.play(.warning)
            errorMessage = error.asAPIError.userMessage
        }
    }

    func clearError() {
        errorMessage = nil
    }
}
