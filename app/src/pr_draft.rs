//! The pull request Claude drafts for the current branch: it reads the branch's diff against its
//! base branch, the commit subjects, and the issue the branch name carries (`feat/44-…` is #44),
//! and proposes a Conventional Commits title and an English body that closes the issue. Nothing is
//! sent to GitHub until the person clicks Create, which runs `gh pr create` with their text.
use crate::git::Repo;
use serde_json::{json, Value};

/// Most diff characters sent; the rest is cut (the issue, the subjects and the instructions come first).
pub const MAX_DIFF_CHARS: usize = crate::ai::MAX_PROMPT_CHARS - 12_000;
/// Most characters of the issue's body sent.
pub const MAX_ISSUE_CHARS: usize = 6_000;
/// Most commit subjects sent; the oldest beyond are only counted.
pub const MAX_COMMITS: usize = 100;

/// The issue the branch closes.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Issue {
    pub number: u64,
    pub title: String,
    pub body: String,
}

/// What Claude reads about the branch.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Branch {
    pub name: String,
    pub base: String,
    /// Commit subjects from base to the branch, newest first.
    pub commits: Vec<String>,
    /// Commits beyond `MAX_COMMITS`, only counted.
    pub more: usize,
    /// `git diff --stat` of the branch against the merge base.
    pub stat: String,
    pub diff: String,
}

/// The issue number a branch name carries: its first token made only of digits
/// (`feat/44-draft-pr` and `44-x` give 44, `fix_7` gives 7, `v2-x` and `main` none).
pub fn issue_number(branch: &str) -> Option<u64> {
    branch.split(|c: char| !c.is_alphanumeric()).filter(|t| !t.is_empty() && t.chars().all(|c| c.is_ascii_digit())).find_map(|t| t.parse().ok().filter(|n| *n > 0))
}

/// The branch's commits and diff against `base`, from their merge base.
pub fn read_branch(repo: &Repo, name: &str, base: &str) -> Branch {
    let range = format!("{base}..{name}");
    let since = format!("{base}...{name}");
    let max = format!("-n{MAX_COMMITS}");
    let commits: Vec<String> = repo.git(&["log", "--no-merges", "--format=%s", &max, &range, "--"]).unwrap_or_default().lines().map(str::to_string).collect();
    let total: usize = repo.git(&["rev-list", "--no-merges", "--count", &range, "--"]).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(commits.len());
    Branch {
        name: name.to_string(),
        base: base.to_string(),
        more: total.saturating_sub(commits.len()),
        commits,
        stat: repo.git(&["diff", "--stat", "-M", &since, "--"]).unwrap_or_default(),
        diff: repo.git(&["diff", "-M", &since, "--"]).unwrap_or_default(),
    }
}

