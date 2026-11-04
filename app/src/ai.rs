//! The AI assistant: talks to Claude through the person's Claude Code sign-in (their
//! subscription), never through an API key. Claude only reads what the app sends and answers;
//! it has no tools, so it can't run anything. The app shows its advice and the person decides.
use crate::git::Repo;
use serde_json::{json, Value};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Most text sent in one request (diffs are cut beyond this).
pub const MAX_PROMPT_CHARS: usize = 60_000;

/// Where the Claude Code command line usually lives. Apps opened from the Finder get a short
/// PATH, so the usual places are checked too.
pub fn find_claude() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    // The original checks exactly these two, expanding "~" (sub_1003e513c, strings 0x1006476b0/d0).
    let mut candidates: Vec<PathBuf> = FORK_CLAUDE_PATHS
        .iter()
        .map(|p| match p.strip_prefix("~/") {
            Some(rest) => home.join(rest),
            None => PathBuf::from(p),
        })
        .collect();
    // Then the PATH and other usual installs (a choice of this rebuild).
    if let Some(p) = std::env::var_os("PATH") {
        candidates.extend(std::env::split_paths(&p).map(|d| d.join("claude")));
    }
    candidates.push(PathBuf::from("/usr/local/bin/claude"));
    candidates.push(home.join(".claude/local/claude"));
    candidates.into_iter().find(|p| p.is_file())
}

/// Where the original looks for Claude Code.
pub const FORK_CLAUDE_PATHS: [&str; 2] = ["/opt/homebrew/bin/claude", "~/.local/bin/claude"];
/// The original's message when neither path exists (sub_1003e4338).
pub const CANNOT_FIND_CLAUDE: &str = "Cannot find Claude";
/// The original's progress text while Claude writes a commit message (sub_1003e4338).
pub const GENERATING_WITH_CLAUDE: &str = "Generating with Claude…";
/// "pageGuideLinePosition" — read by the original, but its default wasn't found (estimated).
pub const PAGE_GUIDE_COLUMN: usize = 72;

/// One question to Claude. With a schema, returns the structured answer; otherwise the text.
pub fn ask(prompt: &str, system: &str, schema: Option<&Value>) -> Result<Value, String> {
    let exe = find_claude().ok_or(format!(
        "{CANNOT_FIND_CLAUDE}. Install Claude Code (claude.com/claude-code), then run `claude` once in Terminal and sign in with your subscription."
    ))?;
    let mut cmd = Command::new(exe);
    cmd.args([
        "-p",
        "--output-format",
        "json",
        "--tools",
        "",
        "--no-session-persistence",
        "--strict-mcp-config",
        "--setting-sources",
        "project",
        "--append-system-prompt",
        system,
    ]);
    if let Some(s) = schema {
        cmd.args(["--json-schema", &s.to_string()]);
    }
    // Use the subscription sign-in: hide any API key so Claude Code can't fall back to it.
    for k in ["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN", "CLAUDE_CODE_USE_BEDROCK", "CLAUDE_CODE_USE_VERTEX"] {
        cmd.env_remove(k);
    }
    cmd.current_dir(std::env::temp_dir()).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("Couldn't start Claude Code: {e}"))?;
    let mut text: String = prompt.chars().take(MAX_PROMPT_CHARS).collect();
    if prompt.chars().count() > MAX_PROMPT_CHARS {
        text.push_str("\n[…cut, too long]");
    }
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(text.as_bytes()).map_err(|e| format!("Couldn't send the question: {e}"))?;
    }
    let out = child.wait_with_output().map_err(|e| format!("Claude Code stopped: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: Value = match serde_json::from_str(stdout.trim()) {
        Ok(v) => v,
        Err(_) => {
            let err = String::from_utf8_lossy(&out.stderr);
            let msg = format!("{} {}", stdout.trim(), err.trim());
            return Err(friendly(&msg));
        }
    };
    if v.get("is_error").and_then(Value::as_bool).unwrap_or(false) {
        let msg = v.get("result").and_then(Value::as_str).unwrap_or("Claude couldn't answer").to_string();
        return Err(friendly(&msg));
    }
    if schema.is_some() {
        if let Some(s) = v.get("structured_output").filter(|s| !s.is_null()) {
            return Ok(s.clone());
        }
        // Fall back to reading the text answer as JSON.
        let r = v.get("result").and_then(Value::as_str).unwrap_or("");
        return serde_json::from_str(r).map_err(|_| "Claude's answer wasn't in the expected form. Try again.".to_string());
    }
    Ok(v.get("result").cloned().unwrap_or(Value::String(String::new())))
}

