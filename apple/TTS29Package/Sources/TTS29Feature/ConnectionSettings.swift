import Foundation
import SwiftUI

struct ConnectionSettings: Equatable {
    static let relayKey = "tts29.relay"
    static let groupKey = "tts29.group"

    let relay: String
    let groupID: String

    static func current(defaults: UserDefaults = .standard) -> ConnectionSettings? {
        guard let resource = Bundle.module.url(
            forResource: "Bootstrap",
            withExtension: "json"
        ),
        let data = try? Data(contentsOf: resource),
        let fallback = try? JSONDecoder().decode(BootstrapConfiguration.self, from: data) else {
            return nil
        }
        return resolve(
            defaults: defaults,
            fallback: ConnectionSettings(
                relay: fallback.relay,
                groupID: fallback.groupID
            )
        )
    }

    static func resolve(
        defaults: UserDefaults,
        fallback: ConnectionSettings
    ) -> ConnectionSettings {
        let relay = nonempty(defaults.string(forKey: relayKey)) ?? fallback.relay
        let group = nonempty(defaults.string(forKey: groupKey)) ?? fallback.groupID
        return ConnectionSettings(relay: relay, groupID: group)
    }

    func save(defaults: UserDefaults = .standard) {
        defaults.set(relay, forKey: Self.relayKey)
        defaults.set(groupID, forKey: Self.groupKey)
    }

    private static func nonempty(_ value: String?) -> String? {
        let trimmed = value?.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed?.isEmpty == false ? trimmed : nil
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

struct ConnectionSettingsView: View {
    @Environment(\.dismiss) private var dismiss
    @State private var relay: String
    @State private var groupID: String

    init() {
        let current = ConnectionSettings.current()
            ?? ConnectionSettings(relay: "", groupID: "")
        _relay = State(initialValue: current.relay)
        _groupID = State(initialValue: current.groupID)
    }

    var body: some View {
        NavigationStack {
            Form {
                TextField("NIP-29 relay", text: $relay)
                    .accessibilityIdentifier("tts29.connection.relay")
                TextField("Group ID", text: $groupID)
                    .accessibilityIdentifier("tts29.connection.group")
                Text("Changes apply the next time TTS29 launches.")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
            }
            .navigationTitle("Connection")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") {
                        ConnectionSettings(relay: relay, groupID: groupID).save()
                        dismiss()
                    }
                    .accessibilityIdentifier("tts29.connection.save")
                    .disabled(relay.trimmed.isEmpty || groupID.trimmed.isEmpty)
                }
            }
        }
    }
}

private extension String {
    var trimmed: String {
        trimmingCharacters(in: .whitespacesAndNewlines)
    }
}
