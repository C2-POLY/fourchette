//! Talks to the `git` command line, the way the original does.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const SEP: &str = "~#."; // field separator used by the original (sub_100399eec)

#[derive(Clone, Debug)]
pub struct Commit {
    pub hash: String,
    pub parents: Vec<String>,
    pub author: String,
    pub email: String,
    pub time: i64,
    pub date: String,
    pub subject: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RefKind {
    Head,
    Local,
    Remote,
    Tag,
    Stash,
}

#[derive(Clone, Debug)]
pub struct GitRef {
    pub name: String, // short name, e.g. "main", "origin/main", "v1.0"
    pub full: String,
    pub kind: RefKind,
    pub target: String,
    pub is_head: bool,
    pub upstream: String,
}

#[derive(Clone, Debug)]
pub struct FileChange {
    pub path: String,
    pub code: char,
}

#[derive(Clone, Debug, Default)]
pub struct Status {
    pub staged: Vec<FileChange>,
    pub unstaged: Vec<FileChange>,
}

impl Status {
    pub fn is_empty(&self) -> bool {
        self.staged.is_empty() && self.unstaged.is_empty()
    }
}

#[derive(Clone, Debug, Default)]
pub struct CommitDetails {
    pub hash: String,
    pub author: String,
    pub email: String,
    pub author_time: i64,
    pub committer: String,
    pub committer_email: String,
    pub committer_time: i64,
    pub parents: Vec<String>,
    pub subject: String,
    pub body: String,
    pub files: Vec<FileChange>,
}

#[derive(Clone, Debug)]
pub struct Stash {
    pub name: String,
    pub message: String,
}

pub struct Repo {
    pub root: PathBuf,
}

// ---- Rules read from the original (see MECHANICS.md) ----
/// Retries when another git holds the index lock (DiscardFilesGitCommand, sub_100053aac).
pub const LOCK_RETRIES: u32 = 3;
/// Wait before retry n is n * 0.5 s ("Retrying in %.1f", ucvtf #1 in sub_100053aac).
pub const LOCK_RETRY_STEP_SECS: f32 = 0.5;
pub const LOCK_MESSAGE: &str = "index.lock': File exists.";
/// Default "maxCommitCount" (settings table, sub_10031b7cc).
pub const MAX_COMMIT_COUNT: usize = 100_000;
/// Default diff "contextSize" (sub_10031b7cc); the person's choice is `Settings::diff_context`.
pub const DIFF_CONTEXT: u32 = 3;

/// Runs git and returns (stdout, stderr). Retries while the repository is locked.
pub fn exec(dir: &Path, args: &[&str]) -> Result<(String, String), String> {
    let mut attempt = 0;
    loop {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_EDITOR", "true")
            .env("LC_ALL", "C")
            .output()
            .map_err(|e| format!("could not start git: {e}"))?;
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        if out.status.success() {
            return Ok((stdout, stderr));
        }
        if stderr.contains(LOCK_MESSAGE) {
            if attempt < LOCK_RETRIES {
                attempt += 1;
                std::thread::sleep(std::time::Duration::from_secs_f32(LOCK_RETRY_STEP_SECS * attempt as f32));
                continue;
            }
            return Err("The repository is locked by another Git process (index.lock exists).".into());
        }
        let err = stderr.trim().to_string();
        let msg = if err.is_empty() { stdout.trim().to_string() } else { err };
        return Err(if msg.is_empty() { format!("git {} failed", args.join(" ")) } else { msg });
    }
}

pub fn run_in(dir: &Path, args: &[&str]) -> Result<String, String> {
    exec(dir, args).map(|(o, _)| o)
}

/// `fetch --prune [--tags] <remote>|--all --progress` (sub_1000c56b8).
pub fn fetch_args(remote: Option<&str>, all_tags: bool) -> Vec<String> {
    let mut a = vec!["fetch".to_string(), "--prune".into()];
    if all_tags {
        a.push("--tags".into());
    }
    a.push(remote.map(String::from).unwrap_or_else(|| "--all".into()));
    a.push("--progress".into());
    a
}

/// "Already up to date" / "Fetched N branch(es)" (sub_1000c56b8).
pub fn fetch_summary(stderr: &str) -> String {
    let n = stderr.lines().filter(|l| l.contains("->") && !l.contains("[up to date]")).count();
    match n {
        0 => "Already up to date".into(),
        1 => "Fetched 1 branch".into(),
        n => format!("Fetched {n} branches"),
    }
}

/// `-c push.default=upstream push <remote> <local>:refs/heads/<remote branch>
///  [--force-with-lease] [--set-upstream] --verbose --progress` (sub_10016e1ec).
pub fn push_args(remote: &str, local: &str, remote_branch: &str, set_upstream: bool, force: bool) -> Vec<String> {
    let mut a = vec![
        "-c".to_string(),
        "push.default=upstream".into(),
        "push".into(),
        remote.into(),
        format!("{local}:refs/heads/{remote_branch}"),
    ];
    if force {
        a.push("--force-with-lease".into());
    }
    if set_upstream {
        a.push("--set-upstream".into());
    }
    a.push("--verbose".into());
    a.push("--progress".into());
    a
}

pub fn push_summary(out: &str, branch: &str, remote: &str) -> String {
    if out.contains("Everything up-to-date") {
        "Everything is up-to-date".into()
    } else {
        format!("Pushed {branch} to {remote}")
    }
}

/// Move an untracked file to the Trash instead of deleting it (sub_100053aac).
pub fn trash(root: &Path, path: &str) -> Result<PathBuf, String> {
    let src = root.join(path);
    let home = std::env::var_os("HOME").ok_or("no home folder")?;
    let trash = PathBuf::from(home).join(".Trash");
    let name = src.file_name().ok_or("bad path")?.to_string_lossy().into_owned();
    let mut dst = trash.join(&name);
    let mut k = 1;
    while dst.exists() {
        dst = trash.join(format!("{name} {k}"));
        k += 1;
    }
    std::fs::rename(&src, &dst).map_err(|e| format!("Couldn't move {path} to the Trash: {e}"))?;
    Ok(dst)
}

pub fn find_root(dir: &Path) -> Option<PathBuf> {
    run_in(dir, &["rev-parse", "--show-toplevel"]).ok().map(|s| PathBuf::from(s.trim()))
}

impl Repo {
    pub fn open(path: &Path) -> Result<Repo, String> {
        match find_root(path) {
            Some(root) => Ok(Repo { root }),
            None => Err(format!("{} is not a Git repository", path.display())),
        }
    }

