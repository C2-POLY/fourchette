import SwiftUI

/// The repositories and the one picked: side by side on iPad, one after the other on iPhone
/// (NavigationSplitView collapses into a stack in a compact width).
struct RootView: View {
    @Environment(Session.self) private var session
    @State private var selection: Repo.ID?
    /// Routes, then the cards and pull requests pushed from them.
    @State private var path = NavigationPath()
    /// The route to open once the selection has changed (a new selection empties the path).
    @State private var opening: Route?

    var body: some View {
        NavigationSplitView {
            RepoListView(selection: $selection)
        } detail: {
            if let repo = session.repos.first(where: { $0.id == selection }) {
                // Its own stack, so the board and a ticket push inside the detail column.
                NavigationStack(path: $path) {
                    RepoDetailView(repo: repo)
                        .navigationDestination(for: Route.self) { route in
                            switch route {
                            case .board: BoardView(repo: repo)
                            case .pulls: PullsView(repo: repo)
                            case .history: BranchesView(repo: repo)
                            }
                        }
                }
                .id(repo.id)
            } else {
                ContentUnavailableView("No Repository Selected", systemImage: "arrow.triangle.branch", description: Text("Pick one in the list."))
            }
        }
        .task {
            // `-sample -open <route>`: the sample project already open there, for screenshots.
            if session.sample, let route = Route.fromLaunch {
                opening = route
                selection = session.repos.first { $0.name == "sample-project" }?.id
            }
            await session.load()
        }
        .onChange(of: selection) {
            path = NavigationPath(opening.map { [$0] } ?? [])
            opening = nil
        }
    }
}

/// What a repository's detail opens.
enum Route: String, Hashable {
    case board, pulls, history

    static var fromLaunch: Route? {
        let args = ProcessInfo.processInfo.arguments
        guard let i = args.firstIndex(of: "-open"), i + 1 < args.count else { return nil }
        return Route(rawValue: args[i + 1])
    }
}
