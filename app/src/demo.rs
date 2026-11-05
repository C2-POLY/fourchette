//! A small sample history, shown when no repository is open yet.
use crate::git::{Commit, CommitDetails, FileChange, GitRef, RefKind, Stash, Status};

fn h(i: usize) -> String {
    format!("{:08x}{:032x}", 0x3fa9c210u32.wrapping_add((i as u32).wrapping_mul(0x9e3779b1)), i * 7919)
}

pub struct Demo {
    pub commits: Vec<Commit>,
    pub refs: Vec<GitRef>,
    pub stashes: Vec<Stash>,
    pub status: Status,
}

pub fn build() -> Demo {
    // (id, parents, subject, author, hours ago)
    let spec: &[(usize, &[usize], &str, &str, i64)] = &[
        (30, &[29, 28], "Merge branch 'feature/graph-colours' into develop", "Camille Laurent", 1),
        (29, &[27], "Tidy up the sidebar spacing", "Lucas Martin", 3),
        (28, &[26], "Give every branch line its own colour", "Camille Laurent", 4),
        (27, &[25, 24], "Merge branch 'hotfix/login-timeout' into develop", "Inès Moreau", 6),
        (26, &[23], "Draw curved lines between lanes", "Camille Laurent", 8),
        (25, &[22], "Update the welcome text", "Lucas Martin", 11),
        (24, &[21], "Fix the sign-in timeout on slow networks", "Inès Moreau", 13),
        (23, &[22], "Start the new graph renderer", "Camille Laurent", 20),
        (22, &[20, 19], "Merge pull request #42 from feature/search", "Sami Haddad", 26),
        (21, &[18], "Release 2.3.0", "Inès Moreau", 30),
        (20, &[17], "Add keyboard shortcuts for fetch and push", "Lucas Martin", 34),
        (19, &[16], "Search commits by message, author or hash", "Sami Haddad", 40),
        (18, &[15, 17], "Merge branch 'develop'", "Inès Moreau", 48),
        (17, &[14], "Show ahead/behind counts in the toolbar", "Lucas Martin", 55),
        (16, &[14], "Search box in the commit list", "Sami Haddad", 60),
        (15, &[12], "Prepare release notes", "Inès Moreau", 70),
        (14, &[13, 11], "Merge branch 'feature/stash'", "Lucas Martin", 80),
        (13, &[10], "Stage and unstage single files", "Camille Laurent", 90),
        (11, &[10], "Stash and pop local changes", "Sami Haddad", 96),
        (12, &[9], "Release 2.2.1", "Inès Moreau", 110),
        (10, &[9], "Commit panel with summary and description", "Camille Laurent", 120),
        (9, &[8], "Diff view with added and removed lines", "Lucas Martin", 140),
        (8, &[7], "Show changed files for each commit", "Sami Haddad", 160),
        (7, &[6], "Sidebar with branches, remotes and tags", "Camille Laurent", 190),
        (6, &[5], "Read branches with for-each-ref", "Lucas Martin", 220),
        (5, &[4], "Load history with git log", "Camille Laurent", 260),
        (4, &[3], "Dark theme", "Inès Moreau", 300),
        (3, &[2], "Window layout", "Camille Laurent", 340),
        (2, &[1], "Add README", "Lucas Martin", 380),
        (1, &[], "Initial commit", "Camille Laurent", 400),
    ];
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let commits: Vec<Commit> = spec
        .iter()
        .map(|(id, ps, s, a, hrs)| Commit {
            hash: h(*id),
            parents: ps.iter().map(|p| h(*p)).collect(),
            author: a.to_string(),
            email: format!("{}@example.com", a.split_whitespace().next().unwrap_or("dev").to_lowercase()),
            time: now - hrs * 3600,
            subject: s.to_string(),
        })
        .collect();
    let r = |name: &str, kind: RefKind, id: usize, head: bool| GitRef {
        name: name.into(),
        full: name.into(),
        kind,
        target: h(id),
        is_head: head,
        upstream: String::new(),
        ahead: 0,
        behind: 0,
    };
    // (upstream, ahead, behind) of the sample's local branches.
    let track = |name: &str| match name {
        "develop" => ("origin/develop", 3, 0),
        "feature/graph-colours" => ("origin/feature/graph-colours", 0, 0),
        "main" => ("origin/main", 0, 2),
        "feature/search" => ("origin/feature/search", 0, 0),
        "hotfix/login-timeout" => ("origin/hotfix/login-timeout", 0, 0),
        _ => ("", 0, 0),
    };
    let r = |name: &str, kind: RefKind, id: usize, head: bool| {
        let local = kind == RefKind::Local;
        let mut g = r(name, kind, id, head);
        if local {
            let (u, a, b) = track(name);
            g.upstream = u.into();
            g.ahead = a;
            g.behind = b;
        }
        g
    };
    let refs = vec![
        r("develop", RefKind::Local, 30, true),
        r("origin/develop", RefKind::Remote, 27, false),
        r("feature/graph-colours", RefKind::Local, 28, false),
        r("origin/feature/graph-colours", RefKind::Remote, 28, false),
        r("hotfix/login-timeout", RefKind::Local, 24, false),
        r("main", RefKind::Local, 21, false),
        r("origin/main", RefKind::Remote, 21, false),
        r("v2.3.0", RefKind::Tag, 21, false),
        r("v2.2.1", RefKind::Tag, 12, false),
        r("feature/search", RefKind::Local, 19, false),
        r("feature/remember-the-last-opened-repository-per-window", RefKind::Local, 17, false),
        r("upstream/main", RefKind::Remote, 18, false),
        r("origin/feature/search", RefKind::Remote, 19, false),
        r("origin/feature/dark-mode", RefKind::Remote, 22, false),
        r("origin/hotfix/login-timeout", RefKind::Remote, 24, false),
        r("origin/release/2.3", RefKind::Remote, 21, false),
    ];
    let stashes = vec![Stash { name: "stash@{0}".into(), message: "WIP on develop: try a lighter toolbar".into() }];
    let status = Status {
        staged: vec![FileChange { path: "src/graph.rs".into(), code: 'M' }],
        unstaged: vec![
            FileChange { path: "src/theme.rs".into(), code: 'M' },
            FileChange { path: "assets/logo-dark.svg".into(), code: '?' },
            FileChange { path: "docs/old-notes.md".into(), code: 'D' },
        ],
    };
    Demo { commits, refs, stashes, status }
}