    pub fn name(&self) -> String {
        self.root.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "repository".into())
    }

    pub fn git(&self, args: &[&str]) -> Result<String, String> {
        run_in(&self.root, args)
    }

    pub fn commits(&self, limit: usize) -> Vec<Commit> {
        let fmt = format!("--pretty=format:%H{SEP}%P{SEP}%aN{SEP}%aE{SEP}%at{SEP}%ad{SEP}%s");
        let n = format!("-n{limit}");
        let out = self
            .git(&["log", "--branches", "--remotes", "--tags", "HEAD", "--topo-order", &n, "--date=format:%d %b %Y, %H:%M", &fmt])
            .or_else(|_| self.git(&["log", "--branches", "--remotes", "--tags", "--topo-order", &n, "--date=format:%d %b %Y, %H:%M", &fmt]))
            .unwrap_or_default();
        out.lines()
            .filter_map(|l| {
                let f: Vec<&str> = l.splitn(7, SEP).collect();
                if f.len() < 7 {
                    return None;
                }
                Some(Commit {
                    hash: f[0].to_string(),
                    parents: f[1].split_whitespace().map(String::from).collect(),
                    author: f[2].to_string(),
                    email: f[3].to_string(),
                    time: f[4].parse().unwrap_or(0),
                    date: f[5].to_string(),
                    subject: f[6].to_string(),
                })
            })
            .collect()
    }

    pub fn refs(&self) -> Vec<GitRef> {
        // Same fields as the original's for-each-ref format (sub_100296878).
        let out = self
            .git(&["for-each-ref", "--format=%(objectname)\t%(refname)\t%(HEAD)\t%(upstream:short)\t%(*objectname)"])
            .unwrap_or_default();
        let mut refs = Vec::new();
        for l in out.lines() {
            let f: Vec<&str> = l.split('\t').collect();
            if f.len() < 5 {
                continue;
            }
            let full = f[1].to_string();
            let target = if f[4].is_empty() { f[0] } else { f[4] }.to_string();
            let (kind, name) = if let Some(n) = full.strip_prefix("refs/heads/") {
                (RefKind::Local, n.to_string())
            } else if let Some(n) = full.strip_prefix("refs/remotes/") {
                if n.ends_with("/HEAD") {
                    continue;
                }
                (RefKind::Remote, n.to_string())
            } else if let Some(n) = full.strip_prefix("refs/tags/") {
                (RefKind::Tag, n.to_string())
            } else if full == "refs/stash" {
                (RefKind::Stash, "stash".to_string())
            } else {
                continue;
            };
            refs.push(GitRef { name, full, kind, target, is_head: f[2] == "*", upstream: f[3].to_string() });
        }
        if !refs.iter().any(|r| r.is_head) {
            if let Ok(h) = self.git(&["rev-parse", "HEAD"]) {
                refs.push(GitRef {
                    name: "HEAD".into(),
                    full: "HEAD".into(),
                    kind: RefKind::Head,
                    target: h.trim().to_string(),
                    is_head: true,
                    upstream: String::new(),
                });
            }
        }
        refs
    }

