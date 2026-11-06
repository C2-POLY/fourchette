import SwiftUI

/// A repository's branches, the default one first, each with its last commit; a branch opens
/// its history.
struct BranchesView: View {
    let repo: Repo
    @Environment(Session.self) private var session
    @State private var branches: [Branch] = []
    @State private var loaded = false
    @State private var error: String?
    @State private var query = ""

    var body: some View {
        List {
            if let error {
                Label(error, systemImage: "exclamationmark.triangle").foregroundStyle(.red).font(.callout)
            }
            ForEach(branches.filter { query.isEmpty || $0.name.localizedCaseInsensitiveContains(query) }) { branch in
                NavigationLink(value: branch) {
                    VStack(alignment: .leading, spacing: 3) {
                        HStack(spacing: 6) {
                            Text(branch.name).font(.body.weight(.medium)).lineLimit(1)
                            if branch.name == repo.defaultBranch {
                                Text("default").font(.caption2.weight(.medium)).padding(.horizontal, 6).padding(.vertical, 1)
                                    .background(Color.accent.opacity(0.18), in: Capsule()).foregroundStyle(Color.accent)
                            }
                        }
                        Text(branch.headline).font(.subheadline).foregroundStyle(.secondary).lineLimit(1)
                        HStack(spacing: 6) {
                            Text(String(branch.sha.prefix(7))).monospaced()
                            Text(branch.author)
                            if let date = branch.date { Text(date, format: .relative(presentation: .named)) }
                        }
                        .font(.caption)
                        .foregroundStyle(.tertiary)
                    }
                    .padding(.vertical, 2)
                }
            }
        }
        .overlay {
            if !loaded { ProgressView("Loading branches…") }
        }
        .searchable(text: $query, prompt: "Filter branches")
        .navigationTitle("Branches")
        .navigationBarTitleDisplayMode(.inline)
        .navigationDestination(for: Branch.self) { CommitsView(repo: repo, branch: $0) }
        .navigationDestination(for: Commit.self) { CommitDetailView(repo: repo, commit: $0) }
        .refreshable { await load() }
        .task { if !loaded { await load() } }
    }

    private func load() async {
        defer { loaded = true }
        if session.sample {
            branches = Sample.branches
            return
        }
        guard let client = session.client else { return }
        do {
            branches = try await client.branches(of: repo)
            error = nil
        } catch {
            self.error = error.localizedDescription
        }
    }
}

/// A branch's commits, newest first, 30 at a time.
struct CommitsView: View {
    let repo: Repo
    let branch: Branch
    @Environment(Session.self) private var session
    @State private var commits: [Commit] = []
    @State private var page = 0
    @State private var more = true
    @State private var loading = false
    @State private var error: String?

    var body: some View {
        List {
            ForEach(commits) { commit in
                NavigationLink(value: commit) { CommitRow(commit: commit) }
            }
            if let error {
                Label(error, systemImage: "exclamationmark.triangle").foregroundStyle(.red).font(.callout)
            } else if more {
                HStack {
                    Spacer()
                    ProgressView()
                    Spacer()
                }
                .task { await next() }
            }
        }
        .navigationTitle(branch.name)
        .navigationBarTitleDisplayMode(.inline)
        .refreshable {
            (commits, page, more) = ([], 0, true)
            await next()
        }
    }

    /// The next page, once the bottom of the list shows.
    private func next() async {
        guard !loading, more else { return }
        loading = true
        defer { loading = false }
        if session.sample {
            commits = Sample.commits
            more = false
            return
        }
        guard let client = session.client else { return }
        do {
            let got = try await client.commits(of: repo, branch: branch.name, page: page + 1)
            commits += got
            page += 1
            more = got.count == 30
            error = nil
        } catch {
            self.error = error.localizedDescription
        }
    }
}

struct CommitRow: View {
    let commit: Commit

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(commit.headline).font(.body).lineLimit(2)
            HStack(spacing: 6) {
                Text(commit.short).monospaced()
                Text(commit.login ?? commit.authorName)
                if let date = commit.date { Text(date, format: .relative(presentation: .named)) }
            }
            .font(.caption)
            .foregroundStyle(.secondary)
        }
        .padding(.vertical, 2)
    }
}

/// A commit: its whole message, author and date, and the files it changed.
struct CommitDetailView: View {
    let repo: Repo
    let commit: Commit
    @Environment(Session.self) private var session
    @Environment(\.openURL) private var openURL
    @State private var files: [ChangedFile]?
    @State private var error: String?

    var body: some View {
        List {
            Section {
                VStack(alignment: .leading, spacing: 8) {
                    Text(commit.headline).font(.title3.bold())
                    if !commit.body.isEmpty {
                        Text(commit.body).font(.callout).foregroundStyle(.secondary).textSelection(.enabled)
                    }
                }
                .padding(.vertical, 4)
                LabeledContent("Author", value: commit.authorName)
                if let date = commit.date {
                    LabeledContent("Date") { Text(date, format: .dateTime.day().month().year().hour().minute()) }
                }
                LabeledContent("SHA") { Text(commit.short).monospaced().textSelection(.enabled) }
            }
            Section(files.map { "\($0.count) changed files" } ?? "Changed files") {
                if let error {
                    Label(error, systemImage: "exclamationmark.triangle").foregroundStyle(.red).font(.callout)
                } else if let files {
                    ForEach(files, id: \.filename) { FileRow(file: $0) }
                } else {
                    ProgressView()
                }
            }
            Section {
                Button {
                    openURL(URL(string: "https://github.com/\(repo.fullName)/commit/\(commit.sha)")!)
                } label: {
                    Label("Open on GitHub", systemImage: "safari")
                }
            }
        }
        .navigationTitle(commit.short)
        .navigationBarTitleDisplayMode(.inline)
        .task { await load() }
    }

    private func load() async {
        if session.sample {
            files = Sample.files
            return
        }
        guard let client = session.client, files == nil else { return }
        do {
            files = try await client.commitDetail(of: repo, sha: commit.sha).files
        } catch {
            self.error = error.localizedDescription
        }
    }
}

struct FileRow: View {
    let file: ChangedFile

    var body: some View {
        HStack(spacing: 10) {
            Text(letter)
                .font(.caption.weight(.bold).monospaced())
                .foregroundStyle(color)
                .frame(width: 20, height: 20)
                .background(color.opacity(0.15), in: RoundedRectangle(cornerRadius: 5))
            VStack(alignment: .leading, spacing: 1) {
                Text((file.filename as NSString).lastPathComponent).lineLimit(1)
                let folder = (file.filename as NSString).deletingLastPathComponent
                if !folder.isEmpty { Text(folder).font(.caption).foregroundStyle(.secondary).lineLimit(1) }
            }
            Spacer()
            HStack(spacing: 4) {
                if file.additions > 0 { Text("+\(file.additions)").foregroundStyle(.green) }
                if file.deletions > 0 { Text("−\(file.deletions)").foregroundStyle(.red) }
            }
            .font(.caption.monospacedDigit())
        }
    }

    private var letter: String {
        switch file.status {
        case "added": "A"
        case "removed": "D"
        case "renamed": "R"
        default: "M"
        }
    }

    private var color: Color {
        switch file.status {
        case "added": .green
        case "removed": .red
        case "renamed": .blue
        default: .orange
        }
    }
}