fn friendly(msg: &str) -> String {
    let m = msg.to_lowercase();
    if m.contains("login") || m.contains("log in") || m.contains("not authenticated") || m.contains("invalid api key") || m.contains("oauth") {
        "Claude Code isn't signed in. Open Terminal, run `claude`, and sign in with your Claude subscription.".into()
    } else if m.contains("usage limit") || m.contains("rate limit") {
        "Your Claude subscription has reached its usage limit for now. Try again later.".into()
    } else {
        let short: String = msg.trim().chars().take(300).collect();
        format!("Claude couldn't answer: {short}")
    }
}

// ------------------------------------------------------------------ branch clean-up

#[derive(Clone, Debug)]
pub struct BranchFacts {
    pub name: String,
    pub merged: bool,
    pub upstream: String,
    pub upstream_gone: bool,
    pub ahead: usize,
    pub behind: usize,
    pub last_date: String,
    pub days_old: i64,
    pub author: String,
    pub subject: String,
    /// Checked out in this linked worktree (it must be removed before the branch can go).
    pub worktree: Option<crate::worktree::Worktree>,
    pub worktree_dirty: bool,
}

#[derive(Clone, Debug)]
pub struct Advice {
    pub name: String,
    /// "delete", "keep" or "review"
    pub action: String,
    pub reason: String,
    pub merged: bool,
    pub selected: bool,
    /// The worktree that will be removed first, and whether it has uncommitted work.
    pub worktree: Option<crate::worktree::Worktree>,
    pub worktree_dirty: bool,
}

/// The branch everything is compared with: origin's default branch, else main/master/develop,
/// else the current branch.
pub fn base_branch(repo: &Repo) -> Option<String> {
    if let Ok(h) = repo.git(&["symbolic-ref", "--short", "-q", "refs/remotes/origin/HEAD"]) {
        if let Some(b) = h.trim().strip_prefix("origin/") {
            if repo.git(&["rev-parse", "-q", "--verify", &format!("refs/heads/{b}")]).is_ok() {
                return Some(b.to_string());
            }
        }
    }
    for b in ["main", "master", "develop", "trunk"] {
        if repo.git(&["rev-parse", "-q", "--verify", &format!("refs/heads/{b}")]).is_ok() {
            return Some(b.to_string());
        }
    }
    repo.current_branch()
}

pub fn branch_facts(repo: &Repo, base: &str) -> Vec<BranchFacts> {
    let current = repo.current_branch().unwrap_or_default();
    let wts: Vec<crate::worktree::Worktree> = repo.worktrees().into_iter().filter(|w| !w.is_main).collect();
    let merged: Vec<String> = repo
        .git(&["branch", "--format=%(refname:short)", "--merged", base])
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim().to_string())
        .collect();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let out = repo
        .git(&[
            "for-each-ref",
            "--format=%(refname:short)\t%(upstream:short)\t%(upstream:track)\t%(committerdate:unix)\t%(committerdate:short)\t%(authorname)\t%(subject)",
            "refs/heads",
        ])
        .unwrap_or_default();
    out.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.splitn(7, '\t').collect();
            if f.len() < 7 || f[0] == current || f[0] == base {
                return None;
            }
            let (ahead, behind) = repo
                .git(&["rev-list", "--left-right", "--count", &format!("{}...{}", f[0], base)])
                .ok()
                .and_then(|s| {
                    let mut p = s.split_whitespace();
                    Some((p.next()?.parse().ok()?, p.next()?.parse().ok()?))
                })
                .unwrap_or((0, 0));
            Some(BranchFacts {
                name: f[0].to_string(),
                merged: merged.iter().any(|m| m == f[0]),
                upstream: f[1].to_string(),
                upstream_gone: f[2].contains("gone"),
                ahead,
                behind,
                last_date: f[4].to_string(),
                days_old: (now - f[3].parse::<i64>().unwrap_or(now)) / 86_400,
                author: f[5].to_string(),
                subject: f[6].to_string(),
                worktree_dirty: wts.iter().find(|w| w.branch.as_deref() == Some(f[0])).map(|w| crate::worktree::is_dirty(&w.path)).unwrap_or(false),
                worktree: wts.iter().find(|w| w.branch.as_deref() == Some(f[0])).cloned(),
            })
        })
        .collect()
}

