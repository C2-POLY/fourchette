import SwiftUI

/// A repository's project board: its columns side by side on iPad, one column at a time on
/// iPhone, and a ticket's detail on a tap.
struct BoardView: View {
    let repo: Repo
    @Environment(Session.self) private var session
    @Environment(\.horizontalSizeClass) private var width
    @State private var projects: [ProjectRef] = []
    @State private var project: ProjectRef?
    @State private var board: Board?
    @State private var column: String?
    @State private var loading = false
    @State private var error: String?

    var body: some View {
        Group {
            if let board {
                if width == .compact {
                    compact(board)
                } else {
                    regular(board)
                }
            } else if loading {
                ProgressView("Loading the project…")
            } else if let error {
                ContentUnavailableView("Can't Read the Project", systemImage: "exclamationmark.triangle", description: Text(error))
            } else {
                ContentUnavailableView("No Project", systemImage: "rectangle.split.3x1", description: Text("No GitHub project holds this repository's issues."))
            }
        }
        .navigationTitle(project?.title ?? "Project")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            if projects.count > 1 {
                ToolbarItem(placement: .topBarTrailing) {
                    Menu {
                        Picker("Project", selection: $project) {
                            ForEach(projects, id: \.self) { Text($0.title).tag(Optional($0)) }
                        }
                    } label: {
                        Image(systemName: "rectangle.stack")
                    }
                }
            }
        }
        .navigationDestination(for: Card.self) { CardDetailView(card: $0) }
        .task { await loadProjects() }
        .onChange(of: project) { Task { await loadBoard() } }
    }

    /// iPhone: a column picker above the column's cards.
    private func compact(_ board: Board) -> some View {
        let current = board.columns.first { $0.name == column } ?? board.columns.first
        return List {
            Section {
                Picker("Column", selection: Binding(get: { current?.name }, set: { column = $0 })) {
                    ForEach(board.columns, id: \.name) { c in
                        Text("\(c.name) (\(board.cards(in: c).count))").tag(Optional(c.name))
                    }
                }
                .pickerStyle(.menu)
            }
            if let current {
                Section {
                    ForEach(board.cards(in: current)) { card in
                        NavigationLink(value: card) { CardRow(card: card) }
                    }
                }
            }
        }
        .refreshable { await loadBoard() }
    }

    /// iPad: every column side by side, scrolling sideways.
    private func regular(_ board: Board) -> some View {
        ScrollView(.horizontal) {
            HStack(alignment: .top, spacing: 14) {
                ForEach(board.columns, id: \.name) { c in
                    let cards = board.cards(in: c)
                    VStack(alignment: .leading, spacing: 8) {
                        HStack(spacing: 6) {
                            Circle().fill(Color.github(c.color)).frame(width: 9, height: 9)
                            Text(c.name).font(.subheadline.weight(.semibold))
                            Text("\(cards.count)").font(.caption).foregroundStyle(.secondary)
                        }
                        .padding(.horizontal, 4)
                        ScrollView {
                            LazyVStack(spacing: 8) {
                                ForEach(cards) { card in
                                    NavigationLink(value: card) {
                                        CardRow(card: card)
                                            .frame(maxWidth: .infinity, alignment: .leading)
                                            .padding(10)
                                            .background(.background, in: RoundedRectangle(cornerRadius: 10))
                                    }
                                    .buttonStyle(.plain)
                                }
                            }
                        }
                        .refreshable { await loadBoard() }
                    }
                    .frame(width: 280)
                    .padding(8)
                    .background(.quaternary.opacity(0.5), in: RoundedRectangle(cornerRadius: 14))
                }
            }
            .padding()
        }
    }

    private func loadProjects() async {
        if session.sample {
            board = Sample.board
            project = Sample.project
            return
        }
        guard let client = session.client, projects.isEmpty else { return }
        loading = true
        do {
            projects = try await client.projects(of: repo)
            project = projects.first
            if project == nil { loading = false }
        } catch {
            self.error = error.localizedDescription
            loading = false
        }
    }

    private func loadBoard() async {
        guard let client = session.client, let project, !session.sample else { return }
        loading = true
        defer { loading = false }
        do {
            board = try await client.board(project)
            error = nil
        } catch {
            self.error = error.localizedDescription
        }
    }
}

