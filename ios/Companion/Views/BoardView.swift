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
    /// Why GitHub refused a move; the card is back in its column.
    @State private var moveError: String?
    @State private var dropTarget: String?

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
        .alert("The card didn't move", isPresented: Binding(get: { moveError != nil }, set: { if !$0 { moveError = nil } })) {
            Button("OK", role: .cancel) {}
        } message: {
            Text(moveError ?? "")
        }
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
                            .contextMenu { moveMenu(card, board) }
                            .swipeActions(edge: .trailing) {
                                Menu { moveButtons(card, board) } label: { Label("Move", systemImage: "arrow.right.square") }
                                    .tint(.accent)
                            }
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
                                    .draggable(card.id) {
                                        CardRow(card: card).padding(10).frame(width: 260).background(.background, in: RoundedRectangle(cornerRadius: 10))
                                    }
                                    .contextMenu { moveMenu(card, board) }
                                }
                            }
                        }
                        .refreshable { await loadBoard() }
                    }
                    .frame(width: 280)
                    .padding(8)
                    .background(.quaternary.opacity(0.5), in: RoundedRectangle(cornerRadius: 14))
                    .overlay {
                        RoundedRectangle(cornerRadius: 14).strokeBorder(Color.accent, lineWidth: 2).opacity(dropTarget == c.name ? 1 : 0)
                    }
                    .dropDestination(for: String.self) { ids, _ in
                        guard let id = ids.first, let card = board.cards.first(where: { $0.id == id }) else { return false }
                        move(card, to: c)
                        return true
                    } isTargeted: { dropTarget = $0 ? c.name : (dropTarget == c.name ? nil : dropTarget) }
                }
            }
            .padding()
        }
    }

    /// "Move to" and the other columns, for a card's context menu.
    private func moveMenu(_ card: Card, _ board: Board) -> some View {
        Menu("Move to") { moveButtons(card, board) }
    }

    /// One button per other column.
    private func moveButtons(_ card: Card, _ board: Board) -> some View {
        ForEach(board.columns.filter { $0 != board.column(of: card) && $0.id != nil }, id: \.name) { c in
            Button(c.name) { move(card, to: c) }
        }
    }

    /// Shown at once, then written; a refusal puts the card back and says why.
    private func move(_ card: Card, to column: Board.Column) {
        guard let before = board, before.column(of: card) != column else { return }
        board = before.moving(card.id, to: column)
        guard !session.sample, let client = session.client else { return }
        Task {
            do {
                try await client.move(card.id, to: column, on: before)
            } catch {
                board = board?.moving(card.id, to: before.column(of: card) ?? column)
                moveError = error.localizedDescription
            }
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
