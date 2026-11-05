//! Claude's resolution of a file left with conflict markers by a merge, a rebase, a cherry-pick
//! or a revert. Claude reads the file and why each side changed it (the subjects of the commits
//! that touch it), and proposes the whole file without markers. Nothing is written until the
//! person applies it, which takes a Snapshot first: Undo puts the markers back, and the conflict
//! too when the file was marked resolved.
use crate::git::Repo;
use serde_json::{json, Value};

/// A conflicted file longer than this is left to the person (Claude would have to send it back whole).
pub const MAX_FILE_CHARS: usize = 40_000;
/// Commit subjects read per side.
pub const MAX_SUBJECTS: usize = 15;
/// Claude's answers asked at the same time.
pub const WORKERS: usize = 3;

/// One conflicted file, as Claude gets it.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Conflict {
    pub path: String,
    /// The working copy, with its conflict markers.
    pub text: String,
    /// "your branch main", "the commit being applied"…
    pub ours: String,
    pub theirs: String,
    /// Why each side changed the file: commit subjects, newest first.
    pub ours_why: Vec<String>,
    pub theirs_why: Vec<String>,
}

/// Claude's proposal for one file.
#[derive(Clone, Debug, PartialEq)]
pub struct Proposal {
    pub text: String,
    pub explanation: String,
    /// The proposal against the conflicted file, as a unified diff.
    pub patch: String,
}

/// The two sides of the operation in progress: (ours, theirs, how to name each).
fn sides(repo: &Repo, op: &str) -> (String, Option<String>, String, String) {
    let branch = repo.current_branch();
    let name = |r: &str| repo.git(&["name-rev", "--name-only", "--always", r]).map(|s| s.trim().to_string()).unwrap_or_else(|_| r.to_string());
    let here = branch.clone().map(|b| format!("your branch {b}")).unwrap_or_else(|| "the checked-out commit".into());
    match op {
        "merge" => ("HEAD".into(), Some("MERGE_HEAD".into()), here, format!("the merged branch {}", name("MERGE_HEAD"))),
        // A rebase replays your commits onto the other branch: HEAD is that branch, the commit
        // being replayed is yours.
        "rebase" => ("HEAD".into(), Some("REBASE_HEAD".into()), "the branch you rebase onto".into(), "your commit being replayed".into()),
        "cherry-pick" => ("HEAD".into(), Some("CHERRY_PICK_HEAD".into()), here, "the commit being cherry-picked".into()),
        "revert" => ("HEAD".into(), Some("REVERT_HEAD".into()), here, "the revert of a commit".into()),
        _ => ("HEAD".into(), None, here, "the changes being applied".into()),
    }
}

/// What Claude needs for `path`, or why it can't help with it.
pub fn read(repo: &Repo, path: &str) -> Result<Conflict, String> {
    let bytes = std::fs::read(repo.root.join(path)).map_err(|_| format!("{path} was deleted on one side: keep or delete it yourself."))?;
    let text = String::from_utf8(bytes).map_err(|_| format!("{path} is a binary file: pick a side yourself."))?;
    if text.contains('\0') {
        return Err(format!("{path} is a binary file: pick a side yourself."));
    }
    if !has_markers(&text) {
        return Err(format!("{path} has no conflict markers left."));
    }
    if text.chars().count() > MAX_FILE_CHARS {
        return Err(format!("{path} is too long for Claude to send back whole ({} characters).", text.chars().count()));
    }
    let op = repo.operation().unwrap_or_default();
    let (ours_ref, theirs_ref, ours, theirs) = sides(repo, &op);
    let subjects = |args: &[&str]| -> Vec<String> { repo.git(args).unwrap_or_default().lines().filter(|l| !l.trim().is_empty()).map(str::to_string).collect() };
    let n = format!("-n{MAX_SUBJECTS}");
    let (ours_why, theirs_why) = match &theirs_ref {
        Some(t) if op == "merge" => {
            let base = repo.git(&["merge-base", &ours_ref, t]).map(|s| s.trim().to_string()).unwrap_or_default();
            if base.is_empty() {
                (vec![], vec![])
            } else {
                (subjects(&["log", "--format=%s", &n, &format!("{base}..{ours_ref}"), "--", path]), subjects(&["log", "--format=%s", &n, &format!("{base}..{t}"), "--", path]))
            }
        }
        // One commit is applied: its message is the reason; HEAD's side is its recent history.
        Some(t) => (subjects(&["log", "--format=%s", &n, &ours_ref, "--", path]), subjects(&["log", "-1", "--format=%s%n%b", t])),
        None => (subjects(&["log", "--format=%s", &n, &ours_ref, "--", path]), vec![]),
    };
    Ok(Conflict { path: path.to_string(), text, ours, theirs, ours_why, theirs_why })
}