pub fn details(c: &Commit) -> CommitDetails {
    CommitDetails {
        hash: c.hash.clone(),
        author: c.author.clone(),
        email: c.email.clone(),
        author_time: c.time,
        committer: c.author.clone(),
        committer_email: c.email.clone(),
        committer_time: c.time,
        parents: c.parents.clone(),
        subject: c.subject.clone(),
        body: "This is a sample history so you can look around.\nOpen one of your own projects to see its real history.".into(),
        files: vec![
            FileChange { path: "src/graph.rs".into(), code: 'M' },
            FileChange { path: "src/theme.rs".into(), code: 'M' },
            FileChange { path: "src/lanes.rs".into(), code: 'A' },
            FileChange { path: "src/old_lanes.rs".into(), code: 'D' },
        ],
    }
}

pub fn diff(path: &str) -> String {
    format!(
        "diff --git a/{path} b/{path}\n--- a/{path}\n+++ b/{path}\n@@ -12,9 +12,12 @@ pub fn draw_lanes(rows: &[Row]) {{\n     let width = lane_width();\n-    let colour = GREY;\n+    // Each branch line gets its own colour.\n+    let colour = palette(row.colour);\n+    let curve = 5.0;\n     for edge in &row.edges {{\n-        draw_line(edge.from, edge.to, colour);\n+        draw_curve(edge.from, edge.to, colour, curve);\n     }}\n+    draw_node(row.lane, colour);\n }}\n"
    )
}