pub fn cleanup_prompt(base: &str, facts: &[BranchFacts]) -> String {
    let mut s = format!(
        "Here are the local branches of a Git repository, compared with its base branch `{base}`.\n\
         For each branch say whether to delete it, keep it, or have the person review it, with a short reason \
         (one sentence, plain words, for someone who doesn't code).\n\
         Rules: recommend delete for branches already merged into `{base}`, or whose remote branch is gone and that have no \
         unique commits. Recommend review (never delete) for branches with commits not in `{base}`. Keep recent active work.\n\n"
    );
    for f in facts {
        s.push_str(&format!(
            "- {}: merged into {base}: {}; commits not in {base}: {}; behind {base}: {}; remote: {}{}; last commit {} ({} days ago) by {}: \"{}\"{}\n",
            f.name,
            if f.merged { "yes" } else { "no" },
            f.ahead,
            f.behind,
            if f.upstream.is_empty() { "none".to_string() } else { f.upstream.clone() },
            if f.upstream_gone { " (deleted on the server)" } else { "" },
            f.last_date,
            f.days_old,
            f.author,
            f.subject,
            match &f.worktree {
                None => String::new(),
                Some(w) if w.missing => format!("; its worktree folder `{}` no longer exists", w.name()),
                Some(w) if f.worktree_dirty => format!("; checked out in worktree folder `{}` which has UNCOMMITTED changes", w.name()),
                Some(w) => format!("; checked out in worktree folder `{}` (no uncommitted changes; it will be removed with the branch)", w.name()),
            }
        ));
    }
    s
}

pub fn cleanup_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "branches": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "name": {"type": "string"},
                        "action": {"type": "string", "enum": ["delete", "keep", "review"]},
                        "reason": {"type": "string"}
                    },
                    "required": ["name", "action", "reason"]
                }
            },
            "summary": {"type": "string"}
        },
        "required": ["branches", "summary"]
    })
}

/// Turn Claude's answer into advice the app trusts: only branches that exist are kept, and a
/// branch with unique commits is never pre-ticked for deletion, whatever Claude said.
pub fn read_advice(v: &Value, facts: &[BranchFacts]) -> (Vec<Advice>, String) {
    let mut out = Vec::new();
    if let Some(list) = v.get("branches").and_then(Value::as_array) {
        for b in list {
            let name = b.get("name").and_then(Value::as_str).unwrap_or("").to_string();
            let Some(f) = facts.iter().find(|f| f.name == name) else { continue };
            let mut action = b.get("action").and_then(Value::as_str).unwrap_or("review").to_string();
            // Unique commits, or uncommitted work in its worktree: never pre-ticked.
            if action == "delete" && ((!f.merged && f.ahead > 0) || f.worktree_dirty) {
                action = "review".into();
            }
            let reason = b.get("reason").and_then(Value::as_str).unwrap_or("").to_string();
            out.push(Advice { selected: action == "delete", name, action, reason, merged: f.merged, worktree: f.worktree.clone(), worktree_dirty: f.worktree_dirty });
        }
    }
    // Branches Claude skipped are listed for review.
    for f in facts {
        if !out.iter().any(|a| a.name == f.name) {
            out.push(Advice {
                name: f.name.clone(),
                action: "review".into(),
                reason: "No advice given for this branch.".into(),
                merged: f.merged,
                selected: false,
                worktree: f.worktree.clone(),
                worktree_dirty: f.worktree_dirty,
            });
        }
    }
    let order = |a: &str| match a {
        "delete" => 0,
        "review" => 1,
        _ => 2,
    };
    out.sort_by_key(|a| order(&a.action));
    let summary = v.get("summary").and_then(Value::as_str).unwrap_or("").to_string();
    (out, summary)
}