/// Whether the text still has a conflict block.
pub fn has_markers(text: &str) -> bool {
    let mut open = false;
    for l in text.lines() {
        if l.starts_with("<<<<<<<") {
            open = true;
        } else if open && l.starts_with(">>>>>>>") {
            return true;
        }
    }
    false
}

pub fn prompt(c: &Conflict) -> String {
    let why = |v: &[String]| if v.is_empty() { "  (no commit message found)\n".to_string() } else { v.iter().fold(String::new(), |acc, s| acc + "  - " + s + "\n") };
    format!(
        "A Git operation stopped on conflicts in `{path}`. Resolve them.\n\n\
         The first part of each conflict block (after `<<<<<<<`) is {ours}; the part after `=======` is {theirs} \
         (a `|||||||` part, when there is one, is the common original).\n\
         Why {ours} changed this file:\n{ow}\
         Why {theirs} changed this file:\n{tw}\n\
         Return in `resolved` the whole file with every conflict resolved and no conflict marker left, keeping \
         everything outside the conflict blocks exactly as it is. Keep both sides' intentions when they can live \
         together; when they can't, prefer the one the messages make more recent or deliberate. In `explanation`, \
         say in one to three sentences what you kept from each side, in the language of the commit messages, and \
         say plainly if a choice is a guess the person should check.\n\n\
         The file:\n{text}",
        path = c.path,
        ours = c.ours,
        theirs = c.theirs,
        ow = why(&c.ours_why),
        tw = why(&c.theirs_why),
        text = c.text,
    )
}

pub fn schema() -> Value {
    json!({
        "type": "object",
        "properties": { "resolved": { "type": "string" }, "explanation": { "type": "string" } },
        "required": ["resolved", "explanation"]
    })
}

/// Claude's answer for `c`, checked: no marker left, the file's last newline kept.
pub fn read_answer(v: &Value, c: &Conflict) -> Result<(String, String), String> {
    let mut text = v["resolved"].as_str().unwrap_or("").to_string();
    let explanation = v["explanation"].as_str().unwrap_or("").trim().to_string();
    if text.trim().is_empty() {
        return Err("Claude sent back an empty file. Try again.".into());
    }
    if has_markers(&text) {
        return Err("Claude left conflict markers in its answer. Try again, or resolve this file yourself.".into());
    }
    if c.text.ends_with('\n') && !text.ends_with('\n') {
        text.push('\n');
    }
    Ok((text, explanation))
}

/// `new` against `old` as a unified diff (`git diff --no-index`), without its file headers.
pub fn patch(old: &str, new: &str) -> String {
    let dir = std::env::temp_dir().join(format!("fourchette-resolve-{}-{}", std::process::id(), seq()));
    let _ = std::fs::create_dir_all(&dir);
    let (a, b) = (dir.join("conflicted"), dir.join("proposed"));
    let _ = std::fs::write(&a, old);
    let _ = std::fs::write(&b, new);
    let out = std::process::Command::new("git").args(["diff", "--no-index", "--no-color", "-U3", "--"]).arg(&a).arg(&b).output();
    let _ = std::fs::remove_dir_all(&dir);
    let text = out.map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    text.lines().skip_while(|l| !l.starts_with("@@")).collect::<Vec<_>>().join("\n")
}

