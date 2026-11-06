import SwiftUI

/// A ticket of the board: its status, labels, body, sub-issues and linked pull requests.
struct CardDetailView: View {
    let card: Card
    @Environment(\.openURL) private var openURL

    var body: some View {
        List {
            Section {
                VStack(alignment: .leading, spacing: 8) {
                    HStack(spacing: 6) {
                        Image(systemName: card.symbol).foregroundStyle(card.stateColor)
                        if let type = card.type { Text(type).foregroundStyle(.secondary) }
                        if let n = card.number { Text("#\(n)").foregroundStyle(.secondary).monospacedDigit() }
                    }
                    .font(.subheadline)
                    Text(card.title).font(.title3.bold())
                    if !card.labels.isEmpty { LabelChips(labels: card.labels) }
                }
                .padding(.vertical, 4)
                LabeledContent("Status", value: card.status ?? "No status")
                if !card.assignees.isEmpty {
                    LabeledContent("Assignees", value: card.assignees.joined(separator: ", "))
                }
                if let repo = card.repository { LabeledContent("Repository", value: repo) }
                if let updated = card.updatedAt {
                    LabeledContent("Updated") { Text(updated, format: .relative(presentation: .named)) }
                }
            }
            if let parent = card.parent {
                Section("Parent") { LinkedRow(item: parent) }
            }
            if !card.subIssues.isEmpty {
                Section("Sub-issues · \(card.subDone)/\(card.subTotal)") {
                    ForEach(card.subIssues, id: \.number) { LinkedRow(item: $0) }
                }
            }
            if !card.pullRequests.isEmpty {
                Section("Pull requests") {
                    ForEach(card.pullRequests, id: \.number) { LinkedRow(item: $0) }
                }
            }
            if !card.body.isEmpty {
                Section("Description") {
                    Text(markdown(card.body)).font(.callout).textSelection(.enabled)
                }
            }
            if let url = card.url {
                Section {
                    Button { openURL(url) } label: { Label("Open on GitHub", systemImage: "safari") }
                }
            }
        }
        .navigationTitle(card.number.map { "#\($0)" } ?? "Draft")
        .navigationBarTitleDisplayMode(.inline)
    }

    private func markdown(_ text: String) -> AttributedString {
        (try? AttributedString(markdown: text, options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace))) ?? AttributedString(text)
    }
}

/// A sub-issue, parent or pull request: its state, number and title; a tap opens it in Safari.
struct LinkedRow: View {
    let item: Linked
    @Environment(\.openURL) private var openURL

    var body: some View {
        Button {
            if let url = item.url { openURL(url) }
        } label: {
            HStack(spacing: 8) {
                Image(systemName: icon).foregroundStyle(color)
                Text("#\(item.number)").monospacedDigit().foregroundStyle(.secondary)
                Text(item.title).foregroundStyle(.primary).lineLimit(2)
            }
            .font(.subheadline)
        }
        .buttonStyle(.plain)
    }

    private var icon: String {
        switch item.state {
        case "MERGED": "arrow.triangle.merge"
        case "CLOSED": "checkmark.circle"
        default: "circle.circle"
        }
    }

    private var color: Color {
        switch item.state {
        case "OPEN": .green
        case "MERGED", "CLOSED": .purple
        default: .secondary
        }
    }
}
