import Foundation
import Observation

private typealias SnapshotCallback = @convention(c) (
    UnsafePointer<CChar>?,
    UnsafeMutableRawPointer?
) -> Void

@_silgen_name("tts29_start")
private func startKernel(
    _ configuration: UnsafePointer<CChar>,
    _ callback: SnapshotCallback?,
    _ context: UnsafeMutableRawPointer?
) -> UnsafeMutableRawPointer?

@_silgen_name("tts29_login")
private func loginKernel(_ handle: UnsafeMutableRawPointer?, _ secret: UnsafePointer<CChar>)

@_silgen_name("tts29_restore_login")
private func restoreKernelLogin(_ handle: UnsafeMutableRawPointer?, _ secret: UnsafePointer<CChar>)

@_silgen_name("tts29_credential_load_failed")
private func reportCredentialLoadFailure(
    _ handle: UnsafeMutableRawPointer?,
    _ error: UnsafePointer<CChar>
)

@_silgen_name("tts29_dispatch")
private func dispatchKernel(_ handle: UnsafeMutableRawPointer?, _ action: UnsafePointer<CChar>)

@_silgen_name("tts29_credential_result")
private func reportCredentialResult(
    _ handle: UnsafeMutableRawPointer?,
    _ requestID: UInt64,
    _ succeeded: Bool,
    _ error: UnsafePointer<CChar>?
)

@_silgen_name("tts29_stop")
private func stopKernel(_ handle: UnsafeMutableRawPointer?)

@Observable
@MainActor
public final class TTS29Store {
    public private(set) var snapshot: QueueSnapshot
    private var isRunning = false
    private var handleBits: UInt?
    private var pendingSecret: String?
    private var lastCredentialRequestID: UInt64?
    private let usesInjectedSnapshot: Bool
    private let vault: any CredentialVault

    public init(initialSnapshot: QueueSnapshot? = nil) {
        snapshot = initialSnapshot ?? .initial
        usesInjectedSnapshot = initialSnapshot != nil
        vault = KeychainCredentialVault()
    }

    init(initialSnapshot: QueueSnapshot? = nil, vault: any CredentialVault) {
        snapshot = initialSnapshot ?? .initial
        usesInjectedSnapshot = initialSnapshot != nil
        self.vault = vault
    }

    public func run() async {
        guard !usesInjectedSnapshot, !isRunning else { return }
        isRunning = true
        defer { isRunning = false }

        let bridge = SnapshotBridge()
        let context = Unmanaged.passRetained(bridge).toOpaque()
        guard let configuration = Self.configurationJSON(),
              let handle = configuration.withCString({ pointer in
                  startKernel(pointer, tts29SnapshotCallback, context)
              }) else {
            Unmanaged<SnapshotBridge>.fromOpaque(context).release()
            snapshot = .startupFailure("The Rust kernel refused startup.")
            return
        }
        handleBits = UInt(bitPattern: handle)
        restoreCredential(using: handle)

        for await value in bridge.snapshots {
            snapshot = value
            await executeCredentialRequest(value.credentialRequest)
            if value.identity.phase == .signedOut, value.credentialRequest == nil {
                pendingSecret = nil
            }
        }

        handleBits = nil
        let bits = UInt(bitPattern: handle)
        await Task.detached {
            stopKernel(UnsafeMutableRawPointer(bitPattern: bits))
        }.value
        Unmanaged<SnapshotBridge>.fromOpaque(context).release()
    }

    public func login(nsec: String) {
        guard let handle = handle else { return }
        pendingSecret = nsec
        nsec.withCString { loginKernel(handle, $0) }
    }

    public func logout() {
        dispatch(LogoutAction())
    }

    public func submitAnswer(itemID: String, answers: [QuestionAnswer]) {
        dispatch(SubmitAnswerAction(itemID: itemID, answers: answers))
    }

    private var handle: UnsafeMutableRawPointer? {
        handleBits.flatMap(UnsafeMutableRawPointer.init(bitPattern:))
    }

    private func restoreCredential(using handle: UnsafeMutableRawPointer) {
        do {
            guard let secret = try vault.load() else { return }
            secret.withCString { restoreKernelLogin(handle, $0) }
        } catch {
            error.localizedDescription.withCString {
                reportCredentialLoadFailure(handle, $0)
            }
        }
    }