/// Reads issue `n` of the repository through `gh`. Blocks: call it from a thread.
pub fn read_issue(slug: &str, n: u64) -> Result<Issue, String> {
    let exe = crate::github::find_gh().ok_or("The GitHub command line (gh) isn't installed.")?;
    let out = crate::github::gh(&exe, &["issue", "view", &n.to_string(), "-R", slug, "--json", "number,title,body"])?;
    let v: Value = serde_json::from_str(&out).map_err(|_| "GitHub's answer wasn't in the expected form.".to_string())?;
    Ok(Issue { number: v["number"].as_u64().unwrap_or(n), title: v["title"].as_str().unwrap_or("").to_string(), body: v["body"].as_str().unwrap_or("").to_string() })
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

/// The question for Claude. Without an issue, the body closes nothing.
pub fn prompt(b: &Branch, issue: Option<&Issue>) -> String {
    let mut s = format!(
        "Write the pull request that merges the branch `{}` into `{}`.\n\n\
         - `title`: one line in Conventional Commits form (`type(scope): subject`, type among feat, fix, docs, \
         style, refactor, perf, test, build, ci, chore, revert), imperative, lowercase after the colon, no final \
         period, under 72 characters.\n\
         - `body`: Markdown in English, whatever language the issue is in. Start with what the change does and \
         why, in a short paragraph or a few bullets, then how it was tested if the diff shows tests. No heading \
         for the title, no mention of AI.\n",
        b.name, b.base
    );
    match issue {
        Some(i) => s.push_str(&format!("- End the body with the line `Closes #{}`.\n", i.number)),
        None => s.push_str("- No issue is linked: close none.\n"),
    }
    s.push_str("Only say what the diff, the commits and the issue support; never invent.\n");
    if let Some(i) = issue {
        s.push_str(&format!("\n## Issue #{}: {}\n{}\n", i.number, i.title, cut(i.body.trim(), MAX_ISSUE_CHARS)));
    }
    s.push_str("\n## Commits, newest first\n");
    for c in &b.commits {
        s.push_str(&format!("- {c}\n"));
    }
    if b.more > 0 {
        s.push_str(&format!("- …and {} older commits\n", b.more));
    }
    s.push_str(&format!("\n## Files changed\n{}\n## Diff\n{}", b.stat.trim_end(), cut(&b.diff, MAX_DIFF_CHARS)));
    s
}

pub fn schema() -> Value {
    json!({
        "type": "object",
        "properties": { "title": { "type": "string" }, "body": { "type": "string" } },
        "required": ["title", "body"]
    })
}

/// Claude's title and body; the body ends with `Closes #n` when there is an issue, even if Claude
/// left it out.
pub fn read_draft(v: &Value, issue: Option<u64>) -> Result<(String, String), String> {
    let title = v["title"].as_str().unwrap_or("").lines().next().unwrap_or("").trim().to_string();
    if title.is_empty() {
        return Err("Claude's answer had no title. Try again.".into());
    }
    let mut body = v["body"].as_str().unwrap_or("").trim().to_string();
    if let Some(n) = issue {
        body = closes(&body, n);
    }
    Ok((title, body))
}

/// `body` with a last line `Closes #n`, unless it already closes that issue.
pub fn closes(body: &str, n: u64) -> String {
    let line = format!("Closes #{n}");
    let already = body.lines().any(|l| {
        let l = l.trim().to_lowercase();
        ["closes", "fixes", "resolves"].iter().any(|w| l.strip_prefix(w).is_some_and(|rest| rest.trim() == format!("#{n}")))
    });
    match (already, body.trim().is_empty()) {
        (true, _) => body.to_string(),
        (false, true) => line,
        (false, false) => format!("{}\n\n{line}", body.trim_end()),
    }
}

/// The title and body as one text to paste.
pub fn as_text(title: &str, body: &str) -> String {
    format!("{}\n\n{}\n", title.trim(), body.trim())
}

/// Opens the pull request of `head` into `base` on GitHub and returns its address. Blocks: call it
/// from a thread.
pub fn create(slug: &str, base: &str, head: &str, title: &str, body: &str) -> Result<String, String> {
    let exe = crate::github::find_gh().ok_or("The GitHub command line (gh) isn't installed.")?;
    let file = std::env::temp_dir().join(format!("fourchette-pr-{}.md", std::process::id()));
    std::fs::write(&file, body).map_err(|e| format!("Couldn't prepare the description: {e}"))?;
    let res = crate::github::gh(&exe, &["pr", "create", "-R", slug, "--base", base, "--head", head, "--title", title, "--body-file", &file.to_string_lossy()]);
    let _ = std::fs::remove_file(&file);
    let out = res?;
    Ok(out.lines().rev().find(|l| l.contains("/pull/")).unwrap_or(out.trim()).trim().to_string())
}

/// The sample project's draft, for the screenshot mode.
pub fn sample() -> (Branch, Issue, String, String) {
    let branch = Branch {
        name: "feature/44-search-by-author".into(),
        base: "main".into(),
        commits: vec!["Match the author's e-mail too".into(), "Search commits by author".into()],
        more: 0,
        stat: " src/search.rs | 42 +++++++++++++++---\n src/history.rs |  9 ++--\n 2 files changed, 41 insertions(+), 10 deletions(-)\n".into(),
        diff: String::new(),
    };
    let issue = Issue { number: 44, title: "Search commits by author".into(), body: String::new() };
    let title = "feat(search): find commits by author name or e-mail".to_string();
    let body = "Adds the author to the commit search: typing a name or an e-mail address in the history's search \
                field now keeps the commits written by that person, alongside the matches on the message and the hash.\n\n\
                - The search matches the author's name and e-mail, ignoring case.\n\
                - The history highlights the author column of the matching rows.\n\n\
                Tested with the new unit tests in `search.rs`.\n\n\
                Closes #44"
        .to_string();
    (branch, issue, title, body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn the_issue_number_is_the_first_number_token() {
        assert_eq!(issue_number("feat/44-draft-pr"), Some(44));
        assert_eq!(issue_number("claude/44-draft-pr-5c2fb8"), Some(44));
        assert_eq!(issue_number("51-foo"), Some(51));
        assert_eq!(issue_number("fix_7"), Some(7));
        assert_eq!(issue_number("v2-x"), None);
        assert_eq!(issue_number("release/0"), None);
        assert_eq!(issue_number("main"), None);
    }

    #[test]
    fn the_prompt_carries_the_issue_the_commits_and_a_cut_diff() {
        let b = Branch { name: "feat/44-x".into(), base: "main".into(), commits: vec!["add x".into()], more: 3, stat: " a.rs | 2 +-\n".into(), diff: "d".repeat(MAX_DIFF_CHARS + 10) };
        let i = Issue { number: 44, title: "Assistant : rédiger la PR".into(), body: "**En tant que** dev".into() };
        let p = prompt(&b, Some(&i));
        assert!(p.contains("`feat/44-x` into `main`"));
        assert!(p.contains("End the body with the line `Closes #44`"));
        assert!(p.contains("## Issue #44: Assistant : rédiger la PR\n**En tant que** dev"));
        assert!(p.contains("- add x\n- …and 3 older commits"));
        assert!(p.contains(" a.rs | 2 +-\n## Diff\n"));
        assert!(p.ends_with("[…cut, too long]\n"));
        assert!(p.chars().count() < crate::ai::MAX_PROMPT_CHARS, "the whole prompt fits");
        let p = prompt(&Branch { diff: "small".into(), ..b }, None);
        assert!(p.contains("No issue is linked") && !p.contains("Closes #"));
        assert!(p.ends_with("## Diff\nsmall"));
    }

    #[test]
    fn the_draft_closes_its_issue_once() {
        let v = json!({"title": " feat(x): add y \nmore", "body": "Adds y.\n"});
        assert_eq!(read_draft(&v, Some(44)).unwrap(), ("feat(x): add y".to_string(), "Adds y.\n\nCloses #44".to_string()));
        assert_eq!(read_draft(&v, None).unwrap().1, "Adds y.");
        let v = json!({"title": "fix: z", "body": "Fixes z.\n\nfixes #44"});
        assert_eq!(read_draft(&v, Some(44)).unwrap().1, "Fixes z.\n\nfixes #44");
        assert_eq!(closes("Closes #440", 44), "Closes #440\n\nCloses #44");
        assert_eq!(closes("", 7), "Closes #7");
        assert!(read_draft(&json!({"title": "", "body": "x"}), None).is_err());
    }

    #[test]
    fn read_branch_compares_with_the_base() {
        let dir = std::env::temp_dir().join(format!("fourchette-pr-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&dir).unwrap();
        let g = |a: &[&str]| crate::git::run_in(&dir, a).unwrap();
        g(&["init", "-q", "-b", "main"]);
        g(&["config", "user.email", "t@example.com"]);
        g(&["config", "user.name", "T"]);
        g(&["commit", "-q", "--allow-empty", "-m", "first"]);
        g(&["checkout", "-q", "-b", "feat/3-x"]);
        std::fs::write(dir.join("x.txt"), "hello\n").unwrap();
        g(&["add", "x.txt"]);
        g(&["commit", "-q", "-m", "feat: add x"]);
        g(&["checkout", "-q", "main"]);
        g(&["commit", "-q", "--allow-empty", "-m", "on main"]);
        let b = read_branch(&Repo { root: dir.clone() }, "feat/3-x", "main");
        assert_eq!(b.commits, vec!["feat: add x".to_string()]);
        assert!(b.stat.contains("x.txt") && b.diff.contains("+hello"));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Reads issue #44 and has Claude draft this branch's pull request for real, without creating
    /// it: `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn live_draft() {
        let r = Repo::open(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        let issue = read_issue("C2-POLY/fourchette", 44).unwrap();
        assert!(issue.title.contains("PR"), "{issue:?}");
        let b = read_branch(&r, "HEAD", "origin/main");
        let v = crate::ai::ask(&prompt(&b, Some(&issue)), crate::ai::SYSTEM, Some(&schema()), Default::default()).unwrap();
        let (title, body) = read_draft(&v, Some(44)).unwrap();
        println!("{title}\n\n{body}");
        assert!(title.contains(':') && body.ends_with("Closes #44"));
    }
}
