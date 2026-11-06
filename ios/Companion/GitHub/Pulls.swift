import Foundation

/// A repository's pull requests with the CI of their last commit, as the Mac app reads them
/// (`app/src/pulls.rs`): every open one, then the most recently updated closed or merged.
struct Pull: Identifiable, Hashable, Sendable {
    var id: Int { number }
    let number: Int
    let title: String
    let url: URL?
    /// OPEN, CLOSED or MERGED.
    let state: String
    let isDraft: Bool
    let author: String
    let branch: String
    let updatedAt: Date?
    /// The rollup of its checks: SUCCESS, FAILURE, PENDING, ERROR, EXPECTED; nil without any.
    let checksState: String?
    let checks: [Check]
}

struct Check: Hashable, Sendable {
    enum Outcome: Int, Comparable, Sendable {
        // In the order they show: what failed first.
        case failed, running, passed, skipped

        static func < (a: Outcome, b: Outcome) -> Bool { a.rawValue < b.rawValue }
    }

    let name: String
    /// The workflow or app that ran it.
    let source: String?
    let outcome: Outcome
    let seconds: Int?
    let url: URL?
}

enum PullQueries {
    static let pulls = """
    fragment C on Commit{statusCheckRollup{state contexts(first:60){nodes{__typename
    ... on CheckRun{name status conclusion startedAt completedAt detailsUrl checkSuite{workflowRun{workflow{name}} app{name}}}
    ... on StatusContext{context state targetUrl}}}}}
    fragment R on PullRequest{number title url state isDraft headRefName updatedAt author{login} commits(last:1){nodes{commit{...C}}}}
    query($owner:String!,$name:String!){repository(owner:$owner,name:$name){
    open:pullRequests(states:OPEN,first:50,orderBy:{field:UPDATED_AT,direction:DESC}){nodes{...R}}
    done:pullRequests(states:[CLOSED,MERGED],first:20,orderBy:{field:UPDATED_AT,direction:DESC}){nodes{...R}}}}
    """

    static func parse(_ data: [String: Any]) -> (open: [Pull], recent: [Pull]) {
        let repo = data["repository"] as? [String: Any] ?? [:]
        let list = { (key: String) in ((repo[key] as? [String: Any])?["nodes"] as? [[String: Any]] ?? []).compactMap(pull) }
        return (list("open"), list("done"))
    }

    private static func pull(_ n: [String: Any]) -> Pull? {
        guard let number = n["number"] as? Int else { return nil }
        let commit = ((((n["commits"] as? [String: Any])?["nodes"] as? [[String: Any]])?.first)?["commit"] as? [String: Any])
        let rollup = commit?["statusCheckRollup"] as? [String: Any]
        let contexts = (rollup?["contexts"] as? [String: Any])?["nodes"] as? [[String: Any]] ?? []
        return Pull(
            number: number, title: n["title"] as? String ?? "", url: (n["url"] as? String).flatMap(URL.init),
            state: n["state"] as? String ?? "", isDraft: n["isDraft"] as? Bool ?? false,
            author: (n["author"] as? [String: Any])?["login"] as? String ?? "ghost", branch: n["headRefName"] as? String ?? "",
            updatedAt: date(n["updatedAt"]), checksState: rollup?["state"] as? String,
            checks: contexts.compactMap(check).sorted { ($0.outcome, $0.name) < ($1.outcome, $1.name) }
        )
    }

    private static func check(_ c: [String: Any]) -> Check? {
        if c["__typename"] as? String == "StatusContext" {
            guard let name = c["context"] as? String else { return nil }
            let outcome: Check.Outcome = switch c["state"] as? String {
            case "SUCCESS": .passed
            case "PENDING", "EXPECTED": .running
            default: .failed
            }
            return Check(name: name, source: nil, outcome: outcome, seconds: nil, url: (c["targetUrl"] as? String).flatMap(URL.init))
        }
        guard let name = c["name"] as? String else { return nil }
        let suite = c["checkSuite"] as? [String: Any]
        let workflow = (((suite?["workflowRun"] as? [String: Any])?["workflow"]) as? [String: Any])?["name"] as? String
        let app = (suite?["app"] as? [String: Any])?["name"] as? String
        let outcome: Check.Outcome = switch (c["status"] as? String, c["conclusion"] as? String) {
        case (let status, _) where status != "COMPLETED": .running
        case (_, "SUCCESS"): .passed
        case (_, "SKIPPED"), (_, "NEUTRAL"), (_, "STALE"): .skipped
        default: .failed
        }
        var seconds: Int?
        if let start = date(c["startedAt"]), let end = date(c["completedAt"]) {
            seconds = max(0, Int(end.timeIntervalSince(start)))
        }
        return Check(name: name, source: workflow ?? app, outcome: outcome, seconds: seconds, url: (c["detailsUrl"] as? String).flatMap(URL.init))
    }

    private static func date(_ v: Any?) -> Date? {
        (v as? String).flatMap { try? Date($0, strategy: .iso8601) }
    }

    /// "41s", "3m 12s", "1h 05m".
    static func duration(_ seconds: Int) -> String {
        switch seconds {
        case ..<60: "\(seconds)s"
        case ..<3600: "\(seconds / 60)m \(String(format: "%02d", seconds % 60))s"
        default: "\(seconds / 3600)h \(String(format: "%02d", seconds % 3600 / 60))m"
        }
    }
}

extension GitHubClient {
    func pulls(of repo: Repo) async throws -> (open: [Pull], recent: [Pull]) {
        PullQueries.parse(try await graphql(PullQueries.pulls, ["owner": repo.owner.login, "name": repo.name]))
    }
}