    pub fn stashes(&self) -> Vec<Stash> {
        self.git(&["stash", "list", "--format=%gd\t%s"])
            .unwrap_or_default()
            .lines()
            .filter_map(|l| l.split_once('\t').map(|(a, b)| Stash { name: a.into(), message: b.into() }))
            .collect()
    }

    pub fn status(&self) -> Status {
        let out = // Same arguments as the original's status refresh (sub_1003ab2d8).
        self.git(&["--no-optional-locks", "status", "--porcelain", "--renames", "--untracked-files=all", "-z"]).unwrap_or_default();
        let mut st = Status::default();
        let mut it = out.split('\0');
        while let Some(entry) = it.next() {
            if entry.len() < 4 {
                continue;
            }
            let x = entry.as_bytes()[0] as char;
            let y = entry.as_bytes()[1] as char;
            let path = entry[3..].to_string();
            if x == 'R' || x == 'C' {
                it.next(); // original path
            }
            if x == '?' {
                st.unstaged.push(FileChange { path, code: '?' });
                continue;
            }
            if x == 'U' || y == 'U' || (x == 'A' && y == 'A') || (x == 'D' && y == 'D') {
                st.unstaged.push(FileChange { path, code: 'U' });
                continue;
            }
            if x != ' ' {
                st.staged.push(FileChange { path: path.clone(), code: x });
            }
            if y != ' ' {
                st.unstaged.push(FileChange { path, code: y });
            }
        }
        st
    }

    pub fn details(&self, hash: &str) -> Option<CommitDetails> {
        // The original's own format string (sub_100399eec).
        let fmt = format!("--pretty=format:%H{SEP}%aN{SEP}%aE{SEP}%at{SEP}%cN{SEP}%cE{SEP}%ct{SEP}%P{SEP}%B");
        let out = self.git(&["show", "--no-patch", &fmt, hash]).ok()?;
        let f: Vec<&str> = out.splitn(9, SEP).collect();
        if f.len() < 9 {
            return None;
        }
        let msg = f[8].trim_end();
        let (subject, body) = match msg.split_once('\n') {
            Some((s, b)) => (s.to_string(), b.trim().to_string()),
            None => (msg.to_string(), String::new()),
        };
        let files = self
            .git(&["diff-tree", "-r", "-M", "--name-status", "--root", "--no-commit-id", "-m", "--first-parent", hash])
            .unwrap_or_default()
            .lines()
            .filter_map(|l| {
                let mut p = l.split('\t');
                let code = p.next()?.chars().next()?;
                let path = p.last()?.to_string();
                Some(FileChange { path, code })
            })
            .collect();
        Some(CommitDetails {
            hash: f[0].into(),
            author: f[1].into(),
            email: f[2].into(),
            author_time: f[3].parse().unwrap_or(0),
            committer: f[4].into(),
            committer_email: f[5].into(),
            committer_time: f[6].parse().unwrap_or(0),
            parents: f[7].split_whitespace().map(String::from).collect(),
            subject,
            body,
            files,
        })
    }

    pub fn diff_commit_file(&self, hash: &str, path: &str, context: u32) -> String {
        self.git(&["show", "--format=", "-M", "--first-parent", &format!("--unified={context}"), hash, "--", path]).unwrap_or_else(|e| e)
    }

    pub fn diff_worktree_file(&self, path: &str, staged: bool, untracked: bool, context: u32) -> String {
        if untracked {
            // `git diff --no-index` exits with 1 when files differ, so read stdout directly.
            let out = Command::new("git")
                .args(["diff", "--no-index", &format!("--unified={context}"), "--", "/dev/null", path])
                .current_dir(&self.root)
                .output();
            return out.map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
        }
        if staged {
            self.git(&["diff", "--cached", "-M", &format!("--unified={context}"), "--", path]).unwrap_or_else(|e| e)
        } else {
            self.git(&["diff", &format!("--unified={context}"), "--", path]).unwrap_or_else(|e| e)
        }
    }

