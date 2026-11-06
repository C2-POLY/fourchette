import SwiftUI

/// The repositories and the one picked: side by side on iPad, one after the other on iPhone
/// (NavigationSplitView collapses into a stack in a compact width).
struct RootView: View {
    @Environment(Session.self) private var session
    @State private var selection: Repo.ID?

    var body: some View {
        NavigationSplitView {
            RepoListView(selection: $selection)
        } detail: {
            if let repo = session.repos.first(where: { $0.id == selection }) {
                RepoDetailView(repo: repo)
            } else {
                ContentUnavailableView("No Repository Selected", systemImage: "arrow.triangle.branch", description: Text("Pick one in the list."))
            }
        }
        .task { await session.load() }
    }
}
