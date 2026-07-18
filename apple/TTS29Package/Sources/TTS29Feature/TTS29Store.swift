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

@_silgen_name("tts29_stop")
private func stopKernel(_ handle: UnsafeMutableRawPointer?)

@Observable
@MainActor
public final class TTS29Store {
    public private(set) var snapshot = QueueSnapshot.initial
    private var isRunning = false

    public init() {}

    public func run() async {
        guard !isRunning else { return }
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

        for await value in bridge.snapshots {
            snapshot = value
        }

        let handleBits = UInt(bitPattern: handle)
        await Task.detached {
            stopKernel(UnsafeMutableRawPointer(bitPattern: handleBits))
        }.value
        Unmanaged<SnapshotBridge>.fromOpaque(context).release()
    }

    private static func configurationJSON() -> String? {
        let directory = FileManager.default.urls(
            for: .applicationSupportDirectory,
            in: .userDomainMask
        ).first
        if let directory {
            try? FileManager.default.createDirectory(
                at: directory,
                withIntermediateDirectories: true
            )
        }
        guard let resource = Bundle.module.url(
            forResource: "Bootstrap",
            withExtension: "json"
        ),
        let data = try? Data(contentsOf: resource),
        let bootstrap = try? JSONDecoder().decode(BootstrapConfiguration.self, from: data) else {
            return nil
        }
        let configuration = KernelConfiguration(
            relay: bootstrap.relay,
            groupID: bootstrap.groupID,
            storePath: directory?.appendingPathComponent("tts29.redb").path
        )
        guard let data = try? JSONEncoder().encode(configuration) else { return nil }
        return String(data: data, encoding: .utf8)
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

private struct BootstrapConfiguration: Decodable {
    let relay: String
    let groupID: String

    enum CodingKeys: String, CodingKey {
        case relay
        case groupID = "group_id"
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
        if snapshot.phase == .stopped {
            continuation.finish()
        }
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
