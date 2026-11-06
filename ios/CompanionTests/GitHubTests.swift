import XCTest
@testable import Fourchette

final class GitHubTests: XCTestCase {
    func testDeviceCodeDecodes() throws {
        let json = #"{"device_code":"3584d83530557fdd1f46af8289938c8ef79f9dc5","user_code":"WDJB-MJHT","verification_uri":"https://github.com/login/device","expires_in":900,"interval":5}"#
        let code = try JSONDecoder().decode(DeviceFlow.Code.self, from: Data(json.utf8))
        XCTAssertEqual(code.userCode, "WDJB-MJHT")
        XCTAssertEqual(code.verificationURI.absoluteString, "https://github.com/login/device")
        XCTAssertEqual(code.interval, 5)
    }

    func testPollAnswers() {
        let poll = { (s: String) in DeviceFlow.parsePoll(Data(s.utf8)) }
        XCTAssertEqual(poll(#"{"access_token":"gho_abc","token_type":"bearer","scope":"repo"}"#), .token("gho_abc"))
        XCTAssertEqual(poll(#"{"error":"authorization_pending"}"#), .pending)
        XCTAssertEqual(poll(#"{"error":"slow_down","interval":10}"#), .slowDown(10))
        if case .failed = poll(#"{"error":"expired_token"}"#) {} else { XCTFail("expired") }
        if case .failed = poll(#"{"error":"access_denied"}"#) {} else { XCTFail("denied") }
        if case .failed = poll("not json") {} else { XCTFail("garbage") }
    }

    func testNextPageFromLinkHeader() {
        let link = #"<https://api.github.com/user/repos?page=2>; rel="next", <https://api.github.com/user/repos?page=5>; rel="last""#
        XCTAssertEqual(GitHubClient.nextPage(link)?.absoluteString, "https://api.github.com/user/repos?page=2")
        XCTAssertNil(GitHubClient.nextPage(#"<https://api.github.com/user/repos?page=1>; rel="prev""#))
        XCTAssertNil(GitHubClient.nextPage(nil))
    }

    func testRepoDecodesAndMatches() throws {
        let json = #"[{"id":1,"name":"fourchette","full_name":"C2-POLY/fourchette","owner":{"login":"C2-POLY","avatar_url":null},"description":null,"private":false,"default_branch":"main","html_url":"https://github.com/C2-POLY/fourchette","pushed_at":"2026-11-06T09:21:13Z","stargazers_count":3,"open_issues_count":7}]"#
        let repo = try GitHubClient.decoder.decode([Repo].self, from: Data(json.utf8))[0]
        XCTAssertEqual(repo.owner.login, "C2-POLY")
        XCTAssertNotNil(repo.pushedAt)
        XCTAssertTrue(repo.matches("c2-poly/FOUR"))
        XCTAssertTrue(repo.matches("  "))
        XCTAssertFalse(repo.matches("warhallow"))
    }

    @MainActor
    func testGroupsPutMineFirst() {
        let session = Session(sample: true)
        let owners = session.grouped(matching: "").map(\.owner)
        XCTAssertEqual(owners, ["camille", "acme", "lab"])
        XCTAssertEqual(session.grouped(matching: "site").flatMap(\.repos).map(\.name), ["sample-site"])
    }
}
