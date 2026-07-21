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
        let menu = app.buttons["tts29.menu"]
        XCTAssertTrue(menu.waitForExistence(timeout: 5))
    }

    @MainActor
    func testConnectionSettingsOpenWithStandaloneDefaults() throws {
        let app = XCUIApplication()
        app.launch()

        let menu = app.buttons["tts29.menu"]
        XCTAssertTrue(menu.waitForExistence(timeout: 5))
        menu.tap()

        let connection = app.buttons["Connection…"]
        XCTAssertTrue(connection.waitForExistence(timeout: 5))
        connection.tap()

        XCTAssertTrue(app.navigationBars["Connection"].waitForExistence(timeout: 5))
        let relay = app.textFields["tts29.connection.relay"]
        let group = app.textFields["tts29.connection.group"]
        XCTAssertTrue(relay.waitForExistence(timeout: 5))
        XCTAssertTrue(group.waitForExistence(timeout: 5))
        XCTAssertFalse((relay.value as? String ?? "").isEmpty)
        XCTAssertFalse((group.value as? String ?? "").isEmpty)

        let save = app.buttons["tts29.connection.save"]
        XCTAssertTrue(save.isEnabled)
        save.tap()
        XCTAssertFalse(app.navigationBars["Connection"].waitForExistence(timeout: 2))
    }

    @MainActor
    func testNsecLoginAndLogout() throws {
        let app = XCUIApplication()
        app.launch()

        let menu = app.buttons["tts29.menu"]
        XCTAssertTrue(menu.waitForExistence(timeout: 5))
        menu.tap()
        let account = app.buttons["Log In…"]
        XCTAssertTrue(account.waitForExistence(timeout: 5))
        account.tap()

        let field = app.secureTextFields["tts29.account.nsec"]
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        field.tap()
        field.typeText(Self.disposableTestNsec)
        let login = app.buttons["tts29.account.login"]
        XCTAssertTrue(login.isEnabled)
        login.tap()

        let logout = app.buttons["tts29.account.logout"]
        XCTAssertTrue(logout.waitForExistence(timeout: 5))
        logout.tap()
        XCTAssertTrue(field.waitForExistence(timeout: 5))
    }

    @MainActor
    func testProjectedAudioCanStartInTheSimulator() throws {
        let app = XCUIApplication()
        app.launchEnvironment["TTS29_UI_AUDIO_BASE64"] = Self.waveFixture.base64EncodedString()
        app.launch()

        let play = app.buttons["tts29.play.ui-fixture"]
        XCTAssertTrue(play.waitForExistence(timeout: 5))
        play.tap()

        let status = app.staticTexts["tts29.playback.status"]
        XCTAssertTrue(status.waitForExistence(timeout: 5))
        let playbackStarted = XCTNSPredicateExpectation(
            predicate: NSPredicate(format: "label == %@ OR label == %@", "Playing", "Finished"),
            object: status
        )
        XCTAssertEqual(
            XCTWaiter.wait(for: [playbackStarted], timeout: 5),
            .completed,
            "playback did not start; final state: \(status.label)"
        )
    }

    private static var waveFixture: Data {
        let sampleRate: UInt32 = 8_000
        let sampleCount: UInt32 = 800
        let dataSize = sampleCount * 2
        var bytes = Data()
        bytes.append(contentsOf: Array("RIFF".utf8))
        bytes.appendLittleEndian(36 + dataSize)
        bytes.append(contentsOf: Array("WAVEfmt ".utf8))
        bytes.appendLittleEndian(UInt32(16))
        bytes.appendLittleEndian(UInt16(1))
        bytes.appendLittleEndian(UInt16(1))
        bytes.appendLittleEndian(sampleRate)
        bytes.appendLittleEndian(sampleRate * 2)
        bytes.appendLittleEndian(UInt16(2))
        bytes.appendLittleEndian(UInt16(16))
        bytes.append(contentsOf: Array("data".utf8))
        bytes.appendLittleEndian(dataSize)
        bytes.append(Data(count: Int(dataSize)))
        return bytes
    }

    // Public, deterministic test-only key. It is used only in the simulator
    // Keychain and never publishes an event.
    private static let disposableTestNsec =
        "nsec1qyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqstywftw"
}

private extension Data {
    mutating func appendLittleEndian<T: FixedWidthInteger>(_ value: T) {
        var value = value.littleEndian
        Swift.withUnsafeBytes(of: &value) { append(contentsOf: $0) }
    }
}
