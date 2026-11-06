import SwiftUI

/// A repository's pull requests: the open ones, then the recently closed or merged, each with
/// the result of its checks; a tap shows the checks.
struct PullsView: View {
    let repo: Repo
    @Environment(Session.self) private var session
    @State private var open: [Pull] = []
    @State private var recent: [Pull] = []
    @State private var loaded = false
    @State private var error: String?

    var body: some View {
        List {
            if let error {
                Label(error, systemImage: "exclamationmark.triangle").foregroundStyle(.red).font(.callout)
            }
            if !open.isEmpty {
                Section("Open") { ForEach(open) { pull in NavigationLink(value: pull) { PullRow(pull: pull) } } }
            }
            if !recent.isEmpty {
                Section("Recently closed") { ForEach(recent) { pull in NavigationLink(value: pull) { PullRow(pull: pull) } } }
            }
        }
        .overlay {
            if !loaded {
                ProgressView("Loading pull requests…")
            } else if open.isEmpty && recent.isEmpty && error == nil {
                ContentUnavailableView("No Pull Requests", systemImage: "arrow.triangle.pull", description: Text("This repository has none yet."))
            }
        }
        .navigationTitle("Pull Requests")
        .navigationBarTitleDisplayMode(.inline)
        .navigationDestination(for: Pull.self) { PullDetailView(pull: $0) }
        .refreshable { await load() }
        .task { if !loaded { await load() } }
    }

    private func load() async {
        defer { loaded = true }
        if session.sample {
            (open, recent) = (Sample.pulls.filter { $0.state == "OPEN" }, Sample.pulls.filter { $0.state != "OPEN" })
            return
        }
        guard let client = session.client else { return }
        do {
            (open, recent) = try await client.pulls(of: repo)
            error = nil
        } catch {
            self.error = error.localizedDescription
        }
    }
}

struct PullRow: View {
    let pull: Pull

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: pull.symbol)
                .foregroundStyle(pull.color)
                .frame(width: 18)
                .padding(.top, 2)
            VStack(alignment: .leading, spacing: 3) {
                Text(pull.title).font(.body.weight(.medium)).lineLimit(2)
                HStack(spacing: 6) {
                    Text("#\(pull.number)").monospacedDigit()
                    Text(pull.author)
                    if pull.isDraft { Text("Draft").foregroundStyle(.orange) }
                    if let updated = pull.updatedAt { Text(updated, format: .relative(presentation: .named)) }
                }
                .font(.caption)
                .foregroundStyle(.secondary)
            }
            Spacer(minLength: 4)
            ChecksBadge(state: pull.checksState)
        }
        .padding(.vertical, 2)
    }
}

/// The rollup of a pull request's checks: a cross, a dot or a tick.
struct ChecksBadge: View {
    let state: String?

    var body: some View {
        switch state {
        case "SUCCESS": Image(systemName: "checkmark.circle.fill").foregroundStyle(.green)
        case "FAILURE", "ERROR": Image(systemName: "xmark.circle.fill").foregroundStyle(.red)
        case "PENDING", "EXPECTED": Image(systemName: "circle.dotted").foregroundStyle(.orange)
        default: EmptyView()
        }
    }
}

/// One pull request: its state and branch, then its checks, failing first, each opening its log.
struct PullDetailView: View {
    let pull: Pull
    @Environment(\.openURL) private var openURL

    var body: some View {
        List {
            Section {
                VStack(alignment: .leading, spacing: 6) {
                    HStack(spacing: 6) {
                        Image(systemName: pull.symbol).foregroundStyle(pull.color)
                        Text(pull.stateLabel).foregroundStyle(pull.color)
                        Text("#\(pull.number)").monospacedDigit().foregroundStyle(.secondary)
                    }
                    .font(.subheadline.weight(.medium))
                    Text(pull.title).font(.title3.bold())
                    Text("\(pull.author) · \(pull.branch)").font(.subheadline).foregroundStyle(.secondary)
                }
                .padding(.vertical, 4)
            }
            Section(pull.checks.isEmpty ? "Checks" : "Checks · \(summary)") {
                if pull.checks.isEmpty {
                    Text("No checks ran on its last commit.").foregroundStyle(.secondary)
                }
                ForEach(pull.checks, id: \.self) { check in
                    Button {
                        if let url = check.url { openURL(url) }
                    } label: {
                        CheckRow(check: check)
                    }
                    .buttonStyle(.plain)
                    .disabled(check.url == nil)
                }
            }
            if let url = pull.url {
                Section {
                    Button { openURL(url) } label: { Label("Open on GitHub", systemImage: "safari") }
                }
            }
        }
        .navigationTitle("#\(pull.number)")
        .navigationBarTitleDisplayMode(.inline)
    }

    private var summary: String {
        let failed = pull.checks.filter { $0.outcome == .failed }.count
        let running = pull.checks.filter { $0.outcome == .running }.count
        let passed = pull.checks.filter { $0.outcome == .passed }.count
        return [failed > 0 ? "\(failed) failing" : nil, running > 0 ? "\(running) running" : nil, passed > 0 ? "\(passed) passed" : nil]
            .compactMap { $0 }.joined(separator: ", ")
    }
}

struct CheckRow: View {
    let check: Check

    var body: some View {
        HStack(spacing: 10) {
            icon.frame(width: 18)
            VStack(alignment: .leading, spacing: 2) {
                Text(check.name).foregroundStyle(.primary)
                if let source = check.source { Text(source).font(.caption).foregroundStyle(.secondary) }
            }
            Spacer()
            if let s = check.seconds {
                Text(PullQueries.duration(s)).font(.caption.monospacedDigit()).foregroundStyle(.secondary)
            }
        }
    }

    @ViewBuilder
    private var icon: some View {
        switch check.outcome {
        case .failed: Image(systemName: "xmark.circle.fill").foregroundStyle(.red)
        case .running: Image(systemName: "circle.dotted").foregroundStyle(.orange)
        case .passed: Image(systemName: "checkmark.circle.fill").foregroundStyle(.green)
        case .skipped: Image(systemName: "minus.circle").foregroundStyle(.secondary)
        }
    }
}

extension Pull {
    var symbol: String {
        switch state {
        case "MERGED": "arrow.triangle.merge"
        case "CLOSED": "xmark.circle"
        default: "arrow.triangle.pull"
        }
    }

    var color: Color {
        switch state {
        case "MERGED": .purple
        case "CLOSED": .red
        default: isDraft ? .secondary : .green
        }
    }

    var stateLabel: String {
        switch state {
        case "MERGED": "Merged"
        case "CLOSED": "Closed"
        default: isDraft ? "Draft" : "Open"
        }
    }
}
