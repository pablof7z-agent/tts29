import Foundation

public struct SpokenItem: Codable, Identifiable, Sendable, Equatable, Hashable {
    public let id: String
    public let author: String
    public let createdAt: UInt64
    public let agentName: String
    public let subject: String
    public let summary: String
    public let body: String
    public let audioURL: String?
    public let audio: DurableArtifact?
    public let attachments: [DurableArtifact]
    public let questions: [Question]
    public let answer: AnswerBundle?
    public let acknowledgement: Acknowledgement?
    public let reactions: [ReactionSummary]

    enum CodingKeys: String, CodingKey {
        case id, author, subject, summary, body, audio, attachments, questions, answer
        case acknowledgement, reactions
        case createdAt = "created_at"
        case agentName = "agent_name"
        case audioURL = "audio_url"
    }

    public init(
        id: String,
        author: String,
        createdAt: UInt64,
        subject: String,
        summary: String,
        body: String,
        audioURL: String?,
        agentName: String = "",
        audio: DurableArtifact? = nil,
        attachments: [DurableArtifact] = [],
        questions: [Question] = [],
        answer: AnswerBundle? = nil,
        acknowledgement: Acknowledgement? = nil,
        reactions: [ReactionSummary] = []
    ) {
        self.id = id
        self.author = author
        self.createdAt = createdAt
        self.agentName = agentName
        self.subject = subject
        self.summary = summary
        self.body = body
        self.audioURL = audioURL
        self.audio = audio
        self.attachments = attachments
        self.questions = questions
        self.answer = answer
        self.acknowledgement = acknowledgement
        self.reactions = reactions
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(String.self, forKey: .id)
        author = try container.decode(String.self, forKey: .author)
        createdAt = try container.decode(UInt64.self, forKey: .createdAt)
        agentName = try container.decodeIfPresent(String.self, forKey: .agentName) ?? ""
        subject = try container.decode(String.self, forKey: .subject)
        summary = try container.decode(String.self, forKey: .summary)
        body = try container.decode(String.self, forKey: .body)
        audioURL = try container.decodeIfPresent(String.self, forKey: .audioURL)
        audio = try container.decodeIfPresent(DurableArtifact.self, forKey: .audio)
        attachments = try container.decodeIfPresent([DurableArtifact].self, forKey: .attachments) ?? []
        questions = try container.decodeIfPresent([Question].self, forKey: .questions) ?? []
        answer = try container.decodeIfPresent(AnswerBundle.self, forKey: .answer)
        acknowledgement = try container.decodeIfPresent(Acknowledgement.self, forKey: .acknowledgement)
        reactions = try container.decodeIfPresent([ReactionSummary].self, forKey: .reactions) ?? []
    }

    /// The playable audio source, preferring the convenience URL and falling
    /// back to the durable artifact the kernel projects.
    public var playableURL: String? { audioURL ?? audio?.url }

    public var hasAttachments: Bool { !attachments.isEmpty }
    public var hasQuestions: Bool { !questions.isEmpty }
    public var isAnswered: Bool { answer != nil }
    public var isHeard: Bool { acknowledgement?.state == .heard }

    /// The item's original generation moment, independent of any replay.
    public var createdDate: Date { Date(timeIntervalSince1970: TimeInterval(createdAt)) }
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
