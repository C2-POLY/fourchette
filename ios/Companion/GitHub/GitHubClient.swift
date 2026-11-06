import Foundation

/// A repository of the list, as the REST API gives it.
struct Repo: Decodable, Identifiable, Hashable, Sendable {
    let id: Int
    let name: String
    let fullName: String
    let owner: Owner
    let description: String?
    let isPrivate: Bool
    let defaultBranch: String
    let htmlURL: URL
    let pushedAt: Date?
    let stargazersCount: Int
    let openIssuesCount: Int

    struct Owner: Decodable, Hashable, Sendable {
        let login: String
        let avatarURL: URL?

        enum CodingKeys: String, CodingKey {
            case login, avatarURL = "avatar_url"
        }
    }

    enum CodingKeys: String, CodingKey {
        case id, name, owner, description
        case fullName = "full_name", isPrivate = "private", defaultBranch = "default_branch", htmlURL = "html_url"
        case pushedAt = "pushed_at", stargazersCount = "stargazers_count", openIssuesCount = "open_issues_count"
    }

    /// The search of the list: the owner or the name, ignoring case and accents.
    func matches(_ query: String) -> Bool {
        let q = query.trimmingCharacters(in: .whitespaces)
        return q.isEmpty || fullName.range(of: q, options: [.caseInsensitive, .diacriticInsensitive]) != nil
    }
}

struct User: Decodable, Sendable {
    let login: String
    let name: String?
    let avatarURL: URL?

    enum CodingKeys: String, CodingKey {
        case login, name, avatarURL = "avatar_url"
    }
}

/// The GitHub REST API with the signed-in person's token.
struct GitHubClient: Sendable {
    let token: String

    enum Failure: LocalizedError {
        /// The token was revoked or has expired: the app signs out.
        case unauthorized
        case status(Int, String)

        var errorDescription: String? {
            switch self {
            case .unauthorized: "GitHub no longer accepts this sign-in. Sign in again."
            case .status(let code, let message): "GitHub answered \(code): \(message)"
            }
        }
    }

    static let decoder: JSONDecoder = {
        let d = JSONDecoder()
        d.dateDecodingStrategy = .iso8601
        return d
    }()

    func user() async throws -> User {
        let (data, _) = try await get(URL(string: "https://api.github.com/user")!)
        return try Self.decoder.decode(User.self, from: data)
    }

    /// My repositories and those of my organisations, the most recently pushed first, up to
    /// `pages` pages of 100.
    func repos(pages: Int = 5) async throws -> [Repo] {
        var next = URL(string: "https://api.github.com/user/repos?per_page=100&sort=pushed&affiliation=owner,collaborator,organization_member")
        var all: [Repo] = []
        var page = 0
        while let url = next, page < pages {
            let (data, response) = try await get(url)
            all += try Self.decoder.decode([Repo].self, from: data)
            next = Self.nextPage(response.value(forHTTPHeaderField: "Link"))
            page += 1
        }
        return all
    }

    /// The `rel="next"` URL of a `Link` header.
    static func nextPage(_ link: String?) -> URL? {
        guard let link else { return nil }
        for part in link.split(separator: ",") {
            let pieces = part.split(separator: ";").map { $0.trimmingCharacters(in: .whitespaces) }
            if pieces.count >= 2, pieces.dropFirst().contains("rel=\"next\""), pieces[0].hasPrefix("<"), pieces[0].hasSuffix(">") {
                return URL(string: String(pieces[0].dropFirst().dropLast()))
            }
        }
        return nil
    }

    private func get(_ url: URL) async throws -> (Data, HTTPURLResponse) {
        var request = URLRequest(url: url)
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        request.setValue("application/vnd.github+json", forHTTPHeaderField: "Accept")
        request.setValue("2022-11-28", forHTTPHeaderField: "X-GitHub-Api-Version")
        let (data, response) = try await URLSession.shared.data(for: request)
        let http = response as! HTTPURLResponse
        switch http.statusCode {
        case 200..<300: return (data, http)
        case 401: throw Failure.unauthorized
        default:
            let message = (try? JSONSerialization.jsonObject(with: data) as? [String: Any])?["message"] as? String
            throw Failure.status(http.statusCode, message ?? HTTPURLResponse.localizedString(forStatusCode: http.statusCode))
        }
    }
}
