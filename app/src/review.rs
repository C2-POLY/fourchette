//! Claude's review of the changes ready to commit: it reads the staged diff, each line numbered as
//! in the staged file, and sends back remarks by file and line with their severity. Nothing is
//! written and the commit is never held back: the remarks are only shown.
use serde_json::{json, Value};

/// Most diff characters sent; the rest is cut (the instructions come first).
pub const MAX_DIFF_CHARS: usize = crate::ai::MAX_PROMPT_CHARS - 6_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warning,
    Suggestion,
}

impl Severity {
    pub const ALL: [Severity; 3] = [Severity::Error, Severity::Warning, Severity::Suggestion];

    fn key(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Suggestion => "suggestion",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Severity::Error => "Error",
            Severity::Warning => "Warning",
            Severity::Suggestion => "Suggestion",
        }
    }

    /// Unknown words count as a warning, the middle ground.
    fn read(s: &str) -> Severity {
        Severity::ALL.into_iter().find(|v| v.key() == s.trim().to_lowercase()).unwrap_or(Severity::Warning)
    }
}

/// One remark of Claude's.
#[derive(Clone, Debug, PartialEq)]
pub struct Remark {
    pub file: String,
    /// The line in the staged file (the new side of the diff); None for the file as a whole.
    pub line: Option<u32>,
    pub severity: Severity,
    pub message: String,
}

/// The staged diff with each line's number in the staged file in front (`  14 +`, `  15  `), and
/// `     -` for a removed line, so Claude doesn't count from the hunk headers.
pub fn numbered(diff: &str) -> String {
    let mut out = String::with_capacity(diff.len() + diff.len() / 4);
    let mut n = 0u32;
    let mut in_hunk = false;
    for l in diff.lines() {
        if l.starts_with("diff --git ") {
            in_hunk = false;
        }
        if let Some(rest) = l.strip_prefix("@@") {
            // `@@ -12,9 +12,12 @@`: the new side starts at 12.
            n = rest.split_whitespace().find_map(|t| t.strip_prefix('+')).and_then(|t| t.split(',').next()).and_then(|t| t.parse().ok()).unwrap_or(1);
            in_hunk = true;
            out.push_str(l);
        } else if !in_hunk || l.starts_with('\\') {
            out.push_str(l);
        } else if l.starts_with('-') {
            out.push_str(&format!("{:>6} {l}", ""));
        } else {
            out.push_str(&format!("{n:>6} {l}"));
            n += 1;
        }
        out.push('\n');
    }
    out
}

/// `text` cut to `max` characters, with a mark saying so.
fn cut(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut s: String = text.chars().take(max).collect();
    s.push_str("\n[…cut, too long]\n");
    s
}

/// The question for Claude. `amend`: the diff also holds the last commit, which this one replaces.
pub fn prompt(diff: &str, amend: bool) -> String {
    let what = if amend { "the changes staged to amend the last commit, shown together with that commit" } else { "the changes staged for the next commit" };
    format!(
        "Review {what}, as a careful colleague would before they are committed.\n\n\
         - Report only real problems: bugs, mistakes, missing cases, leftovers (debug prints, commented-out \
         code, secrets), and changes that don't fit together. No praise, no restating the change, no style \
         nitpicks a formatter would fix.\n\
         - Each remark has `file` (the path after `b/` in the diff), `line`, `severity` and `message`.\n\
         - `line`: the number printed in front of the diff line, which is its line in the staged file. For a \
         removed line, the number of the next line. 0 when the remark is about the whole file.\n\
         - `severity`: `error` (it will break or lose something), `warning` (likely wrong or risky) or \
         `suggestion` (worth improving, not wrong).\n\
         - `message`: one or two short sentences in English saying what is wrong and what to do.\n\
         - `summary`: one sentence on the whole change. With nothing to report, no remarks and say so.\n\
         Only say what the diff supports; never invent.\n\n\
         ## Staged diff\n{}",
        cut(&numbered(diff), MAX_DIFF_CHARS)
    )
}

pub fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "summary": { "type": "string" },
            "remarks": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "file": { "type": "string" },
                        "line": { "type": "integer" },
                        "severity": { "type": "string", "enum": ["error", "warning", "suggestion"] },
                        "message": { "type": "string" }
                    },
                    "required": ["file", "line", "severity", "message"]
                }
            }
        },
        "required": ["summary", "remarks"]
    })
}

