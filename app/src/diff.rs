//! The rows of a diff shown side by side: the unified text from git turned into rows where the
//! old file is on the left and the new one on the right.

/// What a full-width row is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Full {
    /// `diff --git`, `index`, `---`, `+++`, and the "No newline" notes.
    Header,
    /// `@@ -a,b +c,d @@ …`
    Hunk,
}

/// How a cell is painted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Context,
    Deleted,
    Added,
}

/// One line of one side, with its number in that file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    pub num: u32,
    pub text: String,
    pub kind: Kind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
    Full(Full, String),
    /// Old side, new side; `None` is the blank padding facing a line that has no counterpart
    /// (and a whole column for a new or deleted file).
    Pair(Option<Cell>, Option<Cell>),
}

/// "@@ -12,9 +12,12 @@" gives (12, 12).
fn hunk_starts(l: &str) -> (u32, u32) {
    let parse = |s: &str| s.get(1..).and_then(|x| x.split(',').next()).and_then(|x| x.parse::<u32>().ok()).unwrap_or(1);
    let parts: Vec<&str> = l.split_whitespace().collect();
    if parts.len() > 2 {
        (parse(parts[1]), parse(parts[2]))
    } else {
        (1, 1)
    }
}

/// Turns git's unified diff into side-by-side rows. A run of deleted lines followed by a run of
/// added ones is paired line by line, the shorter side padded.
pub fn side_by_side(text: &str) -> Vec<Row> {
    let mut rows = Vec::new();
    let (mut o, mut n) = (0u32, 0u32);
    let mut in_hunk = false;
    let (mut dels, mut adds): (Vec<Cell>, Vec<Cell>) = (Vec::new(), Vec::new());
    fn flush(rows: &mut Vec<Row>, dels: &mut Vec<Cell>, adds: &mut Vec<Cell>) {
        let (mut d, mut a) = (std::mem::take(dels).into_iter(), std::mem::take(adds).into_iter());
        loop {
            match (d.next(), a.next()) {
                (None, None) => break,
                (l, r) => rows.push(Row::Pair(l, r)),
            }
        }
    }
    for l in text.lines() {
        if l.starts_with("diff ") {
            flush(&mut rows, &mut dels, &mut adds);
            in_hunk = false;
            rows.push(Row::Full(Full::Header, l.to_string()));
        } else if l.starts_with("@@") {
            flush(&mut rows, &mut dels, &mut adds);
            in_hunk = true;
            (o, n) = hunk_starts(l);
            rows.push(Row::Full(Full::Hunk, l.to_string()));
        } else if !in_hunk || l.starts_with('\\') {
            flush(&mut rows, &mut dels, &mut adds);
            rows.push(Row::Full(Full::Header, l.to_string()));
        } else if let Some(t) = l.strip_prefix('+') {
            adds.push(Cell { num: n, text: t.to_string(), kind: Kind::Added });
            n += 1;
        } else if let Some(t) = l.strip_prefix('-') {
            // A deletion after additions starts a new block.
            if !adds.is_empty() {
                flush(&mut rows, &mut dels, &mut adds);
            }
            dels.push(Cell { num: o, text: t.to_string(), kind: Kind::Deleted });
            o += 1;
        } else {
            flush(&mut rows, &mut dels, &mut adds);
            let t = l.strip_prefix(' ').unwrap_or(l);
            rows.push(Row::Pair(
                Some(Cell { num: o, text: t.to_string(), kind: Kind::Context }),
                Some(Cell { num: n, text: t.to_string(), kind: Kind::Context }),
            ));
            o += 1;
            n += 1;
        }
    }
    flush(&mut rows, &mut dels, &mut adds);
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(num: u32, text: &str, kind: Kind) -> Option<Cell> {
        Some(Cell { num, text: text.into(), kind })
    }

    #[test]
    fn context_faces_itself_and_changes_pair_up() {
        let t = "diff --git a/f b/f\nindex 1..2 100644\n--- a/f\n+++ b/f\n@@ -5,4 +5,5 @@ fn x\n keep\n-old1\n-old2\n+new1\n+new2\n+new3\n tail\n";
        let rows = side_by_side(t);
        assert_eq!(rows.len(), 4 + 1 + 1 + 3 + 1);
        assert_eq!(rows[4], Row::Full(Full::Hunk, "@@ -5,4 +5,5 @@ fn x".into()));
        assert_eq!(rows[5], Row::Pair(cell(5, "keep", Kind::Context), cell(5, "keep", Kind::Context)));
        assert_eq!(rows[6], Row::Pair(cell(6, "old1", Kind::Deleted), cell(6, "new1", Kind::Added)));
        assert_eq!(rows[7], Row::Pair(cell(7, "old2", Kind::Deleted), cell(7, "new2", Kind::Added)));
        assert_eq!(rows[8], Row::Pair(None, cell(8, "new3", Kind::Added)));
        assert_eq!(rows[9], Row::Pair(cell(8, "tail", Kind::Context), cell(9, "tail", Kind::Context)));
    }

    #[test]
    fn more_deletions_than_additions_pad_the_right() {
        let rows = side_by_side("@@ -1,3 +1,1 @@\n-a\n-b\n-c\n+z\n");
        // Before the first hunk header nothing is a line; here the hunk is the first row.
        assert_eq!(rows[1], Row::Pair(cell(1, "a", Kind::Deleted), cell(1, "z", Kind::Added)));
        assert_eq!(rows[2], Row::Pair(cell(2, "b", Kind::Deleted), None));
        assert_eq!(rows[3], Row::Pair(cell(3, "c", Kind::Deleted), None));
    }

    #[test]
    fn a_new_file_leaves_the_left_column_blank_and_a_deleted_one_the_right() {
        let new = side_by_side("diff --git a/f b/f\nnew file mode 100644\n--- /dev/null\n+++ b/f\n@@ -0,0 +1,2 @@\n+one\n+two\n");
        assert_eq!(new[5], Row::Pair(None, cell(1, "one", Kind::Added)));
        assert_eq!(new[6], Row::Pair(None, cell(2, "two", Kind::Added)));
        let gone = side_by_side("diff --git a/f b/f\ndeleted file mode 100644\n--- a/f\n+++ /dev/null\n@@ -1,1 +0,0 @@\n-one\n");
        assert_eq!(gone[5], Row::Pair(cell(1, "one", Kind::Deleted), None));
    }

    #[test]
    fn a_changed_line_that_looks_like_a_header_is_still_a_line() {
        let rows = side_by_side("@@ -1,1 +1,1 @@\n--- a comment\n+++ b comment\n");
        assert_eq!(rows[1], Row::Pair(cell(1, "-- a comment", Kind::Deleted), cell(1, "++ b comment", Kind::Added)));
    }

    #[test]
    fn the_no_newline_note_is_a_full_row_and_a_second_file_starts_over() {
        let rows = side_by_side("@@ -1 +1 @@\n-a\n\\ No newline at end of file\n+b\ndiff --git a/g b/g\n@@ -9 +9 @@\n x\n");
        assert_eq!(rows[1], Row::Pair(cell(1, "a", Kind::Deleted), None));
        assert!(matches!(rows[2], Row::Full(Full::Header, _)));
        assert_eq!(rows[3], Row::Pair(None, cell(1, "b", Kind::Added)));
        assert_eq!(rows[6], Row::Pair(cell(9, "x", Kind::Context), cell(9, "x", Kind::Context)));
        assert!(side_by_side("").is_empty());
    }
}
