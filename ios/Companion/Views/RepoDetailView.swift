import SwiftUI

/// One repository: what the REST API says of it, and the way to its board and pull requests.
struct RepoDetailView: View {
    let repo: Repo
    @Environment(\.openURL) private var openURL

    var body: some View {
        List {
            Section {
                VStack(alignment: .leading, spacing: 8) {
                    Text(repo.fullName)
                        .font(.title2.bold())
                    if let text = repo.description, !text.isEmpty {
                        Text(text)
                            .foregroundStyle(.secondary)
                    }
                }
                .padding(.vertical, 4)
            }
            Section {
                NavigationLink(value: Route.board) {
                    Label("Project", systemImage: "rectangle.split.3x1")
                }
                NavigationLink(value: Route.pulls) {
                    Label("Pull Requests", systemImage: "arrow.triangle.pull")
                }
            }
            Section {
                LabeledContent("Default branch", value: repo.defaultBranch)
                LabeledContent("Visibility", value: repo.isPrivate ? "Private" : "Public")
                LabeledContent("Open issues and pull requests", value: repo.openIssuesCount.formatted())
                LabeledContent("Stars", value: repo.stargazersCount.formatted())
                if let pushed = repo.pushedAt {
                    LabeledContent("Last push") {
                        Text(pushed, format: .relative(presentation: .named))
                    }
                }
            }
            Section {
                Button {
                    openURL(repo.htmlURL)
                } label: {
                    Label("Open on GitHub", systemImage: "safari")
                }
            }
        }
        .navigationTitle(repo.name)
        .navigationBarTitleDisplayMode(.inline)
    }
}
