import Foundation

/// A GitHub Project of a repository: its Status columns and its items, in the project's order,
/// as the Mac app's board reads them (`app/src/github.rs`).
struct ProjectRef: Hashable, Sendable {
    let owner: String
    let number: Int
    let title: String
    let url: URL?
}

struct Linked: Hashable, Sendable {
    let number: Int
    let title: String
    let state: String
    let url: URL?
}

struct IssueLabel: Hashable, Sendable {
    let name: String
    /// Six hex digits, as GitHub gives a label's colour.
    let color: String
}

struct Card: Identifiable, Hashable, Sendable {
    enum Kind: String, Sendable { case issue, pullRequest, draft }

    /// The project item's id (what a move writes), not the issue's.
    let id: String
    let kind: Kind
    let number: Int?
    let title: String
    let body: String
    let url: URL?
    /// OPEN, CLOSED or MERGED; empty for a draft.
    let state: String
    let status: String?
    let type: String?
    let labels: [IssueLabel]
    let assignees: [String]
    let repository: String?
    let subIssues: [Linked]
    let subDone: Int
    let subTotal: Int
    let parent: Linked?
    let pullRequests: [Linked]
    let updatedAt: Date?
}

struct Board: Sendable {
    struct Column: Hashable, Sendable {
        /// The option's id, nil for the cards without a status.
        let id: String?
        let name: String
        /// GitHub's colour name of the option (GRAY, BLUE, GREEN…).
        let color: String
    }

    /// The project's node id and its Status field's, for moves.
    let projectID: String
    let statusFieldID: String?
    let columns: [Column]
    let cards: [Card]

    /// The cards of a column, in the project's order.
    func cards(in column: Column) -> [Card] {
        cards.filter { $0.status == (column.id == nil ? nil : column.name) }
    }
}

enum ProjectQueries {
    /// Projects linked to the repository, and those its recent issues are in: many projects hold
    /// a repository's issues without being linked to it.
    static let projects = """
    fragment P on ProjectV2{number title url closed owner{... on Organization{login} ... on User{login}}}
    query($owner:String!,$name:String!){repository(owner:$owner,name:$name){
    projectsV2(first:20){nodes{...P}}
    issues(first:50,orderBy:{field:UPDATED_AT,direction:DESC}){nodes{projectItems(first:5){nodes{project{...P}}}}}}}
    """

    static let items = """
    fragment P on ProjectV2{id field(name:"Status"){... on ProjectV2SingleSelectField{id options{id name color}}}
    items(first:100,after:$cursor,orderBy:{field:POSITION,direction:ASC}){pageInfo{hasNextPage endCursor} nodes{id
    fieldValueByName(name:"Status"){... on ProjectV2ItemFieldSingleSelectValue{name}}
    content{__typename
    ... on DraftIssue{title body updatedAt assignees(first:10){nodes{login}}}
    ... on Issue{number title url state body updatedAt issueType{name} repository{nameWithOwner}
    labels(first:20){nodes{name color}} assignees(first:10){nodes{login}} subIssuesSummary{total completed}
    subIssues(first:30){nodes{number title state url}} parent{number title url state}
    closedByPullRequestsReferences(first:5){nodes{number title state url}}}
    ... on PullRequest{number title url state body updatedAt repository{nameWithOwner}
    labels(first:20){nodes{name color}} assignees(first:10){nodes{login}}}}}}}
    query($owner:String!,$number:Int!,$cursor:String){repositoryOwner(login:$owner){
    ... on Organization{projectV2(number:$number){...P}} ... on User{projectV2(number:$number){...P}}}}
    """

    /// The open projects, linked ones first, each once.
    static func parseProjects(_ data: [String: Any]) -> [ProjectRef] {
        let repo = data["repository"] as? [String: Any] ?? [:]
        var nodes = (repo["projectsV2"] as? [String: Any])?["nodes"] as? [[String: Any]] ?? []
        for issue in (repo["issues"] as? [String: Any])?["nodes"] as? [[String: Any]] ?? [] {
            let items = (issue["projectItems"] as? [String: Any])?["nodes"] as? [[String: Any]] ?? []
            nodes += items.compactMap { $0["project"] as? [String: Any] }
        }
        var out: [ProjectRef] = []
        for n in nodes where n["closed"] as? Bool != true {
            guard let owner = (n["owner"] as? [String: Any])?["login"] as? String, let number = n["number"] as? Int else { continue }
            if !out.contains(where: { $0.owner == owner && $0.number == number }) {
                out.append(ProjectRef(owner: owner, number: number, title: n["title"] as? String ?? "", url: (n["url"] as? String).flatMap(URL.init)))
            }
        }
        return out
    }

