import Foundation

/// A branch with its last commit.
struct Branch: Identifiable, Hashable, Sendable {
    var id: String { name }
    let name: String
    let sha: String
    let headline: String
    let author: String
    let date: Date?
}

/// A commit of a branch's history, from the REST API.
struct Commit: Identifiable, Hashable, Sendable, Decodable {
    var id: String { sha }
    let sha: String
    let message: String
    let authorName: String
    let login: String?
    let date: Date?

    var headline: String { String(message.split(separator: "\n", maxSplits: 1).first ?? "") }
    /// The message after its first line.
    var body: String {
        let parts = message.split(separator: "\n", maxSplits: 1, omittingEmptySubsequences: false)
        return parts.count > 1 ? parts[1].trimmingCharacters(in: .whitespacesAndNewlines) : ""
    }
    var short: String { String(sha.prefix(7)) }

    init(sha: String, message: String, authorName: String, login: String?, date: Date?) {
        (self.sha, self.message, self.authorName, self.login, self.date) = (sha, message, authorName, login, date)
    }

    private struct Raw: Decodable {
        struct Inner: Decodable {
            struct Person: Decodable { let name: String?; let date: Date? }
            let message: String
            let author: Person?
        }
        struct User: Decodable { let login: String }
        let sha: String
        let commit: Inner
        let author: User?
    }

    init(from decoder: Decoder) throws {
        let r = try Raw(from: decoder)
        self.init(sha: r.sha, message: r.commit.message, authorName: r.commit.author?.name ?? r.author?.login ?? "Unknown", login: r.author?.login, date: r.commit.author?.date)
    }
}

/// What a commit changed.
struct ChangedFile: Hashable, Sendable, Decodable {
    let filename: String
    /// added, removed, modified, renamed, copied, changed or unchanged.
    let status: String
    let additions: Int
    let deletions: Int
}

struct CommitDetail: Sendable, Decodable {
    let files: [ChangedFile]

    enum CodingKeys: String, CodingKey { case files }

    init(files: [ChangedFile]) { self.files = files }

    init(from decoder: Decoder) throws {
        files = try decoder.container(keyedBy: CodingKeys.self).decodeIfPresent([ChangedFile].self, forKey: .files) ?? []
    }
}

enum HistoryQueries {
    /// The branches, the most recently committed first, with their last commit.
    static let branches = """
    query($owner:String!,$name:String!){repository(owner:$owner,name:$name){defaultBranchRef{name}
    refs(refPrefix:"refs/heads/",first:100,orderBy:{field:TAG_COMMIT_DATE,direction:DESC}){nodes{name
    target{... on Commit{oid messageHeadline committedDate author{name user{login}}}}}}}}
    """

    /// The default branch first, then the others as GitHub ordered them.
    static func parseBranches(_ data: [String: Any]) -> [Branch] {
        let repo = data["repository"] as? [String: Any] ?? [:]
        let main = (repo["defaultBranchRef"] as? [String: Any])?["name"] as? String
        let nodes = (repo["refs"] as? [String: Any])?["nodes"] as? [[String: Any]] ?? []
        let branches = nodes.compactMap { n -> Branch? in
            guard let name = n["name"] as? String, let t = n["target"] as? [String: Any], let sha = t["oid"] as? String else { return nil }
            let author = t["author"] as? [String: Any]
            let login = (author?["user"] as? [String: Any])?["login"] as? String
            return Branch(name: name, sha: sha, headline: t["messageHeadline"] as? String ?? "", author: login ?? author?["name"] as? String ?? "",
                          date: (t["committedDate"] as? String).flatMap { try? Date($0, strategy: .iso8601) })
        }
        return branches.filter { $0.name == main } + branches.filter { $0.name != main }
    }
}

extension GitHubClient {
    func branches(of repo: Repo) async throws -> [Branch] {
        HistoryQueries.parseBranches(try await graphql(HistoryQueries.branches, ["owner": repo.owner.login, "name": repo.name]))
    }

    /// One page of a branch's commits, newest first.
    func commits(of repo: Repo, branch: String, page: Int, perPage: Int = 30) async throws -> [Commit] {
        var c = URLComponents(string: "https://api.github.com/repos/\(repo.fullName)/commits")!
        c.queryItems = [.init(name: "sha", value: branch), .init(name: "per_page", value: "\(perPage)"), .init(name: "page", value: "\(page)")]
        return try Self.decoder.decode([Commit].self, from: try await data(c.url!))
    }

    func commitDetail(of repo: Repo, sha: String) async throws -> CommitDetail {
        try Self.decoder.decode(CommitDetail.self, from: try await data(URL(string: "https://api.github.com/repos/\(repo.fullName)/commits/\(sha)")!))
    }
}
