//! The changed files grouped by folder, for the Tree view of the local changes.

use crate::git::FileChange;
use std::collections::HashSet;

/// A folder or a file of the tree.
#[derive(Debug)]
pub enum Node<'a> {
    /// `name` is what the row shows (`src/app` when a folder holds only another folder),
    /// `path` the folder from the repository root, without a trailing slash.
    Dir { name: String, path: String, children: Vec<Node<'a>> },
    File(&'a FileChange),
}

/// One line of the tree as drawn, with its depth.
#[derive(Debug)]
pub enum Row<'a> {
    Dir { name: String, path: String, depth: usize, open: bool, count: usize },
    File { file: &'a FileChange, depth: usize },
}

/// Groups the files by folder: folders first then files, each by name, and a folder whose only
/// child is a folder merged with it.
pub fn build(files: &[FileChange]) -> Vec<Node<'_>> {
    let mut sorted: Vec<&FileChange> = files.iter().collect();
    sorted.sort_by(|a, b| a.path.to_lowercase().cmp(&b.path.to_lowercase()).then_with(|| a.path.cmp(&b.path)));
    level(&sorted, "")
}

/// The nodes under `prefix` (a folder path with its trailing slash, or empty for the root).
fn level<'a>(files: &[&'a FileChange], prefix: &str) -> Vec<Node<'a>> {
    let mut dirs: Vec<(String, Vec<&'a FileChange>)> = Vec::new();
    let mut leaves = Vec::new();
    for f in files {
        let rest = &f.path[prefix.len()..];
        match rest.split_once('/') {
            Some((d, _)) => match dirs.iter_mut().find(|(n, _)| n == d) {
                Some((_, v)) => v.push(f),
                None => dirs.push((d.to_string(), vec![f])),
            },
            None => leaves.push(*f),
        }
    }
    dirs.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()).then_with(|| a.0.cmp(&b.0)));
    let mut out = Vec::new();
    for (name, fs) in dirs {
        let mut name = name;
        let mut children = level(&fs, &format!("{prefix}{name}/"));
        // A folder holding only one folder shares its row with it.
        while let [Node::Dir { .. }] = children.as_slice() {
            if let Some(Node::Dir { name: n, children: c, .. }) = children.pop() {
                name = format!("{name}/{n}");
                children = c;
            }
        }
        out.push(Node::Dir { path: format!("{prefix}{name}"), name, children });
    }
    out.extend(leaves.into_iter().map(Node::File));
    out
}

fn count(nodes: &[Node<'_>]) -> usize {
    nodes.iter().map(|n| match n {
        Node::Dir { children, .. } => count(children),
        Node::File(_) => 1,
    }).sum()
}

/// The lines to draw: a collapsed folder (its path is in `collapsed`) hides what is under it.
pub fn rows<'a>(files: &'a [FileChange], collapsed: &HashSet<String>) -> Vec<Row<'a>> {
    fn walk<'a>(nodes: Vec<Node<'a>>, depth: usize, collapsed: &HashSet<String>, out: &mut Vec<Row<'a>>) {
        for n in nodes {
            match n {
                Node::File(file) => out.push(Row::File { file, depth }),
                Node::Dir { name, path, children } => {
                    let open = !collapsed.contains(&path);
                    out.push(Row::Dir { name, depth, open, count: count(&children), path });
                    if open {
                        walk(children, depth + 1, collapsed, out);
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(build(files), 0, collapsed, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fc(p: &str) -> FileChange {
        FileChange { path: p.into(), code: 'M' }
    }

    fn names(rows: &[Row<'_>]) -> Vec<String> {
        rows.iter()
            .map(|r| match r {
                Row::Dir { name, depth, .. } => format!("{}{name}/", " ".repeat(*depth)),
                Row::File { file, depth } => format!("{}{}", " ".repeat(*depth), file.path.rsplit('/').next().unwrap()),
            })
            .collect()
    }

    #[test]
    fn root_files_stay_at_the_root() {
        let files = [fc("b.md"), fc("a.rs")];
        assert_eq!(names(&rows(&files, &HashSet::new())), ["a.rs", "b.md"]);
    }

    #[test]
    fn nests_folders_and_sorts_folders_first() {
        let files = [fc("README.md"), fc("src/b.rs"), fc("src/a.rs"), fc("src/ui/x.rs"), fc("docs/g.md"), fc("src/ui/y.rs")];
        assert_eq!(names(&rows(&files, &HashSet::new())), ["docs/", " g.md", "src/", " ui/", "  x.rs", "  y.rs", " a.rs", " b.rs", "README.md"]);
    }

    #[test]
    fn a_folder_with_a_single_folder_merges() {
        let files = [fc("app/src/app/a.rs"), fc("app/src/app/b.rs")];
        let r = rows(&files, &HashSet::new());
        assert_eq!(names(&r), ["app/src/app/", " a.rs", " b.rs"]);
        assert!(matches!(&r[0], Row::Dir { path, count: 2, .. } if path == "app/src/app"));
    }

    #[test]
    fn a_folder_with_a_file_and_a_folder_does_not_merge() {
        let files = [fc("src/lib.rs"), fc("src/app/a.rs")];
        assert_eq!(names(&rows(&files, &HashSet::new())), ["src/", " app/", "  a.rs", " lib.rs"]);
    }

    #[test]
    fn merging_stops_where_a_folder_branches() {
        let files = [fc("a/b/c/x.rs"), fc("a/b/d/y.rs")];
        let r = rows(&files, &HashSet::new());
        assert_eq!(names(&r), ["a/b/", " c/", "  x.rs", " d/", "  y.rs"]);
        assert!(matches!(&r[1], Row::Dir { path, .. } if path == "a/b/c"));
    }

    #[test]
    fn a_collapsed_folder_hides_its_content() {
        let files = [fc("src/a.rs"), fc("src/ui/x.rs"), fc("z.rs")];
        let collapsed: HashSet<String> = ["src".to_string()].into();
        let r = rows(&files, &collapsed);
        assert_eq!(names(&r), ["src/", "z.rs"]);
        assert!(matches!(&r[0], Row::Dir { open: false, count: 2, .. }));
    }
}
