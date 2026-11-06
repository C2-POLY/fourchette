import SwiftUI

/// One repository: what the REST API says of it. The board, the pull requests and the history
/// come with their own issues (#174, #176, #177).
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
