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

final class ProjectBoardTests: XCTestCase {
    func testProjectsLinkedFirstAndOnce() {
        let p = { (n: Int, title: String, closed: Bool) -> [String: Any] in ["number": n, "title": title, "url": "https://github.com/orgs/acme/projects/\(n)", "closed": closed, "owner": ["login": "acme"]] }
        let data: [String: Any] = ["repository": [
            "projectsV2": ["nodes": [p(1, "Roadmap", false), p(9, "Old", true)]],
            "issues": ["nodes": [["projectItems": ["nodes": [["project": p(2, "Bugs", false)], ["project": p(1, "Roadmap", false)]]]]]],
        ]]
        XCTAssertEqual(ProjectQueries.parseProjects(data).map(\.number), [1, 2])
    }

    func testItemsPageReadsColumnsCardsAndCursor() throws {
        let data: [String: Any] = ["repositoryOwner": ["projectV2": [
            "id": "PVT_1",
            "field": ["id": "F_1", "options": [["id": "o1", "name": "Todo", "color": "GRAY"], ["id": "o2", "name": "Done", "color": "GREEN"]]],
            "items": [
                "pageInfo": ["hasNextPage": true, "endCursor": "C2"],
                "nodes": [
                    ["id": "I1", "fieldValueByName": ["name": "Todo"], "content": [
                        "__typename": "Issue", "number": 5, "title": "Fix it", "url": "https://github.com/a/b/issues/5", "state": "OPEN", "body": "",
                        "issueType": ["name": "Bug"], "labels": ["nodes": [["name": "bug", "color": "d73a4a"]]], "assignees": ["nodes": [["login": "camille"]]],
                        "subIssuesSummary": ["total": 2, "completed": 1], "subIssues": ["nodes": [["number": 6, "title": "Part", "state": "CLOSED", "url": NSNull()]]],
                        "closedByPullRequestsReferences": ["nodes": [["number": 7, "title": "PR", "state": "OPEN", "url": "https://github.com/a/b/pull/7"]]],
                    ]],
                    ["id": "I2", "fieldValueByName": NSNull(), "content": ["__typename": "DraftIssue", "title": "Idea", "body": "x"]],
                ],
            ],
        ]]]
        let page = try XCTUnwrap(ProjectQueries.parseItems(data))
        XCTAssertEqual(page.projectID, "PVT_1")
        XCTAssertEqual(page.field, "F_1")
        XCTAssertEqual(page.columns.map(\.name), ["Todo", "Done"])
        XCTAssertEqual(page.next, "C2")
        let issue = page.cards[0]
        XCTAssertEqual(issue.kind, .issue)
        XCTAssertEqual(issue.status, "Todo")
        XCTAssertEqual(issue.type, "Bug")
        XCTAssertEqual(issue.labels.first?.name, "bug")
        XCTAssertEqual(issue.subDone, 1)
        XCTAssertEqual(issue.subIssues.first?.number, 6)
        XCTAssertEqual(issue.pullRequests.first?.number, 7)
        XCTAssertEqual(page.cards[1].kind, .draft)
        XCTAssertNil(page.cards[1].status)
    }

    func testBoardColumnsHoldTheirCards() {
        let board = Sample.board
        let ready = board.columns.first { $0.name == "Ready" }!
        XCTAssertEqual(board.cards(in: ready).map(\.number), [52])
    }
}

final class MoveTests: XCTestCase {
    func testMovingChangesTheColumnOnly() {
        let board = Sample.board
        let card = board.cards.first { $0.number == 52 }!
        let done = board.columns.first { $0.name == "Done" }!
        let moved = board.moving(card.id, to: done)
        XCTAssertEqual(moved.column(of: moved.cards.first { $0.id == card.id }!)?.name, "Done")
        XCTAssertEqual(moved.cards.count, board.cards.count)
        XCTAssertEqual(board.column(of: card)?.name, "Ready", "the original board is unchanged")
    }

    func testNoStatusColumnHoldsCardsWithout() {
        let none = Board.Column(id: nil, name: "No Status", color: "GRAY")
        let sample = Sample.board
        var board = Board(projectID: sample.projectID, statusFieldID: sample.statusFieldID, columns: [none] + sample.columns, cards: sample.cards)
        board.cards[0].status = nil
        XCTAssertEqual(board.column(of: board.cards[0]), none)
        XCTAssertEqual(board.cards(in: none).map(\.id), [board.cards[0].id])
    }
}

