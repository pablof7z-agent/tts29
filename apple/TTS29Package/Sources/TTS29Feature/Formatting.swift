import Foundation

/// Presentation-only formatters shared across the player surfaces. These render
/// device-local facts (elapsed time, byte sizes, generation age) and never
/// interpret protocol state.
public enum Formatting {
    /// Compact relative time for the first day ("just now", "4m", "3h"), then a
    /// stable absolute stamp — matching the reference player so an item keeps
    /// its original generation time even when replayed.
    public static func timestamp(_ date: Date, now: Date = Date()) -> String {
        let interval = now.timeIntervalSince(date)
        if interval < 60 { return "just now" }
        if interval < 3_600 { return "\(Int(interval / 60))m" }
        if interval < 86_400 { return "\(Int(interval / 3_600))h" }
        let formatter = DateFormatter()
        formatter.locale = .current
        let sameYear = Calendar.current.isDate(date, equalTo: now, toGranularity: .year)
        formatter.setLocalizedDateFormatFromTemplate(sameYear ? "MMMdHmm" : "yMMMd")
        return formatter.string(from: date)
    }

    /// `m:ss` (or `h:mm:ss`) clock for scrubbers and now-playing metadata.
    public static func clock(_ seconds: TimeInterval) -> String {
        guard seconds.isFinite, seconds >= 0 else { return "0:00" }
        let total = Int(seconds.rounded())
        let hours = total / 3_600
        let minutes = (total % 3_600) / 60
        let secs = total % 60
        if hours > 0 {
            return String(format: "%d:%02d:%02d", hours, minutes, secs)
        }
        return String(format: "%d:%02d", minutes, secs)
    }

    /// A short, human byte size for attachment cards.
    public static func byteCount(_ count: UInt64) -> String {
        let formatter = ByteCountFormatter()
        formatter.countStyle = .file
        formatter.allowsNonnumericFormatting = false
        return formatter.string(fromByteCount: Int64(bitPattern: count))
    }
}