    /// Remote to fetch from / push to: the current branch's upstream remote, else "origin",
    /// else the first remote (the original's Fetch sheet fetches one remote by default,
    /// "fetchSheetFetchAllRemotes" = false, sub_10031b7cc / sub_10045e7d4).
    pub fn default_remote(&self) -> Option<String> {
        if let Ok(up) = self.git(&["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{upstream}"]) {
            if let Some((r, _)) = up.trim().split_once('/') {
                return Some(r.to_string());
            }
        }
        let remotes: Vec<String> = self.git(&["remote"]).unwrap_or_default().lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
        if remotes.iter().any(|r| r == "origin") {
            Some("origin".into())
        } else {
            remotes.into_iter().next()
        }
    }

    /// The upstream as (remote, branch), if the current branch has one.
    pub fn upstream(&self) -> Option<(String, String)> {
        let up = self.git(&["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{upstream}"]).ok()?;
        let (r, b) = up.trim().split_once('/')?;
        Some((r.to_string(), b.to_string()))
    }

    /// Commit the way the original does: the message goes in a temporary file passed with
    /// `--file=` (CommitGitCommand, sub_100318828). Returns the original's result text.
    pub fn commit(&self, message: &str, amend: bool, files: usize) -> Result<String, String> {
        let tmp = std::env::temp_dir().join(format!("fourchette-commit-{}.txt", std::process::id()));
        std::fs::write(&tmp, message).map_err(|e| format!("Couldn't write the commit message: {e}"))?;
        let file_arg = format!("--file={}", tmp.display());
        let mut args = vec!["commit"];
        if amend {
            args.push("--amend");
        }
        args.push(&file_arg);
        let res = self.git(&args);
        let _ = std::fs::remove_file(&tmp);
        res?;
        let branch = self.current_branch().unwrap_or_else(|| "HEAD".into());
        let sha = self.git(&["rev-parse", "--short", "HEAD"]).unwrap_or_default().trim().to_string();
        let noun = if files == 1 { "file" } else { "files" };
        Ok(if amend {
            format!("Amended commit with {files} {noun} in {branch}")
        } else {
            format!("Committed {files} {noun} to {branch} ({sha})")
        })
    }

    pub fn current_branch(&self) -> Option<String> {
        self.git(&["symbolic-ref", "--short", "-q", "HEAD"]).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
    }

    pub fn ahead_behind(&self) -> Option<(usize, usize)> {
        let out = self.git(&["rev-list", "--left-right", "--count", "HEAD...@{upstream}"]).ok()?;
        let mut p = out.split_whitespace();
        Some((p.next()?.parse().ok()?, p.next()?.parse().ok()?))
    }
}

/// "3 hours ago" style text.
pub fn relative_time(t: i64) -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let d = (now - t).max(0);
    let (n, unit) = if d < 60 {
        return "just now".into();
    } else if d < 3600 {
        (d / 60, "minute")
    } else if d < 86400 {
        (d / 3600, "hour")
    } else if d < 86400 * 30 {
        (d / 86400, "day")
    } else if d < 86400 * 365 {
        (d / (86400 * 30), "month")
    } else {
        (d / (86400 * 365), "year")
    };
    format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" })
}

