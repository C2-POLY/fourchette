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

        let mut fresh: Vec<usize> = Vec::new();
        for (pi, p) in c.parents.iter().enumerate() {
            if !known.contains_key(p.as_str()) {
                // Parent outside the loaded range: draw a short tail down.
                if pi == 0 {
                    row.bottom.push(Edge { from: node, to: node, color });
                }
                continue;
            }
            let existing = lanes.iter().position(|l| l.as_ref().map(|(h, _)| h == p).unwrap_or(false));
            match existing {
                Some(j) => {
                    let col = lanes[j].as_ref().unwrap().1;
                    row.bottom.push(Edge { from: node, to: j, color: if pi == 0 { color } else { col } });
                }
                None => {
                    let (slot, col) = if pi == 0 && lanes[node].is_none() {
                        (node, color)
                    } else {
                        let col = next_color;
                        next_color += 1;
                        let slot = lanes.iter().enumerate().position(|(k, l)| l.is_none() && k > node).unwrap_or_else(|| {
                            lanes.push(None);
                            lanes.len() - 1
                        });
                        (slot, col)
                    };
                    lanes[slot] = Some((p.clone(), col));
                    fresh.push(slot);
                    row.bottom.push(Edge { from: node, to: slot, color: col });
                }
            }
        }
        for (i, l) in lanes.iter().enumerate() {
            if let Some((_, col)) = l {
                if !fresh.contains(&i) {
                    row.bottom.push(Edge { from: i, to: i, color: *col });
                }
            }
        }
        while matches!(lanes.last(), Some(None)) {
            lanes.pop();
        }
        row.width = width_before.max(lanes.len()).max(node + 1);
        max_w = max_w.max(row.width);
        rows.push(row);
    }
    (rows, max_w)
}