/// Claude's summary and remarks, grouped by file in the order the files first appear, the most
/// severe first within a file, then by line. Remarks without a message are dropped.
pub fn read(v: &Value) -> (String, Vec<Remark>) {
    let summary = v["summary"].as_str().unwrap_or("").trim().to_string();
    let mut remarks: Vec<Remark> = v["remarks"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .filter_map(|r| {
            let message = r["message"].as_str()?.trim().to_string();
            let file = r["file"].as_str()?.trim().trim_start_matches("b/").to_string();
            if message.is_empty() || file.is_empty() {
                return None;
            }
            let line = r["line"].as_u64().and_then(|n| u32::try_from(n).ok()).filter(|n| *n > 0);
            Some(Remark { file, line, severity: Severity::read(r["severity"].as_str().unwrap_or("")), message })
        })
        .collect();
    let mut order: Vec<String> = Vec::new();
    for r in &remarks {
        if !order.contains(&r.file) {
            order.push(r.file.clone());
        }
    }
    remarks.sort_by_key(|r| (order.iter().position(|f| *f == r.file), r.severity, r.line));
    (summary, remarks)
}

/// The sample project's review, for the screenshot mode (on its staged `src/graph.rs`).
pub fn sample() -> (String, Vec<Remark>) {
    let r = |line: Option<u32>, severity, message: &str| Remark { file: "src/graph.rs".into(), line, severity, message: message.into() };
    (
        "Colours each branch line and draws its node; one undefined name will stop it from building.".into(),
        vec![
            r(Some(14), Severity::Error, "`row` isn't defined here: the function takes `rows`. Loop over `rows` and take the colour of each row inside the loop."),
            r(Some(15), Severity::Warning, "The curve radius 5.0 is hard-coded next to `lane_width()`: it won't follow a wider lane. Derive it from the width."),
            r(Some(19), Severity::Suggestion, "Drawing the node after the edges covers their ends; that looks intended, a short comment would say so."),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbered_gives_each_line_its_place_in_the_staged_file() {
        let d = crate::demo::diff("src/graph.rs", false);
        let n = numbered(&d);
        assert!(n.contains("\n@@ -12,9 +12,12 @@"));
        assert!(n.contains("\n    12      let width = lane_width();\n"));
        assert!(n.contains("\n       -    let colour = GREY;\n    13 +    // Each branch line"));
        assert!(n.contains("\n    19 +    draw_node(row.lane, colour);\n    20  }\n"));
        // Headers stay as they are, and a second file starts again from its own hunk.
        assert!(n.starts_with("diff --git a/src/graph.rs b/src/graph.rs\n--- a/src/graph.rs\n+++ b/src/graph.rs\n"));
        let two = format!("{d}diff --git a/x b/x\nnew file mode 100644\n--- /dev/null\n+++ b/x\n@@ -0,0 +1,2 @@\n+a\n+b\n\\ No newline at end of file\n");
        let n = numbered(&two);
        assert!(n.contains("+++ b/x\n@@ -0,0 +1,2 @@\n     1 +a\n     2 +b\n\\ No newline"));
    }

    #[test]
    fn the_prompt_carries_the_numbered_diff_cut_to_fit() {
        let p = prompt(&crate::demo::diff("a.rs", false), false);
        assert!(p.contains("the changes staged for the next commit"));
        assert!(p.contains("## Staged diff\ndiff --git a/a.rs b/a.rs\n"));
        assert!(p.contains("    13 +    // Each branch line"));
        assert!(prompt("x", true).contains("amend the last commit"));
        let long = format!("@@ -1 +1 @@\n{}", "+x\n".repeat(MAX_DIFF_CHARS));
        let p = prompt(&long, false);
        assert!(p.ends_with("[…cut, too long]\n"));
        assert!(p.chars().count() < crate::ai::MAX_PROMPT_CHARS, "the whole prompt fits");
    }

    #[test]
    fn remarks_are_grouped_by_file_most_severe_first() {
        let v = json!({"summary": " Fine. ", "remarks": [
            {"file": "b/a.rs", "line": 9, "severity": "suggestion", "message": "s"},
            {"file": "z.rs", "line": 3, "severity": "warning", "message": "w"},
            {"file": "a.rs", "line": 20, "severity": "error", "message": "e2"},
            {"file": "a.rs", "line": 4, "severity": "error", "message": "e1"},
            {"file": "a.rs", "line": 0, "severity": "odd", "message": "whole file"},
            {"file": "a.rs", "line": 1, "severity": "error", "message": "  "},
        ]});
        let (summary, r) = read(&v);
        assert_eq!(summary, "Fine.");
        let got: Vec<(&str, Option<u32>, Severity)> = r.iter().map(|r| (r.file.as_str(), r.line, r.severity)).collect();
        assert_eq!(
            got,
            vec![
                ("a.rs", Some(4), Severity::Error),
                ("a.rs", Some(20), Severity::Error),
                ("a.rs", None, Severity::Warning),
                ("a.rs", Some(9), Severity::Suggestion),
                ("z.rs", Some(3), Severity::Warning),
            ]
        );
        assert_eq!(read(&json!({})), (String::new(), vec![]));
    }

    /// Has Claude review a small staged change for real: `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn live_review() {
        let diff = "diff --git a/src/sum.rs b/src/sum.rs\n--- a/src/sum.rs\n+++ b/src/sum.rs\n@@ -1,6 +1,7 @@\n \
                    pub fn average(xs: &[f64]) -> f64 {\n-    if xs.is_empty() { return 0.0; }\n     let mut total = 0.0;\n     \
                    for x in xs {\n         total += x;\n     }\n+    println!(\"debug total {total}\");\n     total / xs.len() as f64\n";
        let v = crate::ai::ask(&prompt(diff, false), crate::ai::SYSTEM, Some(&schema()), Default::default()).unwrap();
        let (summary, remarks) = read(&v);
        println!("{summary}");
        for r in &remarks {
            println!("{}:{:?} {:?} {}", r.file, r.line, r.severity, r.message);
        }
        assert!(!remarks.is_empty() && remarks.iter().all(|r| r.file == "src/sum.rs"));
    }
}