pub fn refs_by_target(refs: &[GitRef]) -> HashMap<String, Vec<GitRef>> {
    let mut m: HashMap<String, Vec<GitRef>> = HashMap::new();
    for r in refs {
        if r.kind == RefKind::Stash {
            continue;
        }
        m.entry(r.target.clone()).or_default().push(r.clone());
    }
    for v in m.values_mut() {
        v.sort_by_key(|r| match r.kind {
            RefKind::Head => 0,
            RefKind::Local => if r.is_head { 0 } else { 1 },
            RefKind::Remote => 2,
            RefKind::Tag => 3,
            RefKind::Stash => 4,
        });
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_small_repository() {
        let dir = std::env::temp_dir().join(format!("fourchette-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let g = |a: &[&str]| run_in(&dir, a).unwrap();
        g(&["init", "-q", "-b", "main"]);
        g(&["config", "user.name", "Test Person"]);
        g(&["config", "user.email", "t@example.com"]);
        std::fs::write(dir.join("a.txt"), "one\n").unwrap();
        g(&["add", "-A"]);
        g(&["commit", "-q", "-m", "first"]);
        g(&["checkout", "-q", "-b", "side"]);
        std::fs::write(dir.join("b.txt"), "two\n").unwrap();
        g(&["add", "-A"]);
        g(&["commit", "-q", "-m", "side work", "-m", "with a body"]);
        g(&["checkout", "-q", "main"]);
        std::fs::write(dir.join("a.txt"), "one\nmore\n").unwrap();
        g(&["commit", "-q", "-am", "main work"]);
        g(&["merge", "-q", "--no-edit", "side"]);

        let repo = Repo::open(&dir).unwrap();
        let commits = repo.commits(100);
        assert_eq!(commits.len(), 4);
        assert_eq!(commits[0].parents.len(), 2);
        let (rows, width) = crate::graph::layout(&commits);
        assert_eq!(rows.len(), 4);
        assert!(width >= 2);
        assert!(rows[0].is_merge);

        let refs = repo.refs();
        assert!(refs.iter().any(|r| r.name == "main" && r.is_head));
        assert!(refs.iter().any(|r| r.name == "side" && r.kind == RefKind::Local));
        assert_eq!(repo.current_branch().as_deref(), Some("main"));

        let side = commits.iter().find(|c| c.subject == "side work").unwrap();
        let d = repo.details(&side.hash).unwrap();
        assert_eq!(d.body, "with a body");
        assert_eq!(d.files.len(), 1);
        assert_eq!(d.files[0].path, "b.txt");
        assert!(repo.diff_commit_file(&side.hash, "b.txt", DIFF_CONTEXT).contains("+two"));

        // Working copy: one modified (staged), one untracked, one deleted (unstaged).
        std::fs::write(dir.join("a.txt"), "changed\n").unwrap();
        g(&["add", "a.txt"]);
        std::fs::write(dir.join("new.txt"), "x\n").unwrap();
        std::fs::remove_file(dir.join("b.txt")).unwrap();
        let st = repo.status();
        assert!(st.staged.iter().any(|f| f.path == "a.txt" && f.code == 'M'));
        assert!(st.unstaged.iter().any(|f| f.path == "new.txt" && f.code == '?'));
        assert!(st.unstaged.iter().any(|f| f.path == "b.txt" && f.code == 'D'));
        assert!(repo.diff_worktree_file("new.txt", false, true, DIFF_CONTEXT).contains("+x"));
        assert!(repo.diff_worktree_file("a.txt", true, false, DIFF_CONTEXT).contains("+changed"));

        // Stage everything (as "Stage all" does), commit, unstage path.
        g(&["add", "-A"]);
        assert!(repo.status().unstaged.is_empty());
        g(&["reset", "-q", "--", "new.txt"]);
        assert!(repo.status().unstaged.iter().any(|f| f.path == "new.txt"));
        g(&["add", "-A", "--", "new.txt"]);
        let files = repo.status().staged.len();
        let done = repo.commit("wip\n\ndesc\n", false, files).unwrap();
        assert!(done.starts_with(&format!("Committed {files} files to main (")), "{done}");
        assert!(repo.status().is_empty());
        assert_eq!(repo.commits(100).len(), 5);
        let d = repo.details("HEAD").unwrap();
        assert_eq!((d.subject.as_str(), d.body.as_str()), ("wip", "desc"));
        let amended = repo.commit("wip again\n", true, 0).unwrap();
        assert_eq!(amended, "Amended commit with 0 files in main");

        // Push and fetch with the original's commands against a local bare remote.
        let bare = dir.with_extension("remote.git");
        run_in(&dir, &["init", "-q", "--bare", bare.to_str().unwrap()]).unwrap();
        g(&["remote", "add", "origin", bare.to_str().unwrap()]);
        assert_eq!(repo.default_remote().as_deref(), Some("origin"));
        assert!(repo.upstream().is_none());
        let args = push_args("origin", "main", "main", true, false);
        let a: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let (o, e) = exec(&dir, &a).unwrap();
        assert_eq!(push_summary(&format!("{o}{e}"), "main", "origin"), "Pushed main to origin");
        assert_eq!(repo.upstream(), Some(("origin".to_string(), "main".to_string())));
        let (o, e) = exec(&dir, &a).unwrap();
        assert_eq!(push_summary(&format!("{o}{e}"), "main", "origin"), "Everything is up-to-date");
        let fa = fetch_args(Some("origin"), false);
        assert_eq!(fa, ["fetch", "--prune", "origin", "--progress"]);
        let f: Vec<&str> = fa.iter().map(|s| s.as_str()).collect();
        let (_, e) = exec(&dir, &f).unwrap();
        assert_eq!(fetch_summary(&e), "Already up to date");
        assert_eq!(fetch_summary(" * [new branch]  dev -> origin/dev\n   1a..2b  main -> origin/main\n"), "Fetched 2 branches");
        let _ = std::fs::remove_dir_all(&bare);
        g(&["stash", "list"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
