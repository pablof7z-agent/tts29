import SwiftUI

struct AccountView: View {
    let identity: IdentitySnapshot
    let onLogin: (String) -> Void
    let onLogout: () -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var nsec = ""

    var body: some View {
        NavigationStack {
            Form {
                switch identity.phase {
                case .signedOut:
                    loginSection
                case .saving:
                    progress("Saving login securely…")
                case .signedIn:
                    signedInSection
                case .loggingOut:
                    progress("Removing saved login…")
                }
                if let error = identity.error {
                    Text(error)
                        .font(.footnote)
                        .foregroundStyle(.red)
                        .accessibilityIdentifier("tts29.account.error")
                }
            }
            .navigationTitle("Account")
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") { dismiss() }
                }
            }
        }
    }

    private var loginSection: some View {
        Section {
            SecureField("nsec1…", text: $nsec)
                .textContentType(.password)
                .autocorrectionDisabled()
                .accessibilityIdentifier("tts29.account.nsec")
            Button("Log In", systemImage: "key.fill") {
                let submitted = nsec.trimmingCharacters(in: .whitespacesAndNewlines)
                nsec = ""
                onLogin(submitted)
            }
            .disabled(nsec.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            .accessibilityIdentifier("tts29.account.login")
        } header: {
            Text("Nostr key")
        } footer: {
            Text("Your nsec stays in this device’s Keychain and signs only your actions.")
        }
    }

    private var signedInSection: some View {
        Section("Signed in") {
            LabeledContent("Public key", value: identity.shortPubkey ?? "Unknown")
            Button("Log Out", role: .destructive, action: onLogout)
                .accessibilityIdentifier("tts29.account.logout")
        }
    }

    private func progress(_ title: String) -> some View {
        HStack(spacing: 12) {
            ProgressView()
            Text(title)
        }
    }
}
