import Foundation

public struct SpokenItem: Codable, Identifiable, Sendable {
    public let id: String
    public let author: String
    public let createdAt: UInt64
    public let subject: String
    public let summary: String
    public let body: String
    public let audioURL: String?

    enum CodingKeys: String, CodingKey {
        case id, author, subject, summary, body
        case createdAt = "created_at"
        case audioURL = "audio_url"
    }

    public init(
        id: String,
        author: String,
        createdAt: UInt64,
        subject: String,
        summary: String,
        body: String,
        audioURL: String?
    ) {
        self.id = id
        self.author = author
        self.createdAt = createdAt
        self.subject = subject
        self.summary = summary
        self.body = body
        self.audioURL = audioURL
    }
}

public enum KernelPhase: String, Codable, Sendable {
    case starting
    case listening
    case failed
    case stopped

    var symbolName: String {
        switch self {
        case .starting: "hourglass"
        case .listening: "antenna.radiowaves.left.and.right"
        case .failed: "exclamationmark.triangle"
        case .stopped: "stop.circle"
        }
    }
}

public struct QueueEvidence: Codable, Sendable {
    public let sourceCount: Int
    public let shortfallCount: Int

    enum CodingKeys: String, CodingKey {
        case sourceCount = "source_count"
        case shortfallCount = "shortfall_count"
    }

    public init(sourceCount: Int, shortfallCount: Int) {
        self.sourceCount = sourceCount
        self.shortfallCount = shortfallCount
    }
}

public struct QueueSnapshot: Codable, Sendable {
    public let phase: KernelPhase
    public let relay: String
    public let groupID: String
    public let items: [SpokenItem]
    public let evidence: QueueEvidence
    public let error: String?

    enum CodingKeys: String, CodingKey {
        case phase, relay, items, evidence, error
        case groupID = "group_id"
    }

    public static let initial = QueueSnapshot(
        phase: .starting,
        relay: "",
        groupID: "",
        items: [],
        evidence: QueueEvidence(sourceCount: 0, shortfallCount: 0),
        error: nil
    )

    public init(
        phase: KernelPhase,
        relay: String,
        groupID: String,
        items: [SpokenItem],
        evidence: QueueEvidence,
        error: String?
    ) {
        self.phase = phase
        self.relay = relay
        self.groupID = groupID
        self.items = items
        self.evidence = evidence
        self.error = error
    }

    public var statusMessage: String {
        switch phase {
        case .starting: "Starting NMP queue…"
        case .listening: "Listening for group updates"
        case .failed: error ?? "The queue could not start"
        case .stopped: "Queue stopped"
        }
    }
}
