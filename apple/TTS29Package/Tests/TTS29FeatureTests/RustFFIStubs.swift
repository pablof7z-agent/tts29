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