fn seq() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    N.fetch_add(1, Ordering::Relaxed)
}

/// Asks Claude for `c`. Blocks: call it from a thread.
pub fn propose(c: &Conflict, model: crate::settings::ClaudeModel) -> Result<Proposal, String> {
    let v = crate::ai::ask(&prompt(c), crate::ai::SYSTEM, Some(&schema()), model)?;
    let (text, explanation) = read_answer(&v, c)?;
    Ok(Proposal { patch: patch(&c.text, &text), text, explanation })
}

/// Writes Claude's `text` over the conflicted file, and with `resolve` stages it, which marks it
/// resolved. Returns the Undo step: the conflicted file goes to the Trash first (Undo brings it
/// back over the new one), and a staged file is put back in conflict (`update-index --unresolve`).
pub fn apply(repo: &Repo, path: &str, text: &str, resolve: bool) -> Result<crate::undo::Snapshot, String> {
    let label = if resolve { format!("Resolve {path} with Claude") } else { format!("Apply Claude's resolution to {path}") };
    let mut snap = repo.snapshot(&label, false, false);
    let full = repo.root.join(path);
    snap.trashed = Some((path.to_string(), crate::git::trash(&repo.root, path)?));
    if let Err(e) = std::fs::write(&full, text) {
        if let Some((_, in_trash)) = &snap.trashed {
            let _ = std::fs::rename(in_trash, &full);
        }
        return Err(format!("Couldn't write {path}: {e}"));
    }
    if resolve {
        repo.git(&["add", "--", path])?;
        snap.resolved.push(path.to_string());
    }
    Ok(snap)
}