final class PullTests: XCTestCase {
    func testPullsWithChecksFailingFirst() throws {
        let commit: [String: Any] = ["statusCheckRollup": ["state": "FAILURE", "contexts": ["nodes": [
            ["__typename": "CheckRun", "name": "build", "status": "COMPLETED", "conclusion": "SUCCESS", "startedAt": "2026-11-06T10:00:00Z", "completedAt": "2026-11-06T10:02:05Z",
             "detailsUrl": "https://github.com/a/b/runs/1", "checkSuite": ["workflowRun": ["workflow": ["name": "CI"]], "app": ["name": "GitHub Actions"]]],
            ["__typename": "CheckRun", "name": "lint", "status": "COMPLETED", "conclusion": "FAILURE", "startedAt": "2026-11-06T10:00:00Z", "completedAt": "2026-11-06T10:00:41Z", "detailsUrl": NSNull()],
            ["__typename": "CheckRun", "name": "test", "status": "IN_PROGRESS", "conclusion": NSNull()],
            ["__typename": "StatusContext", "context": "ci/legacy", "state": "SUCCESS", "targetUrl": "https://ci.example.com/1"],
        ]]]]
        let pr: [String: Any] = ["number": 5, "title": "Fix", "url": "https://github.com/a/b/pull/5", "state": "OPEN", "isDraft": false, "headRefName": "fix/5",
                                 "updatedAt": "2026-11-06T10:03:00Z", "author": ["login": "camille"], "commits": ["nodes": [["commit": commit]]]]
        let data: [String: Any] = ["repository": ["open": ["nodes": [pr]], "done": ["nodes": []]]]
        let pulls = PullQueries.parse(data)
        let pull = try XCTUnwrap(pulls.open.first)
        XCTAssertEqual(pull.checksState, "FAILURE")
        XCTAssertEqual(pull.checks.map(\.name), ["lint", "test", "build", "ci/legacy"])
        XCTAssertEqual(pull.checks[2].seconds, 125)
        XCTAssertEqual(pull.checks[2].source, "CI")
        XCTAssertTrue(pulls.recent.isEmpty)
    }

    func testDurations() {
        XCTAssertEqual(PullQueries.duration(41), "41s")
        XCTAssertEqual(PullQueries.duration(192), "3m 12s")
        XCTAssertEqual(PullQueries.duration(3900), "1h 05m")
    }
}

final class HistoryTests: XCTestCase {
    func testBranchesDefaultFirst() {
        let ref = { (name: String, date: String) -> [String: Any] in
            ["name": name, "target": ["oid": "abc1234def", "messageHeadline": "Work on \(name)", "committedDate": date, "author": ["name": "Camille", "user": ["login": "camille"]]]]
        }
        let data: [String: Any] = ["repository": ["defaultBranchRef": ["name": "main"], "refs": ["nodes": [ref("feat/x", "2026-11-06T10:00:00Z"), ref("main", "2026-11-05T10:00:00Z")]]]]
        let branches = HistoryQueries.parseBranches(data)
        XCTAssertEqual(branches.map(\.name), ["main", "feat/x"])
        XCTAssertEqual(branches[0].author, "camille")
        XCTAssertNotNil(branches[0].date)
    }

    func testCommitsAndFilesDecode() throws {
        let json = #"[{"sha":"ca2a04c0123","commit":{"message":"Fix it\n\nBecause.","author":{"name":"Camille Laurent","date":"2026-11-06T10:00:00Z"}},"author":{"login":"camille"}},{"sha":"2bf28b1","commit":{"message":"Ghost","author":{"name":"Someone","date":"2026-11-05T10:00:00Z"}},"author":null}]"#
        let commits = try GitHubClient.decoder.decode([Commit].self, from: Data(json.utf8))
        XCTAssertEqual(commits[0].headline, "Fix it")
        XCTAssertEqual(commits[0].body, "Because.")
        XCTAssertEqual(commits[0].short, "ca2a04c")
        XCTAssertNil(commits[1].login)
        XCTAssertEqual(commits[1].authorName, "Someone")
        let detail = try GitHubClient.decoder.decode(CommitDetail.self, from: Data(#"{"files":[{"filename":"src/a.rs","status":"added","additions":3,"deletions":0}]}"#.utf8))
        XCTAssertEqual(detail.files.first?.status, "added")
    }
}
