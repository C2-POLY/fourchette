import Foundation
import Observation

/// Who is signed in and what the app shows of GitHub. Network work runs in tasks; the views
/// only read this and call its methods.
@MainActor
@Observable
final class Session {
    enum SignIn: Equatable {
        case idle
        /// Asking GitHub for a code.
        case requesting
        /// The code to type on github.com, while the app polls.
        case waiting(DeviceFlow.Code)
        case failed(String)
    }

    private(set) var client: GitHubClient?
    private(set) var user: User?
    var signIn: SignIn = .idle
    private(set) var repos: [Repo] = []
    private(set) var loading = false
    private(set) var error: String?
    /// Invented repositories, for the simulator's screenshots (`-sample` at launch).
    let sample: Bool
    private var polling: Task<Void, Never>?

    init(sample: Bool = ProcessInfo.processInfo.arguments.contains("-sample")) {
        self.sample = sample
        if sample {
            user = User(login: "camille", name: "Camille Laurent", avatarURL: nil)
            repos = Self.sampleRepos
        } else if let token = Keychain.token() {
            client = GitHubClient(token: token)
        }
    }

    var signedIn: Bool { client != nil || sample }

    // MARK: Sign in

    func startSignIn() {
        guard let clientID = DeviceFlow.clientID else {
            signIn = .failed(DeviceFlow.Failure.noClientID.localizedDescription)
            return
        }
        signIn = .requesting
        polling?.cancel()
        polling = Task {
            do {
                let code = try await DeviceFlow.requestCode(clientID: clientID)
                signIn = .waiting(code)
                try await waitForToken(clientID: clientID, code: code)
            } catch is CancellationError {
            } catch {
                signIn = .failed(error.localizedDescription)
            }
        }
    }

    func cancelSignIn() {
        polling?.cancel()
        signIn = .idle
    }

    private func waitForToken(clientID: String, code: DeviceFlow.Code) async throws {
        var interval = code.interval
        let deadline = Date().addingTimeInterval(TimeInterval(code.expiresIn))
        while Date() < deadline {
            try await Task.sleep(for: .seconds(interval))
            switch try await DeviceFlow.poll(clientID: clientID, deviceCode: code.deviceCode) {
            case .token(let token):
                Keychain.save(token)
                client = GitHubClient(token: token)
                signIn = .idle
                await load()
                return
            case .pending: continue
            case .slowDown(let next): interval = next
            case .failed(let message):
                signIn = .failed(message)
                return
            }
        }
        signIn = .failed("The code expired. Ask for a new one.")
    }

    func signOut() {
        Keychain.delete()
        client = nil
        user = nil
        repos = []
        error = nil
    }

    // MARK: Repositories

    func load() async {
        guard let client, !sample else { return }
        loading = true
        defer { loading = false }
        do {
            async let me = client.user()
            async let list = client.repos()
            (user, repos) = try await (me, list)
            error = nil
        } catch GitHubClient.Failure.unauthorized {
            signOut()
            signIn = .failed(GitHubClient.Failure.unauthorized.localizedDescription)
        } catch {
            self.error = error.localizedDescription
        }
    }

    /// The repositories by owner: mine first, then each organisation by name.
    func grouped(matching query: String) -> [(owner: String, repos: [Repo])] {
        let shown = repos.filter { $0.matches(query) }
        let owners = Dictionary(grouping: shown, by: \.owner.login)
        let me = user?.login
        return owners.keys
            .sorted { a, b in a == me ? true : b == me ? false : a.localizedCaseInsensitiveCompare(b) == .orderedAscending }
            .map { (owner: $0, repos: owners[$0]!) }
    }

    private static var sampleRepos: [Repo] {
        func repo(_ id: Int, _ owner: String, _ name: String, _ text: String, hours: Double, stars: Int = 0, issues: Int = 0, isPrivate: Bool = false) -> Repo {
            Repo(id: id, name: name, fullName: "\(owner)/\(name)", owner: .init(login: owner, avatarURL: nil), description: text, isPrivate: isPrivate, defaultBranch: "main",
                 htmlURL: URL(string: "https://github.com/\(owner)/\(name)")!, pushedAt: Date().addingTimeInterval(-hours * 3600), stargazersCount: stars, openIssuesCount: issues)
        }
        return [
            repo(1, "acme", "sample-project", "The sample project of Fourchette's screenshots", hours: 1, stars: 128, issues: 12),
            repo(2, "acme", "sample-site", "Its website", hours: 5, stars: 9, issues: 2),
            repo(3, "acme", "sample-api", "The API behind it", hours: 30, issues: 4, isPrivate: true),
            repo(4, "camille", "dotfiles", "My shell and editor settings", hours: 72, stars: 3),
            repo(5, "camille", "notes", "Notes and drafts", hours: 200, isPrivate: true),
            repo(6, "lab", "graph-experiments", "Trying out commit graph layouts", hours: 400, stars: 21, issues: 1),
        ]
    }
}
