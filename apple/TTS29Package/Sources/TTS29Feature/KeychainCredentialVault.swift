import Foundation
import Security

protocol CredentialVault: Sendable {
    func load() throws -> String?
    func save(_ secret: String) throws
    func delete() throws
}

struct KeychainCredentialVault: CredentialVault {
    private let service = "io.f7z.tts29.user-nsec"
    private let account = "active"

    func load() throws -> String? {
        var query = baseQuery
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne
        var result: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        if status == errSecItemNotFound { return nil }
        guard status == errSecSuccess else { throw KeychainVaultError(status) }
        guard let data = result as? Data, let secret = String(data: data, encoding: .utf8) else {
            throw KeychainVaultError(errSecDecode)
        }
        return secret
    }

    func save(_ secret: String) throws {
        let data = Data(secret.utf8)
        let updateStatus = SecItemUpdate(
            baseQuery as CFDictionary,
            [kSecValueData as String: data] as CFDictionary
        )
        if updateStatus == errSecSuccess { return }
        guard updateStatus == errSecItemNotFound else { throw KeychainVaultError(updateStatus) }
        var item = baseQuery
        item[kSecValueData as String] = data
        item[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
        let addStatus = SecItemAdd(item as CFDictionary, nil)
        guard addStatus == errSecSuccess else { throw KeychainVaultError(addStatus) }
    }

    func delete() throws {
        let status = SecItemDelete(baseQuery as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw KeychainVaultError(status)
        }
    }

    private var baseQuery: [String: Any] {
        [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
        ]
    }
}

private struct KeychainVaultError: LocalizedError {
    let status: OSStatus

    init(_ status: OSStatus) {
        self.status = status
    }

    var errorDescription: String? {
        SecCopyErrorMessageString(status, nil) as String?
            ?? "Keychain error \(status)."
    }
}