/// What a card shows: type and number, title, labels, sub-issue progress and assignees.
struct CardRow: View {
    let card: Card

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 6) {
                Image(systemName: card.symbol)
                    .foregroundStyle(card.stateColor)
                    .font(.caption)
                if let type = card.type {
                    Text(type).font(.caption.weight(.medium)).foregroundStyle(.secondary)
                }
                if let n = card.number {
                    Text("#\(n)").font(.caption.monospacedDigit()).foregroundStyle(.secondary)
                }
                Spacer(minLength: 0)
                if let who = card.assignees.first {
                    Text(who).font(.caption2).foregroundStyle(.tertiary)
                }
            }
            Text(card.title)
                .font(.subheadline.weight(.medium))
                .lineLimit(3)
            if !card.labels.isEmpty {
                LabelChips(labels: card.labels)
            }
            if card.subTotal > 0 {
                ProgressView(value: Double(card.subDone), total: Double(card.subTotal)) {
                    Text("\(card.subDone)/\(card.subTotal) sub-issues").font(.caption2).foregroundStyle(.secondary)
                }
                .tint(.accent)
            }
        }
        .padding(.vertical, 2)
    }
}

struct LabelChips: View {
    let labels: [IssueLabel]

    var body: some View {
        ViewThatFits(in: .horizontal) {
            chips(labels)
            chips(Array(labels.prefix(2)))
            chips(Array(labels.prefix(1)))
        }
    }

    private func chips(_ shown: [IssueLabel]) -> some View {
        HStack(spacing: 4) {
            ForEach(shown, id: \.name) { l in
                Text(l.name)
                    .font(.caption2.weight(.medium))
                    .padding(.horizontal, 7)
                    .padding(.vertical, 2)
                    .background(Color(hex: l.color).opacity(0.22), in: Capsule())
                    .foregroundStyle(Color(hex: l.color).labelInk)
                    .lineLimit(1)
            }
        }
    }
}

extension Card {
    var symbol: String {
        switch kind {
        case .draft: "doc.text"
        case .pullRequest: state == "MERGED" ? "arrow.triangle.merge" : "arrow.triangle.pull"
        case .issue: state == "CLOSED" ? "checkmark.circle" : "circle.circle"
        }
    }

    var stateColor: Color {
        switch state {
        case "OPEN": .green
        case "MERGED": .purple
        case "CLOSED": kind == .issue ? .purple : .red
        default: .secondary
        }
    }
}

extension Color {
    /// A label's colour: six hex digits.
    init(hex: String) {
        let v = UInt32(hex.prefix(6), radix: 16) ?? 0x888888
        self.init(red: Double(v >> 16 & 0xFF) / 255, green: Double(v >> 8 & 0xFF) / 255, blue: Double(v & 0xFF) / 255)
    }

    /// Text on a label's tint: the colour itself, darker in light mode so it stays readable.
    var labelInk: Color {
        Color(UIColor { traits in
            let base = UIColor(self)
            return traits.userInterfaceStyle == .dark ? base : base.withBrightness(0.55)
        })
    }

    /// A Project option's colour name.
    static func github(_ name: String) -> Color {
        switch name {
        case "BLUE": .blue
        case "GREEN": .green
        case "YELLOW": .yellow
        case "ORANGE": .orange
        case "RED": .red
        case "PINK": .pink
        case "PURPLE": .purple
        default: .gray
        }
    }
}

private extension UIColor {
    func withBrightness(_ factor: CGFloat) -> UIColor {
        var h: CGFloat = 0, s: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        getHue(&h, saturation: &s, brightness: &b, alpha: &a)
        return UIColor(hue: h, saturation: s, brightness: b * factor, alpha: a)
    }
}