    private func executeCredentialRequest(_ request: CredentialRequest?) async {
        guard let request, request.id != lastCredentialRequestID, let handle else { return }
        lastCredentialRequestID = request.id
        let vault = vault
        let result: VaultResult
        switch request.operation {
        case .store:
            guard let secret = pendingSecret else {
                report(request, result: .failure("The pending login was unavailable."), to: handle)
                return
            }
            pendingSecret = nil
            result = await Task.detached {
                do {
                    try vault.save(secret)
                    return VaultResult.success
                } catch {
                    return VaultResult.failure(error.localizedDescription)
                }
            }.value
        case .delete:
            result = await Task.detached {
                do {
                    try vault.delete()
                    return VaultResult.success
                } catch {
                    return VaultResult.failure(error.localizedDescription)
                }
            }.value
        }
        report(request, result: result, to: handle)
    }

    private func report(
        _ request: CredentialRequest,
        result: VaultResult,
        to handle: UnsafeMutableRawPointer
    ) {
        switch result {
        case .success:
            reportCredentialResult(handle, request.id, true, nil)
        case let .failure(error):
            error.withCString {
                reportCredentialResult(handle, request.id, false, $0)
            }
        }
    }

    private func dispatch<Action: Encodable>(_ action: Action) {
        guard let handle,
              let data = try? JSONEncoder().encode(action),
              let json = String(data: data, encoding: .utf8) else { return }
        json.withCString { dispatchKernel(handle, $0) }
    }

    private static func configurationJSON() -> String? {
        let directory = FileManager.default.urls(
            for: .applicationSupportDirectory,
            in: .userDomainMask
        ).first
        if let directory {
            try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        }
        guard let bootstrap = ConnectionSettings.current() else { return nil }
        let configuration = KernelConfiguration(
            relay: bootstrap.relay,
            groupID: bootstrap.groupID,
            storePath: directory?.appendingPathComponent("tts29.redb").path
        )
        guard let data = try? JSONEncoder().encode(configuration) else { return nil }
        return String(data: data, encoding: .utf8)
    }
}

private enum VaultResult: Sendable {
    case success
    case failure(String)
}

private struct LogoutAction: Encodable {
    let type = "logout"
}

private struct SubmitAnswerAction: Encodable {
    let type = "submit_answer"
    let itemID: String
    let answers: [QuestionAnswer]

    enum CodingKeys: String, CodingKey {
        case type, answers
        case itemID = "item_id"
    }
}

private struct KernelConfiguration: Encodable {
    let relay: String
    let groupID: String
    let storePath: String?

    enum CodingKeys: String, CodingKey {
        case relay
        case groupID = "group_id"
        case storePath = "store_path"
    }
}

private final class SnapshotBridge: @unchecked Sendable {
    let snapshots: AsyncStream<QueueSnapshot>
    private let continuation: AsyncStream<QueueSnapshot>.Continuation

    init() {
        (snapshots, continuation) = AsyncStream.makeStream()
    }

    func receive(_ bytes: UnsafePointer<CChar>) {
        let data = Data(bytes: bytes, count: strlen(bytes))
        guard let snapshot = try? JSONDecoder().decode(QueueSnapshot.self, from: data) else {
            continuation.yield(.startupFailure("The kernel returned an invalid snapshot."))
            continuation.finish()
            return
        }
        continuation.yield(snapshot)
        if snapshot.phase == .stopped { continuation.finish() }
    }
}

@_cdecl("tts29_snapshot_callback")
func tts29SnapshotCallback(
    _ bytes: UnsafePointer<CChar>?,
    _ context: UnsafeMutableRawPointer?
) {
    guard let bytes, let context else { return }
    Unmanaged<SnapshotBridge>.fromOpaque(context).takeUnretainedValue().receive(bytes)
}

private extension QueueSnapshot {
    static func startupFailure(_ message: String) -> QueueSnapshot {
        QueueSnapshot(
            phase: .failed,
            relay: "",
            groupID: "",
            items: [],
            evidence: QueueEvidence(sourceCount: 0, shortfallCount: 0),
            error: message
        )
    }
}
