import Foundation

/// Remembers a listening speed per agent, mirroring the reference player's
/// per-voice memory: different producers have different cadences, so the rate
/// you pick for one is restored the next time you hear them.
@MainActor
public final class PlaybackRateStore {
    /// The speeds the transport capsule cycles through on tap.
    public static let cycle: [Float] = [1.0, 1.2, 1.5, 2.0, 0.9]
    /// The fuller set offered in the long-press menu.
    public static let menu: [Float] = [0.8, 0.9, 1.0, 1.2, 1.5, 1.75, 2.0, 2.5]

    private let defaults: UserDefaults
    private let prefix = "tts29.rate."

    public init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
    }

    public func rate(for agent: String) -> Float {
        let stored = defaults.object(forKey: key(agent)) as? Double
        guard let stored, stored > 0 else { return 1.0 }
        return Float(stored)
    }

    public func setRate(_ rate: Float, for agent: String) {
        defaults.set(Double(rate), forKey: key(agent))
    }

    /// The next rate in the cycle after the current one, wrapping around.
    public func nextRate(after rate: Float) -> Float {
        let cycle = Self.cycle
        guard let index = cycle.firstIndex(where: { abs($0 - rate) < 0.001 }) else {
            return cycle.first ?? 1.0
        }
        return cycle[(index + 1) % cycle.count]
    }

    private func key(_ agent: String) -> String {
        let trimmed = agent.trimmingCharacters(in: .whitespacesAndNewlines)
        return prefix + (trimmed.isEmpty ? "default" : trimmed)
    }
}

public extension Float {
    /// A compact "1×" / "1.2×" label with no trailing zeroes.
    var rateLabel: String {
        let rounded = (self * 100).rounded() / 100
        if rounded == rounded.rounded() {
            return "\(Int(rounded))×"
        }
        return "\(String(format: "%g", rounded))×"
    }
}
