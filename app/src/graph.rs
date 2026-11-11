//! Lays commits out in lanes so the history can be drawn as coloured lines.
use crate::git::Commit;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug)]
pub struct Edge {
    pub from: usize,
    pub to: usize,
    pub color: usize,
}

#[derive(Clone, Debug, Default)]
pub struct Row {
    pub lane: usize,
    pub color: usize,
    /// Lines from the top of the row to its middle.
    pub top: Vec<Edge>,
    /// Lines from the middle of the row to its bottom.
    pub bottom: Vec<Edge>,
    pub width: usize,
    pub is_merge: bool,
}

pub fn layout(commits: &[Commit]) -> (Vec<Row>, usize) {
    let known: HashMap<&str, ()> = commits.iter().map(|c| (c.hash.as_str(), ())).collect();
    let mut lanes: Vec<Option<(String, usize)>> = Vec::new();
    let mut next_color = 0usize;
    let mut rows = Vec::with_capacity(commits.len());
    let mut max_w = 1;

    for c in commits {
        let mut row = Row { is_merge: c.parents.len() > 1, ..Default::default() };
        let matching: Vec<usize> =
            lanes.iter().enumerate().filter(|(_, l)| l.as_ref().map(|(h, _)| h == &c.hash).unwrap_or(false)).map(|(i, _)| i).collect();

        let (node, color) = if let Some(&i) = matching.first() {
            (i, lanes[i].as_ref().unwrap().1)
        } else {
            let col = next_color;
            next_color += 1;
            let slot = lanes.iter().position(|l| l.is_none()).unwrap_or_else(|| {
                lanes.push(None);
                lanes.len() - 1
            });
            (slot, col)
        };
        row.lane = node;
        row.color = color;

        for (i, l) in lanes.iter().enumerate() {
            if let Some((h, col)) = l {
                if h == &c.hash {
                    row.top.push(Edge { from: i, to: node, color: *col });
                } else {
                    row.top.push(Edge { from: i, to: i, color: *col });
                }
            }
        }
        let width_before = lanes.len();
        for &i in &matching {
            lanes[i] = None;
        }

        // Edges are first built with the lane indices of this row; `map` then
        // compacts them, so every lane right of a freed one slides one column left.
        let mut fresh: Vec<usize> = Vec::new();
        let mut edges: Vec<Edge> = Vec::new();
        let mut tail = false;
        for (pi, p) in c.parents.iter().enumerate() {
            if !known.contains_key(p.as_str()) {
                // Parent outside the loaded range: draw a short tail down.
                if pi == 0 {
                    tail = true;
                }
                continue;
            }
            let existing = lanes.iter().position(|l| l.as_ref().map(|(h, _)| h == p).unwrap_or(false));
            match existing {
                Some(_) if pi == 0 => {
                    // Another lane awaits the same parent: keep our own line down to
                    // its row, where all of them end on its dot.
                    lanes[node] = Some((p.clone(), color));
                    fresh.push(node);
                    edges.push(Edge { from: node, to: node, color });
                }
                Some(j) => {
                    let col = lanes[j].as_ref().unwrap().1;
                    edges.push(Edge { from: node, to: j, color: col });
                }
                None => {
                    let (slot, col) = if pi == 0 && lanes[node].is_none() {
                        (node, color)
                    } else {
                        let col = next_color;
                        next_color += 1;
                        let slot = lanes.iter().position(|l| l.is_none()).unwrap_or_else(|| {
                            lanes.push(None);
                            lanes.len() - 1
                        });
                        (slot, col)
                    };
                    lanes[slot] = Some((p.clone(), col));
                    fresh.push(slot);
                    edges.push(Edge { from: node, to: slot, color: col });
                }
            }
        }
        let width_now = lanes.len();
        let mut map = vec![0usize; lanes.len()];
        let mut n = 0;
        for (i, l) in lanes.iter().enumerate() {
            map[i] = n;
            if l.is_some() {
                n += 1;
            }
        }
        for e in edges {
            // An edge starting at the node keeps its place; the lane it merges from keeps its own.
            row.bottom.push(Edge { from: e.from, to: map[e.to], color: e.color });
        }
        if tail {
            row.bottom.push(Edge { from: node, to: node, color });
        }
        for (i, l) in lanes.iter().enumerate() {
            if let Some((_, col)) = l {
                if !fresh.contains(&i) {
                    row.bottom.push(Edge { from: i, to: map[i], color: *col });
                }
            }
        }
        lanes.retain(|l| l.is_some());
        row.width = width_before.max(width_now).max(node + 1);
        max_w = max_w.max(row.width);
        rows.push(row);
    }
    (rows, max_w)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(h: &str, parents: &[&str]) -> Commit {
        Commit {
            hash: h.into(),
            parents: parents.iter().map(|p| p.to_string()).collect(),
            author: String::new(),
            email: String::new(),
            time: 0,
            subject: String::new(),
        }
    }

    #[test]
    fn linear_history_stays_in_lane_zero() {
        let commits = vec![c("d", &["c"]), c("c", &["b"]), c("b", &["a"]), c("a", &[])];
        let (rows, w) = layout(&commits);
        assert_eq!(w, 1);
        assert!(rows.iter().all(|r| r.lane == 0 && r.width == 1));
    }

    #[test]
    fn freed_lane_is_compacted() {
        // m merges x (lane 1) and y (lane 2); x's branch ends, so y slides from lane 2 to 1.
        let commits = vec![c("m", &["b", "x", "y"]), c("x", &[]), c("y", &["b"]), c("b", &[])];
        let (rows, _) = layout(&commits);
        assert_eq!(rows[1].lane, 1);
        assert!(rows[1].bottom.iter().any(|e| e.from == 2 && e.to == 1));
        assert_eq!(rows[2].lane, 1);
        assert_eq!(rows[3].lane, 0);
    }

    #[test]
    fn main_stays_left_when_a_feature_merges_main_repeatedly() {
        let mut commits = Vec::new();
        let mut main_hashes = Vec::new();
        // Built oldest to newest, then reversed (children first).
        let mut main = "m0".to_string();
        commits.push(c("m0", &[]));
        main_hashes.push(main.clone());
        for r in 0..10 {
            let fork = main.clone();
            let mut feat = format!("f{r}_0");
            commits.push(c(&feat, &[&fork]));
            for k in 1..4 {
                // Main advances, the feature merges it ("Merge origin/main into feat").
                let nm = format!("m{r}_{k}");
                commits.push(c(&nm, &[&main]));
                main_hashes.push(nm.clone());
                main = nm;
                let nf = format!("f{r}_{k}");
                commits.push(c(&nf, &[&feat, &main]));
                feat = nf;
            }
            let nm = format!("merge{r}");
            commits.push(c(&nm, &[&main, &feat]));
            main_hashes.push(nm.clone());
            main = nm;
        }
        commits.reverse();
        let (rows, w) = layout(&commits);
        assert!(w <= 3, "max width {w}");
        for (r, cm) in rows.iter().zip(&commits) {
            if main_hashes.contains(&cm.hash) {
                assert_eq!(r.lane, 0, "{} left lane 0", cm.hash);
            }
        }
    }

    #[test]
    fn branches_reach_their_shared_parent_without_looping() {
        // Two branches forked from the same main commit `b`, merged one after the other.
        let commits = vec![c("m2", &["m1", "y"]), c("y", &["b"]), c("m1", &["b", "x"]), c("x", &["b"]), c("b", &[])];
        let (rows, _) = layout(&commits);
        for (i, r) in rows.iter().enumerate() {
            // No column is both entered from another column and left toward another one.
            for e1 in r.bottom.iter().filter(|e| e.from != e.to) {
                assert!(
                    !r.bottom.iter().any(|e2| e2.from == e1.to && e2.to != e2.from),
                    "row {i}: a line enters column {} while another leaves it",
                    e1.to
                );
            }
        }
        // In b's row every lane converges on its dot through `top` edges.
        let b = &rows[4];
        assert_eq!(b.lane, 0);
        assert!(b.top.len() >= 2 && b.top.iter().all(|e| e.to == b.lane));
        // Branch lines stay straight in their own column until b's row.
        for r in &rows[1..4] {
            assert!(r.bottom.iter().filter(|e| e.to != 0).all(|e| e.from == e.to || r.lane == e.from));
        }
    }

    /// Measures the layout of a real repository; skipped when it is absent.
    #[test]
    #[ignore]
    fn real_repository_lanes() {
        let root = std::path::PathBuf::from(std::env::var("GRAPH_REPO").unwrap_or("/Users/yoni/dev/warhallow".into()));
        if !root.exists() {
            return;
        }
        let repo = crate::git::Repo { root };
        let commits = repo.commits(2000);
        let (rows, w) = layout(&commits);
        let head = commits.first().map(|c| c.hash.clone()).unwrap_or_default();
        let mut chain = std::collections::HashSet::new();
        let mut cur = head;
        let idx: HashMap<&str, &Commit> = commits.iter().map(|c| (c.hash.as_str(), c)).collect();
        while let Some(cm) = idx.get(cur.as_str()) {
            chain.insert(cm.hash.clone());
            match cm.parents.first() {
                Some(p) => cur = p.clone(),
                None => break,
            }
        }
        let max_main = rows.iter().zip(&commits).filter(|(_, c)| chain.contains(&c.hash)).map(|(r, _)| r.lane).max();
        println!("commits {} max width {w} first-parent chain of HEAD: {} commits, max lane {max_main:?}", commits.len(), chain.len());
    }
}
