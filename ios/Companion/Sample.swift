import Foundation

/// Invented content for the simulator's screenshots (`-sample` at launch), like the Mac app's
/// sample project.
enum Sample {
    static let project = ProjectRef(owner: "acme", number: 1, title: "Sample roadmap", url: nil)

    static var pulls: [Pull] {
        func check(_ name: String, _ source: String, _ outcome: Check.Outcome, _ s: Int?) -> Check {
            Check(name: name, source: source, outcome: outcome, seconds: s, url: URL(string: "https://github.com/acme/sample-project/actions"))
        }
        func pull(_ n: Int, _ title: String, _ state: String, _ rollup: String?, _ checks: [Check], who: String = "camille", hours: Double, draft: Bool = false) -> Pull {
            Pull(number: n, title: title, url: URL(string: "https://github.com/acme/sample-project/pull/\(n)"), state: state, isDraft: draft, author: who,
                 branch: "feat/\(n)-sample", updatedAt: Date().addingTimeInterval(-hours * 3600), checksState: rollup, checks: checks)
        }
        return [
            pull(55, "Search commits by author and hash", "OPEN", "FAILURE", [check("lint", "CI", .failed, 41), check("test (macos-latest)", "CI", .failed, 192),
                 check("build (ubuntu-latest)", "CI", .passed, 125), check("typos", "Spelling", .passed, 9), check("Deploy preview", "Pages", .skipped, nil)], who: "sami", hours: 2, draft: true),
            pull(58, "Draw curved lines between lanes", "OPEN", "PENDING", [check("test (macos-latest)", "CI", .running, nil), check("lint", "CI", .passed, 38)], who: "ines", hours: 3),
            pull(57, "Keep branch names readable next to the badges", "OPEN", "SUCCESS", [check("lint", "CI", .passed, 40), check("test (macos-latest)", "CI", .passed, 201)], who: "lucas", hours: 20),
            pull(54, "Give every branch line its own colour", "MERGED", "SUCCESS", [check("test (macos-latest)", "CI", .passed, 188)], hours: 30),
            pull(50, "Try a darker sidebar", "CLOSED", nil, [], who: "lucas", hours: 90),
        ]
    }

    static var board: Board {
        func card(_ id: String, _ n: Int, _ title: String, _ status: String, type: String = "Feature", labels: [IssueLabel] = [], who: [String] = [], sub: (Int, Int) = (0, 0), kind: Card.Kind = .issue, state: String = "OPEN") -> Card {
            Card(id: id, kind: kind, number: n, title: title, body: "**As someone** with many repositories, I want to *group* them, so that the list stays short.",
                 url: URL(string: "https://github.com/acme/sample-project/issues/\(n)"), state: state, status: status, type: kind == .issue ? type : nil, labels: labels,
                 assignees: who, repository: "acme/sample-project",
                 subIssues: sub.1 > 0 ? (0..<sub.1).map { Linked(number: n + 1 + $0, title: "Part \($0 + 1) of \(title.lowercased())", state: $0 < sub.0 ? "CLOSED" : "OPEN", url: nil) } : [],
                 subDone: sub.0, subTotal: sub.1, parent: nil,
                 pullRequests: status == "In review" ? [Linked(number: n + 20, title: title, state: "OPEN", url: nil)] : [], updatedAt: Date().addingTimeInterval(-7200))
        }
        let ux = IssueLabel(name: "ux", color: "d876e3"), graph = IssueLabel(name: "zone:graphe", color: "0e8a16"), bug = IssueLabel(name: "bug", color: "d73a4a")
        return Board(projectID: "P1", statusFieldID: "F1", columns: [
            .init(id: "o1", name: "Backlog", color: "GRAY"), .init(id: "o2", name: "Ready", color: "YELLOW"),
            .init(id: "o3", name: "In progress", color: "BLUE"), .init(id: "o4", name: "In review", color: "PURPLE"), .init(id: "o5", name: "Done", color: "GREEN"),
        ], cards: [
            card("I1", 44, "Search commits by author", "Backlog", labels: [graph]),
            card("I2", 45, "Keyboard shortcut for stash", "Backlog", type: "Task"),
            card("I3", 52, "Show the linked issue in the toolbar", "Ready", labels: [ux], who: ["camille"]),
            card("I4", 51, "Group repositories in the switcher", "In progress", labels: [ux], who: ["camille"], sub: (2, 4)),
            card("I5", 46, "Crash when a stash message is empty", "In progress", type: "Bug", labels: [bug], who: ["lucas"]),
            card("I6", 48, "Draw curved lines between lanes", "In review", labels: [graph], who: ["ines"]),
            card("I7", 42, "Give every branch line its own colour", "Done", labels: [graph], state: "CLOSED"),
        ])
    }
}
