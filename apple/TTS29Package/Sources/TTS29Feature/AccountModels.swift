import Foundation

public enum IdentityPhase: String, Codable, Sendable {
    case signedOut = "signed_out"
    case saving
    case signedIn = "signed_in"
    case loggingOut = "logging_out"
}

public struct IdentitySnapshot: Codable, Sendable, Equatable {
    public let phase: IdentityPhase
    public let pubkey: String?
    public let error: String?

    public static let signedOut = IdentitySnapshot(phase: .signedOut, pubkey: nil, error: nil)

    public init(phase: IdentityPhase, pubkey: String?, error: String?) {
        self.phase = phase
        self.pubkey = pubkey
        self.error = error
    }

    public var shortPubkey: String? {
        guard let pubkey, pubkey.count > 12 else { return pubkey }
        return "\(pubkey.prefix(8))…\(pubkey.suffix(4))"
    }
}

public enum CredentialOperation: String, Codable, Sendable {
    case store
    case delete
}

public struct CredentialRequest: Codable, Sendable, Equatable {
    public let id: UInt64
    public let operation: CredentialOperation
}

public enum AnswerSubmissionPhase: String, Codable, Sendable {
    case sending
    case published
    case failed
}

public struct AnswerSubmission: Codable, Sendable, Equatable, Identifiable {
    public let itemId: String
    public let phase: AnswerSubmissionPhase
    public let receiptId: UInt64?
    public let eventId: String?
    public let error: String?

    public var id: String { itemId }

    enum CodingKeys: String, CodingKey {
        case phase, error
        case itemId = "item_id"
        case receiptId = "receipt_id"
        case eventId = "event_id"
    }
}