/// The sample project's conflicts, for the screenshot mode: one proposal ready, one on its way.
pub fn sample() -> Vec<(String, Option<Result<Proposal, String>>)> {
    let new = "fn timeout() -> Duration {\n    Duration::from_secs(settings().timeout.max(30))\n}\n";
    let ready = Proposal {
        text: new.into(),
        explanation: "Kept the setting from hotfix/login-timeout and your 30-second value as the minimum, so slow networks still get at least 30 seconds.".into(),
        patch: "@@ -1,7 +1,3 @@\n fn timeout() -> Duration {\n-<<<<<<< HEAD\n-    Duration::from_secs(30)\n-=======\n-    Duration::from_secs(settings().timeout)\n->>>>>>> hotfix/login-timeout\n+    Duration::from_secs(settings().timeout.max(30))\n }".into(),
    };
    vec![("src/net/login.rs".into(), Some(Ok(ready))), ("src/settings.rs".into(), None), ("assets/logo.png".into(), Some(Err("assets/logo.png is a binary file: pick a side yourself.".into())))]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn markers_need_an_opened_and_closed_block() {
        assert!(has_markers("a\n<<<<<<< HEAD\nx\n=======\ny\n>>>>>>> b\n"));
        assert!(!has_markers("a\n<<<<<<< HEAD\nx\n"));
        assert!(!has_markers(">>>>>>> b\n<<<<<<< HEAD\n"));
        assert!(!has_markers("plain\n"));
    }

    #[test]
    fn an_answer_with_markers_or_empty_is_refused_and_the_last_newline_kept() {
        let c = Conflict { text: "a\n".into(), ..Default::default() };
        assert!(read_answer(&json!({"resolved": "<<<<<<< a\nx\n=======\ny\n>>>>>>> b", "explanation": ""}), &c).is_err());
        assert!(read_answer(&json!({"resolved": "  ", "explanation": ""}), &c).is_err());
        assert_eq!(read_answer(&json!({"resolved": "b", "explanation": " why "}), &c).unwrap(), ("b\n".to_string(), "why".to_string()));
    }

    #[test]
    fn the_prompt_names_both_sides_and_their_reasons() {
        let c = Conflict { path: "f.txt".into(), text: "T".into(), ours: "your branch main".into(), theirs: "the merged branch topic".into(), ours_why: vec!["fix: a".into()], theirs_why: vec![] };
        let p = prompt(&c);
        assert!(p.contains("`<<<<<<<`) is your branch main; the part after `=======` is the merged branch topic"));
        assert!(p.contains("Why your branch main changed this file:\n  - fix: a\n"));
        assert!(p.contains("Why the merged branch topic changed this file:\n  (no commit message found)\n"));
        assert!(p.ends_with("The file:\nT"));
    }

    #[test]
    fn the_patch_shows_the_markers_removed() {
        let p = patch("a\n<<<<<<< HEAD\nx\n=======\ny\n>>>>>>> b\nz\n", "a\nxy\nz\n");
        assert!(p.starts_with("@@"), "{p}");
        assert!(p.contains("\n-<<<<<<< HEAD") && p.contains("\n+xy"), "{p}");
    }

    /// A repository stopped on a merge conflict in f.txt.
    fn conflicted() -> (PathBuf, Repo) {
        let dir = std::env::temp_dir().join(format!("fourchette-resolve-test-{}-{}", std::process::id(), seq()));
        std::fs::create_dir_all(&dir).unwrap();
        let g = |a: &[&str]| crate::git::run_in(&dir, a).unwrap();
        g(&["init", "-q", "-b", "main"]);
        g(&["config", "user.email", "t@example.com"]);
        g(&["config", "user.name", "T"]);
        std::fs::write(dir.join("f.txt"), "one\n").unwrap();
        g(&["add", "."]);
        g(&["commit", "-q", "-m", "first"]);
        g(&["checkout", "-q", "-b", "topic"]);
        std::fs::write(dir.join("f.txt"), "two\n").unwrap();
        g(&["commit", "-q", "-am", "feat: say two"]);
        g(&["checkout", "-q", "main"]);
        std::fs::write(dir.join("f.txt"), "three\n").unwrap();
        g(&["commit", "-q", "-am", "feat: say three"]);
        let _ = crate::git::run_in(&dir, &["merge", "topic"]);
        (dir.clone(), Repo { root: dir })
    }

    #[test]
    fn read_gives_both_sides_of_a_merge() {
        let (dir, r) = conflicted();
        let c = read(&r, "f.txt").unwrap();
        assert!(has_markers(&c.text));
        assert_eq!((c.ours.as_str(), c.ours_why.clone()), ("your branch main", vec!["feat: say three".to_string()]));
        assert!(c.theirs.starts_with("the merged branch topic"), "{}", c.theirs);
        assert_eq!(c.theirs_why, vec!["feat: say two".to_string()]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn applying_and_resolving_is_undone_back_to_the_conflict() {
        let (dir, r) = conflicted();
        let before = std::fs::read_to_string(dir.join("f.txt")).unwrap();
        // Applied only: the file changes, the conflict stays.
        let snap = apply(&r, "f.txt", "two and three\n", false).unwrap();
        assert_eq!(r.conflicted_files(), ["f.txt"]);
        r.undo(&snap).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("f.txt")).unwrap(), before);
        // Marked resolved: the conflict is gone, and Undo brings it back with the markers.
        let snap = apply(&r, "f.txt", "two and three\n", true).unwrap();
        assert!(r.conflicted_files().is_empty());
        r.undo(&snap).unwrap();
        assert_eq!(r.conflicted_files(), ["f.txt"]);
        assert_eq!(std::fs::read_to_string(dir.join("f.txt")).unwrap(), before);
        assert_eq!(r.operation().as_deref(), Some("merge"));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Calls Claude for real: `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn live_resolve() {
        let (dir, r) = conflicted();
        let p = propose(&read(&r, "f.txt").unwrap(), Default::default()).unwrap();
        println!("{}\n{}\n{}", p.explanation, p.text, p.patch);
        assert!(!has_markers(&p.text) && p.patch.contains("-<<<<<<<"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