// ------------------------------------------------------------------ writing help

/// The original's prompt, word for word (sub_1003e4338), with its limits filled in.
/// The original lets Claude read the repository itself; here Claude has no tools, so the
/// diff is added after it the way the original's OpenAI prompt does ("Below is my git diff",
/// sub_1004a7d4c).
pub fn commit_message_prompt(diff: &str, amend: bool, low: usize, high: usize, wrap: usize) -> String {
    let what = if amend { "staged changes combined with the last commit (i.e. `git diff --cached HEAD^`)" } else { "staged changes" };
    format!(
        "Generate commit message for {what}.\n\
         - Only respond with the commit message.\n\
         - Start directly with the subject line (no preamble).\n\
         - The header must be less than {low} (soft limit) and {high} (hard limit).\n\
         - Hard wrap lines at {wrap} characters.\n\
         - Don't use the 'generated' footer.\n\nBelow is my git diff:\n\n```\n{diff}\n```"
    )
}

/// The original splits Claude's text at the first line break: subject before it, description
/// after it, both trimmed of blank lines (sub_1003e4338, `range(of: "\n")`).
pub fn split_message(text: &str) -> (String, String) {
    let t = text.trim_matches(|c| c == '\n' || c == '\r');
    match t.split_once('\n') {
        Some((s, b)) => (s.trim().to_string(), b.trim_matches(|c| c == '\n' || c == '\r').trim().to_string()),
        None => (t.trim().to_string(), String::new()),
    }
}

pub fn explain_prompt(subject: &str, body: &str, diff: &str) -> String {
    format!(
        "Explain in plain words, for someone who doesn't code, what this commit changes and why it matters. \
         3 to 6 short sentences, no code.\n\nCommit: {subject}\n{body}\n\nChanges:\n{diff}"
    )
}

pub const SYSTEM: &str = "You are the assistant inside Fourchette, a Git app. You only see what the app sends you and you can't run anything. Answer briefly and in the language of the person's repository texts when obvious, else English.";

#[cfg(test)]
mod tests {
    use super::*;

    fn fact(name: &str, merged: bool, ahead: usize) -> BranchFacts {
        BranchFacts { name: name.into(), merged, upstream: String::new(), upstream_gone: false, ahead, behind: 0, last_date: "2026-01-01".into(), days_old: 300, author: "A".into(), subject: "s".into(), worktree: None, worktree_dirty: false }
    }

    #[test]
    fn dirty_worktree_is_never_preticked() {
        let mut f = fact("claude/task", true, 0);
        f.worktree = Some(crate::worktree::Worktree { path: "/tmp/x/task".into(), branch: Some("claude/task".into()), head: String::new(), is_main: false, missing: false, locked: false });
        f.worktree_dirty = true;
        let v = json!({"branches": [{"name": "claude/task", "action": "delete", "reason": "merged"}], "summary": ""});
        let (adv, _) = read_advice(&v, &[f.clone()]);
        assert_eq!(adv[0].action, "review");
        assert!(!adv[0].selected);
        assert!(adv[0].worktree.is_some());
        assert!(cleanup_prompt("main", &[f.clone()]).contains("worktree folder `task` which has UNCOMMITTED changes"));
        f.worktree_dirty = false;
        assert!(cleanup_prompt("main", &[f.clone()]).contains("it will be removed with the branch"));
        let (adv, _) = read_advice(&v, &[f]);
        assert_eq!(adv[0].action, "delete");
        assert!(adv[0].selected);
    }

    #[test]
    fn commit_message_rules() {
        let p = commit_message_prompt("DIFF", false, 50, 70, 72);
        assert!(p.starts_with("Generate commit message for staged changes.\n- Only respond with the commit message."));
        assert!(p.contains("- The header must be less than 50 (soft limit) and 70 (hard limit).\n- Hard wrap lines at 72 characters.\n- Don't use the 'generated' footer."));
        assert!(commit_message_prompt("D", true, 50, 70, 72).contains("staged changes combined with the last commit (i.e. `git diff --cached HEAD^`)"));
        assert_eq!(split_message("\nAdd login\n\nBecause users asked.\n"), ("Add login".to_string(), "Because users asked.".to_string()));
        assert_eq!(split_message("Fix typo"), ("Fix typo".to_string(), String::new()));
        assert_eq!(FORK_CLAUDE_PATHS, ["/opt/homebrew/bin/claude", "~/.local/bin/claude"]);
    }

