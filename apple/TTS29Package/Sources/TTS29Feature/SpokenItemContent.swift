import Foundation

/// A durable, content-addressed artifact published alongside a spoken item:
/// the item's own audio or one of its labeled attachments.
public struct DurableArtifact: Codable, Hashable, Sendable, Identifiable {
    public let url: String
    public let sha256: String
    public let mediaType: String
    public let byteCount: UInt64
    public let label: String?

    enum CodingKeys: String, CodingKey {
        case url, sha256, label
        case mediaType = "media_type"
        case byteCount = "byte_count"
    }

    public var id: String { sha256.isEmpty ? url : sha256 }

    public init(
        url: String,
        sha256: String,
        mediaType: String,
        byteCount: UInt64,
        label: String? = nil
    ) {
        self.url = url
        self.sha256 = sha256
        self.mediaType = mediaType
        self.byteCount = byteCount
        self.label = label
    }
}

public enum QuestionKind: String, Codable, Sendable {
    case singleChoice = "single_choice"
    case multipleChoice = "multiple_choice"
    case freeform
}

public struct QuestionOption: Codable, Hashable, Sendable, Identifiable {
    public let id: String
    public let title: String
    public let description: String?

    public init(id: String, title: String, description: String? = nil) {
        self.id = id
        self.title = title
        self.description = description
    }
}

public struct Question: Codable, Hashable, Sendable, Identifiable {
    public let id: String
    public let kind: QuestionKind
    public let shortTitle: String
    public let title: String
    public let description: String?
    public let options: [QuestionOption]

    enum CodingKeys: String, CodingKey {
        case id, kind, title, description, options
        case shortTitle = "short_title"
    }

    public init(
        id: String,
        kind: QuestionKind,
        shortTitle: String,
        title: String,
        description: String? = nil,
        options: [QuestionOption] = []
    ) {
        self.id = id
        self.kind = kind
        self.shortTitle = shortTitle
        self.title = title
        self.description = description
        self.options = options
    }
}

public struct QuestionAnswer: Codable, Hashable, Sendable, Identifiable {
    public let questionId: String
    public let values: [String]

    enum CodingKeys: String, CodingKey {
        case values
        case questionId = "question_id"
    }

    public var id: String { questionId }

    public init(questionId: String, values: [String]) {
        self.questionId = questionId
        self.values = values
    }
}

public struct AnswerBundle: Codable, Hashable, Sendable {
    public let eventId: String
    public let author: String
    public let createdAt: UInt64
    public let answers: [QuestionAnswer]

    enum CodingKeys: String, CodingKey {
        case author, answers
        case eventId = "event_id"
        case createdAt = "created_at"
    }

    public init(eventId: String, author: String, createdAt: UInt64, answers: [QuestionAnswer]) {
        self.eventId = eventId
        self.author = author
        self.createdAt = createdAt
        self.answers = answers
    }

    public func values(for questionId: String) -> [String] {
        answers.first { $0.questionId == questionId }?.values ?? []
    }
}

public enum AcknowledgementState: String, Codable, Sendable {
    case active
    case heard
    case dismissed
    case archived
}

public struct Acknowledgement: Codable, Hashable, Sendable {
    public let eventId: String
    public let author: String
    public let createdAt: UInt64
    public let state: AcknowledgementState
    public let reason: String?

    enum CodingKeys: String, CodingKey {
        case author, state, reason
        case eventId = "event_id"
        case createdAt = "created_at"
    }

    public init(
        eventId: String,
        author: String,
        createdAt: UInt64,
        state: AcknowledgementState,
        reason: String? = nil
    ) {
        self.eventId = eventId
        self.author = author
        self.createdAt = createdAt
        self.state = state
        self.reason = reason
    }
}

/// Links a narrated child item to the parent that references it. The parent's
/// inline `[label](attachment:)` is matched against this child's `subject`.
public struct AttachLink: Codable, Hashable, Sendable {
    public let parentId: String

    enum CodingKeys: String, CodingKey {
        case parentId = "parent_id"
    }

    public init(parentId: String) {
        self.parentId = parentId
    }
}

public struct ReactionSummary: Codable, Hashable, Sendable, Identifiable {
    public let emoji: String
    public let count: Int
    public let authors: [String]

    public var id: String { emoji }

    public init(emoji: String, count: Int, authors: [String] = []) {
        self.emoji = emoji
        self.count = count
        self.authors = authors
    }
}
