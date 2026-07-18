import XCTest

final class TTS29UITests: XCTestCase {
    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    @MainActor
    func testQueueShellLaunches() throws {
        let app = XCUIApplication()
        app.launch()

        XCTAssertTrue(app.navigationBars["TTS29"].waitForExistence(timeout: 5))
        let status = app.descendants(matching: .any)["tts29.status"]
        XCTAssertTrue(status.waitForExistence(timeout: 5))
    }
}