    #[test]
    fn advice_is_kept_safe() {
        let facts = vec![fact("old", true, 0), fact("wip", false, 3), fact("other", false, 0)];
        let v = json!({"branches": [
            {"name": "old", "action": "delete", "reason": "merged"},
            {"name": "wip", "action": "delete", "reason": "looks old"},
            {"name": "ghost", "action": "delete", "reason": "does not exist"}
        ], "summary": "ok"});
        let (adv, summary) = read_advice(&v, &facts);
        assert_eq!(summary, "ok");
        assert_eq!(adv.len(), 3);
        let get = |n: &str| adv.iter().find(|a| a.name == n).unwrap();
        assert_eq!(get("old").action, "delete");
        assert!(get("old").selected);
        assert_eq!(get("wip").action, "review"); // unique commits: never pre-ticked
        assert!(!get("wip").selected);
        assert_eq!(get("other").action, "review"); // skipped by Claude
        assert!(adv.iter().all(|a| a.name != "ghost"));
    }
}

/// Talks to the real Claude through the subscription: run with `cargo test -- --ignored`.
#[cfg(test)]
mod live {
    use super::*;
    use crate::git::run_in;

    #[test]
    #[ignore]
    fn live_cleanup_advice() {
        let dir = std::env::temp_dir().join(format!("fourchette-ai-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let g = |a: &[&str]| run_in(&dir, a).unwrap();
        g(&["init", "-q", "-b", "main"]);
        g(&["config", "user.name", "T"]);
        g(&["config", "user.email", "t@example.com"]);
        std::fs::write(dir.join("a.txt"), "1\n").unwrap();
        g(&["add", "-A"]);
        g(&["commit", "-q", "-m", "first"]);
        g(&["checkout", "-q", "-b", "feature/done"]);
        std::fs::write(dir.join("b.txt"), "2\n").unwrap();
        g(&["add", "-A"]);
        g(&["commit", "-q", "-m", "finished feature"]);
        g(&["checkout", "-q", "main"]);
        g(&["merge", "-q", "--no-ff", "--no-edit", "feature/done"]);
        g(&["checkout", "-q", "-b", "experiment/wip"]);
        std::fs::write(dir.join("c.txt"), "3\n").unwrap();
        g(&["add", "-A"]);
        g(&["commit", "-q", "-m", "half-done experiment"]);
        g(&["checkout", "-q", "main"]);
        let repo = crate::git::Repo::open(&dir).unwrap();
        let base = base_branch(&repo).unwrap();
        assert_eq!(base, "main");
        let facts = branch_facts(&repo, &base);
        assert_eq!(facts.len(), 2);
        let v = ask(&cleanup_prompt(&base, &facts), SYSTEM, Some(&cleanup_schema())).expect("Claude answered");
        let (adv, summary) = read_advice(&v, &facts);
        println!("summary: {summary}");
        for a in &adv {
            println!("{} -> {} ({})", a.name, a.action, a.reason);
        }
        let get = |n: &str| adv.iter().find(|a| a.name == n).unwrap();
        assert_eq!(get("feature/done").action, "delete");
        assert!(get("experiment/wip").action != "delete");
        assert!(!get("experiment/wip").selected);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod live_commit {
    use super::*;

    #[test]
    #[ignore]
    fn live_commit_message() {
        let diff = "diff --git a/login.rs b/login.rs\n--- a/login.rs\n+++ b/login.rs\n@@ -1,3 +1,4 @@\n fn login() {\n-    let timeout = 5;\n+    // slow networks need more time\n+    let timeout = 30;\n }\n";
        let v = ask(&commit_message_prompt(diff, false, 50, 70, 72), SYSTEM, None).expect("Claude answered");
        let (s, b) = split_message(v.as_str().unwrap());
        println!("subject: {s}\nbody: {b}");
        assert!(!s.is_empty() && s.chars().count() <= 70);
        assert!(!s.contains("Generated"));
    }
}