    /// One page of items: the project's ids and columns, its cards, and the next page's cursor.
    static func parseItems(_ data: [String: Any]) -> (projectID: String, field: String?, columns: [Board.Column], cards: [Card], next: String?)? {
        guard let project = (data["repositoryOwner"] as? [String: Any])?["projectV2"] as? [String: Any], let id = project["id"] as? String else { return nil }
        let field = project["field"] as? [String: Any]
        let options = field?["options"] as? [[String: Any]] ?? []
        let columns = options.compactMap { o -> Board.Column? in
            guard let name = o["name"] as? String else { return nil }
            return Board.Column(id: o["id"] as? String, name: name, color: o["color"] as? String ?? "GRAY")
        }
        let items = project["items"] as? [String: Any] ?? [:]
        let page = items["pageInfo"] as? [String: Any]
        let next = page?["hasNextPage"] as? Bool == true ? page?["endCursor"] as? String : nil
        let cards = (items["nodes"] as? [[String: Any]] ?? []).compactMap(card)
        return (id, field?["id"] as? String, columns, cards, next)
    }

    private static func card(_ n: [String: Any]) -> Card? {
        guard let id = n["id"] as? String, let c = n["content"] as? [String: Any], let title = c["title"] as? String else { return nil }
        let kind: Card.Kind = switch c["__typename"] as? String {
        case "PullRequest": .pullRequest
        case "DraftIssue": .draft
        default: .issue
        }
        let summary = c["subIssuesSummary"] as? [String: Any]
        return Card(
            id: id, kind: kind, number: c["number"] as? Int, title: title, body: c["body"] as? String ?? "",
            url: (c["url"] as? String).flatMap(URL.init), state: c["state"] as? String ?? "",
            status: (n["fieldValueByName"] as? [String: Any])?["name"] as? String,
            type: (c["issueType"] as? [String: Any])?["name"] as? String,
            labels: nodes(c["labels"]).compactMap { l in (l["name"] as? String).map { IssueLabel(name: $0, color: l["color"] as? String ?? "888888") } },
            assignees: nodes(c["assignees"]).compactMap { $0["login"] as? String },
            repository: (c["repository"] as? [String: Any])?["nameWithOwner"] as? String,
            subIssues: nodes(c["subIssues"]).compactMap(linked),
            subDone: summary?["completed"] as? Int ?? 0, subTotal: summary?["total"] as? Int ?? 0,
            parent: (c["parent"] as? [String: Any]).flatMap(linked),
            pullRequests: nodes(c["closedByPullRequestsReferences"]).compactMap(linked),
            updatedAt: (c["updatedAt"] as? String).flatMap { try? Date($0, strategy: .iso8601) }
        )
    }

    private static func nodes(_ v: Any?) -> [[String: Any]] {
        (v as? [String: Any])?["nodes"] as? [[String: Any]] ?? []
    }

    private static func linked(_ v: [String: Any]) -> Linked? {
        guard let number = v["number"] as? Int else { return nil }
        return Linked(number: number, title: v["title"] as? String ?? "", state: v["state"] as? String ?? "", url: (v["url"] as? String).flatMap(URL.init))
    }
}

extension GitHubClient {
    func projects(of repo: Repo) async throws -> [ProjectRef] {
        ProjectQueries.parseProjects(try await graphql(ProjectQueries.projects, ["owner": repo.owner.login, "name": repo.name]))
    }

    /// The whole board, up to `limit` items.
    func board(_ project: ProjectRef, limit: Int = 500) async throws -> Board {
        var cursor: String?
        var cards: [Card] = []
        var head: (String, String?, [Board.Column])?
        repeat {
            var vars: [String: Any] = ["owner": project.owner, "number": project.number]
            if let cursor { vars["cursor"] = cursor }
            guard let page = ProjectQueries.parseItems(try await graphql(ProjectQueries.items, vars)) else {
                throw Failure.status(404, "The project \(project.title) can't be read.")
            }
            head = head ?? (page.projectID, page.field, page.columns)
            cards += page.cards
            cursor = page.next
        } while cursor != nil && cards.count < limit
        let (id, field, options) = head!
        // The cards without a status, in a column of their own when there are any.
        let none = cards.contains { $0.status == nil } ? [Board.Column(id: nil, name: "No Status", color: "GRAY")] : []
        return Board(projectID: id, statusFieldID: field, columns: none + options, cards: Array(cards.prefix(limit)))
    }
}
