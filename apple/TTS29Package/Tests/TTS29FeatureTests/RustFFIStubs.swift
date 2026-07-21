import Foundation

typealias SnapshotCallback = @convention(c) (
    UnsafePointer<CChar>?,
    UnsafeMutableRawPointer?
) -> Void

@_cdecl("tts29_start")
func testStartKernel(
    _ configuration: UnsafePointer<CChar>,
    _ callback: SnapshotCallback?,
    _ context: UnsafeMutableRawPointer?
) -> UnsafeMutableRawPointer? {
    nil
}

@_cdecl("tts29_stop")
func testStopKernel(_ handle: UnsafeMutableRawPointer?) {}

@_cdecl("tts29_login")
func testLoginKernel(_ handle: UnsafeMutableRawPointer?, _ secret: UnsafePointer<CChar>) {}

@_cdecl("tts29_restore_login")
func testRestoreKernelLogin(_ handle: UnsafeMutableRawPointer?, _ secret: UnsafePointer<CChar>) {}

@_cdecl("tts29_credential_load_failed")
func testCredentialLoadFailure(
    _ handle: UnsafeMutableRawPointer?,
    _ error: UnsafePointer<CChar>
) {}

@_cdecl("tts29_dispatch")
func testDispatchKernel(_ handle: UnsafeMutableRawPointer?, _ action: UnsafePointer<CChar>) {}

@_cdecl("tts29_credential_result")
func testCredentialResult(
    _ handle: UnsafeMutableRawPointer?,
    _ requestID: UInt64,
    _ succeeded: Bool,
    _ error: UnsafePointer<CChar>?
) {}
