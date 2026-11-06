import SwiftUI

/// My repositories and my organisations', grouped by owner, with a search and pull to refresh.
struct RepoListView: View {
    @Environment(Session.self) private var session
    @Binding var selection: Repo.ID?
    @State private var query = ""
    @State private var confirmSignOut = false

    var body: some View {
        let groups = session.grouped(matching: query)
        List(selection: $selection) {
            if let error = session.error {
                Label(error, systemImage: "exclamationmark.triangle")
                    .foregroundStyle(.red)
                    .font(.callout)
            }
            ForEach(groups, id: \.owner) { group in
                Section(group.owner) {
                    ForEach(group.repos) { repo in
                        RepoRow(repo: repo)
                            .tag(repo.id)
                    }
                }
            }
        }
        .overlay {
            if session.loading && session.repos.isEmpty {
                ProgressView("Loading repositories…")
            } else if !session.loading && groups.isEmpty {
                if query.isEmpty {
                    ContentUnavailableView("No Repositories", systemImage: "tray", description: Text("Your GitHub account has no repositories yet."))
                } else {
                    ContentUnavailableView.search(text: query)
                }
            }
        }
        .navigationTitle("Repositories")
        .searchable(text: $query, prompt: "Search repositories")
        .refreshable { await session.load() }
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Menu {
                    if let user = session.user {
                        Text("Signed in as \(user.login)")
                    }
                    Button("Sign Out", role: .destructive) { confirmSignOut = true }
                } label: {
                    Image(systemName: "person.crop.circle")
                }
            }
        }
        .confirmationDialog("Sign out of GitHub?", isPresented: $confirmSignOut, titleVisibility: .visible) {
            Button("Sign Out", role: .destructive) { session.signOut() }
        } message: {
            Text("The token is removed from this device.")
        }
    }
}

struct RepoRow: View {
    let repo: Repo

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            HStack(spacing: 6) {
                Text(repo.name)
                    .font(.body.weight(.medium))
                if repo.isPrivate {
                    Image(systemName: "lock.fill")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
            }
            if let text = repo.description, !text.isEmpty {
                Text(text)
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }
            if let pushed = repo.pushedAt {
                Text("Pushed \(pushed, format: .relative(presentation: .named))")
                    .font(.caption)
                    .foregroundStyle(.tertiary)
            }
        }
        .padding(.vertical, 2)
    }
}
