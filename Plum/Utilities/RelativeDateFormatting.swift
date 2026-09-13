import Foundation

/// Relative dates, in French, in one place. `RelativeDateTimeFormatter` is
/// cheap to reuse and expensive to recreate per row.
enum RelativeDateFormatting {
    private static let relative: RelativeDateTimeFormatter = {
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .short
        formatter.locale = Locale(identifier: "fr_FR")
        return formatter
    }()

    private static let clock: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "fr_FR")
        formatter.dateFormat = "HH:mm"
        return formatter
    }()

    private static let dayAndClock: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "fr_FR")
        formatter.setLocalizedDateFormatFromTemplate("EEE HH:mm")
        return formatter
    }()

    static func phrase(for date: Date, relativeTo now: Date = .now) -> String {
        relative.localizedString(for: date, relativeTo: now)
    }

    /// Timestamps inside a conversation: the time today, the weekday this week,
    /// the date beyond that.
    static func messageTimestamp(for date: Date, relativeTo now: Date = .now) -> String {
        let calendar = Calendar.current
        if calendar.isDateInToday(date) {
            return clock.string(from: date)
        }
        if let days = calendar.dateComponents([.day], from: date, to: now).day, days < 7 {
            return dayAndClock.string(from: date)
        }
        return date.formatted(date: .abbreviated, time: .shortened)
    }

    /// The right-hand label in the conversation list.
    static func inboxTimestamp(for date: Date, relativeTo now: Date = .now) -> String {
        let calendar = Calendar.current
        if calendar.isDateInToday(date) { return clock.string(from: date) }
        if calendar.isDateInYesterday(date) { return "Hier" }
        return date.formatted(.dateTime.day().month(.abbreviated))
    }
}
