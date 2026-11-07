//! The state of every repository of the list, for the switcher (⌘O): its uncommitted changes,
//! how far it is ahead of and behind its upstream, its open pull requests (and those waiting
//! for my review), and "Fetch all". Everything runs in background threads, a few repositories
//! at a time, and comes back through one channel polled each frame; what was read is kept, so
//! the switcher shows it at once and only reads again what is older than `LOCAL_TTL` or
//! `PULLS_TTL`.
use crate::github::{find_gh, slug};
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Repositories read or fetched at the same time.
pub const WORKERS: usize = 4;
/// How long a repository's changes and ahead/behind are kept before being read again.
pub const LOCAL_TTL: Duration = Duration::from_secs(60);
/// How long the pull request counts are kept before GitHub is asked again.
pub const PULLS_TTL: Duration = Duration::from_secs(300);
/// Repositories asked about in one GraphQL query.
const PULLS_BATCH: usize = 40;

/// What `git status` says of a repository.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Local {
    /// Files changed, staged, untracked or in conflict.
    pub changes: usize,
    pub ahead: usize,
    pub behind: usize,
}

/// The open pull requests of a GitHub repository.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pulls {
    pub open: usize,
    /// Open ones where my review is requested.
    pub review: usize,
    /// "owner/name".
    pub slug: String,
}

impl Pulls {
    pub fn url(&self) -> String {
        format!("https://github.com/{}/pulls", self.slug)
    }

    pub fn review_url(&self) -> String {
        format!("https://github.com/{}/pulls?q=is%3Apr+is%3Aopen+review-requested%3A%40me", self.slug)
    }
}

/// Where "Fetch all" stands.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FetchAll {
    pub total: usize,
    pub done: usize,
    /// The repositories whose fetch failed, with Git's message.
    pub failures: Vec<(PathBuf, String)>,
}

impl FetchAll {
    pub fn running(&self) -> bool {
        self.done < self.total
    }
}

enum Msg {
    Local(PathBuf, Option<Local>),
    Pulls(HashMap<PathBuf, Pulls>),
    Fetched(PathBuf, Result<(), String>),
}

pub struct States {
    local: HashMap<PathBuf, (Instant, Local)>,
    /// Being read, so not asked again.
    pending: HashSet<PathBuf>,
    pulls: HashMap<PathBuf, Pulls>,
    pulls_at: Option<Instant>,
    pulls_pending: bool,
    pub fetch: Option<FetchAll>,
    /// The last fetch error of each repository, until it fetches again.
    fetch_errors: HashMap<PathBuf, String>,
    /// When `ensure` last ran: a longer gap means the switcher was opened again.
    seen: Option<Instant>,
    /// Screenshots: sample states, nothing read, and these paths count as missing.
    pub sample: Option<Vec<PathBuf>>,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
}

impl Default for States {
    fn default() -> Self {
        let (tx, rx) = channel();
        States {
            local: HashMap::new(),
            pending: HashSet::new(),
            pulls: HashMap::new(),
            pulls_at: None,
            pulls_pending: false,
            fetch: None,
            fetch_errors: HashMap::new(),
            seen: None,
            sample: None,
            tx,
            rx,
        }
    }
}

impl States {
    pub fn local(&self, p: &Path) -> Option<Local> {
        self.local.get(p).map(|(_, l)| *l)
    }

    pub fn pulls(&self, p: &Path) -> Option<&Pulls> {
        self.pulls.get(p)
    }

    pub fn fetch_error(&self, p: &Path) -> Option<&str> {
        self.fetch_errors.get(p).map(|s| s.as_str())
    }

    /// Screenshots: the sample paths that count as missing.
    pub fn sample_missing(&self) -> Option<&[PathBuf]> {
        self.sample.as_deref()
    }

    pub fn missing(&self, p: &Path) -> bool {
        match &self.sample {
            Some(gone) => gone.iter().any(|g| g == p),
            None => !p.exists(),
        }
    }

    /// Called each frame the switcher is open: reads in the background what is not known or is
    /// older than its TTL. On opening, the repositories open in a tab (`fresh`) are read again
    /// whatever their age, since they are the ones being worked on.
    pub fn ensure(&mut self, paths: &[PathBuf], fresh: &[PathBuf], ctx: &egui::Context) {
        if self.sample.is_some() {
            return;
        }
        let now = Instant::now();
        let reopened = self.seen.is_none_or(|t| now - t > Duration::from_secs(1));
        self.seen = Some(now);
        let fetching = self.fetch.as_ref().is_some_and(|f| f.running());
        let stale: Vec<PathBuf> = paths
            .iter()
            .filter(|p| !self.pending.contains(*p) && p.exists())
            .filter(|p| (reopened && fresh.contains(p)) || self.local.get(*p).is_none_or(|(t, _)| now - *t > LOCAL_TTL))
            .cloned()
            .collect();
        // While "Fetch all" runs, each repository is read again after its fetch.
        if !stale.is_empty() && !fetching {
            self.pending.extend(stale.iter().cloned());
            pool(stale, self.tx.clone(), ctx, |p| vec![Msg::Local(p.to_path_buf(), read_local(p))]);
        }
        if !self.pulls_pending && self.pulls_at.is_none_or(|t| now - t > PULLS_TTL) {
            self.load_pulls(paths, ctx);
        }
    }

    fn load_pulls(&mut self, paths: &[PathBuf], ctx: &egui::Context) {
        let paths: Vec<PathBuf> = paths.iter().filter(|p| p.exists()).cloned().collect();
        if paths.is_empty() {
            return;
        }
        self.pulls_pending = true;
        let (tx, ctx) = (self.tx.clone(), ctx.clone());
        std::thread::spawn(move || {
            let _ = tx.send(Msg::Pulls(read_pulls(&paths)));
            ctx.request_repaint();
        });
    }

    /// Fetches every repository of the list (`git fetch --all --prune`), `WORKERS` at a time,
    /// then reads its state again. Folders that are gone are left out.
    pub fn fetch_all(&mut self, paths: &[PathBuf], ctx: &egui::Context) -> Result<(), &'static str> {
        if self.sample.is_some() {
            return Err("This is the sample list — open Fourchette on your own repositories to fetch them.");
        }
        if self.fetch.as_ref().is_some_and(|f| f.running()) {
            return Err("Fetch all is already running.");
        }
        let paths: Vec<PathBuf> = paths.iter().filter(|p| p.exists()).cloned().collect();
        if paths.is_empty() {
            return Err("There is no repository to fetch.");
        }
        self.fetch = Some(FetchAll { total: paths.len(), ..Default::default() });
        self.pending.extend(paths.iter().cloned());
        pool(paths, self.tx.clone(), ctx, |p| {
            let args = crate::git::fetch_args(None, false);
            let a: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            let r = crate::git::exec(p, &a).map(|_| ());
            vec![Msg::Fetched(p.to_path_buf(), r), Msg::Local(p.to_path_buf(), read_local(p))]
        });
        // The pull requests may have moved too.
        self.pulls_at = None;
        Ok(())
    }

    /// Takes what the threads sent. Returns "Fetch all" once it has just finished.
    pub fn poll(&mut self) -> Option<FetchAll> {
        let mut finished = None;
        while let Ok(m) = self.rx.try_recv() {
            match m {
                Msg::Local(p, l) => {
                    self.pending.remove(&p);
                    match l {
                        Some(l) => {
                            self.local.insert(p, (Instant::now(), l));
                        }
                        None => {
                            self.local.remove(&p);
                        }
                    }
                }
                Msg::Pulls(m) => {
                    self.pulls = m;
                    self.pulls_at = Some(Instant::now());
                    self.pulls_pending = false;
                }
                Msg::Fetched(p, r) => {
                    match r {
                        Ok(()) => {
                            self.fetch_errors.remove(&p);
                        }
                        Err(e) => {
                            self.fetch_errors.insert(p.clone(), e.clone());
                            if let Some(f) = &mut self.fetch {
                                f.failures.push((p, e));
                            }
                        }
                    }
                    if let Some(f) = &mut self.fetch {
                        f.done += 1;
                        if !f.running() {
                            finished = Some(f.clone());
                        }
                    }
                }
            }
        }
        finished
    }

    /// Screenshots (FOURCHETTE_SWITCHER): states for the sample list, `fetch` "fetching" or
    /// "fetched" to show Fetch all under way or finished with failures.
    pub fn sample(entries: &[PathBuf], missing: Vec<PathBuf>, fetch: &str) -> States {
        let mut s = States { sample: Some(missing), ..Default::default() };
        let at = Instant::now();
        let locals = [(3, 2, 0), (0, 0, 4), (0, 0, 0), (12, 0, 0), (1, 1, 1), (0, 0, 0), (0, 5, 0), (2, 0, 0)];
        let pulls = [(4, 1), (0, 0), (2, 0), (0, 0), (7, 3), (1, 0), (0, 0), (0, 0)];
        for (k, p) in entries.iter().enumerate() {
            let (changes, ahead, behind) = locals[k % locals.len()];
            s.local.insert(p.clone(), (at, Local { changes, ahead, behind }));
            let (open, review) = pulls[k % pulls.len()];
            if open > 0 {
                s.pulls.insert(p.clone(), Pulls { open, review, slug: "me/sample".into() });
            }
        }
        let failed = entries.get(3).map(|p| (p.clone(), "fatal: unable to access 'https://gitlab.example.com/web/shop.git/': Could not resolve host: gitlab.example.com".to_string()));
        match fetch {
            "fetching" => s.fetch = Some(FetchAll { total: entries.len(), done: entries.len() / 2, failures: failed.into_iter().collect() }),
            "fetched" => {
                let mut failures: Vec<(PathBuf, String)> = failed.into_iter().collect();
                if let Some(p) = entries.get(6) {
                    failures.push((p.clone(), "Permission denied (publickey).\nfatal: Could not read from remote repository.".into()));
                }
                s.fetch = Some(FetchAll { total: entries.len(), done: entries.len(), failures });
            }
            _ => {}
        }
        for (p, e) in s.fetch.iter().flat_map(|f| f.failures.iter()) {
            s.fetch_errors.insert(p.clone(), e.clone());
        }
        s
    }
}

/// Runs `job` on each path, `WORKERS` threads taking them in turn from one queue.
fn pool(paths: Vec<PathBuf>, tx: Sender<Msg>, ctx: &egui::Context, job: fn(&Path) -> Vec<Msg>) {
    let n = paths.len().min(WORKERS);
    let queue = Arc::new(Mutex::new(paths.into_iter().collect::<VecDeque<_>>()));
    for _ in 0..n {
        let (queue, tx, ctx) = (queue.clone(), tx.clone(), ctx.clone());
        // The guard goes with the closure that pops: the queue is free while the job runs.
        std::thread::spawn(move || {
            while let Some(p) = queue.lock().ok().and_then(|mut q| q.pop_front()) {
                for m in job(&p) {
                    let _ = tx.send(m);
                }
                ctx.request_repaint();
            }
        });
    }
}

/// One `git status`, without taking the index lock (the repository may be open in a tab).
fn read_local(p: &Path) -> Option<Local> {
    crate::git::run_in(p, &["--no-optional-locks", "status", "--porcelain=v2", "--branch"]).ok().map(|s| parse_status(&s))
}

/// From `git status --porcelain=v2 --branch`: "# branch.ab +1 -2", then one line per file.
pub fn parse_status(s: &str) -> Local {
    let mut l = Local::default();
    for line in s.lines() {
        if let Some(ab) = line.strip_prefix("# branch.ab ") {
            let mut it = ab.split_whitespace();
            l.ahead = it.next().and_then(|a| a.trim_start_matches('+').parse().ok()).unwrap_or(0);
            l.behind = it.next().and_then(|b| b.trim_start_matches('-').parse().ok()).unwrap_or(0);
        } else if !line.starts_with('#') && !line.starts_with('!') && !line.is_empty() {
            l.changes += 1;
        }
    }
    l
}

/// The GitHub repository a folder works with: its "origin", else its first remote on GitHub.
fn github_slug(p: &Path) -> Option<String> {
    let out = crate::git::run_in(p, &["config", "--get-regexp", r"^remote\..*\.url$"]).ok()?;
    let remotes: Vec<(&str, &str)> = out.lines().filter_map(|l| l.split_once(' ')).collect();
    let origin = remotes.iter().find(|(k, _)| *k == "remote.origin.url").and_then(|(_, u)| slug(u));
    origin.or_else(|| remotes.iter().find_map(|(_, u)| slug(u))).filter(|s| safe_slug(s))
}

/// Only the characters GitHub allows in names, since the slug goes into the query's text.
fn safe_slug(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'))
}

/// The GraphQL query for these repositories: for each one, its open pull requests and a search
/// for those that request my review.
pub fn pulls_query(slugs: &[String]) -> String {
    let mut q = String::from("query{");
    for (k, s) in slugs.iter().enumerate() {
        let (owner, name) = s.split_once('/').unwrap_or((s, ""));
        q.push_str(&format!(
            "r{k}:repository(owner:\"{owner}\",name:\"{name}\"){{pullRequests(states:OPEN){{totalCount}}}} \
             v{k}:search(type:ISSUE,first:0,query:\"repo:{s} is:pr is:open review-requested:@me\"){{issueCount}} "
        ));
    }
    q.push('}');
    q
}

/// The counts of `slugs` from GitHub's answer; a repository it couldn't read is left out.
pub fn parse_pulls(v: &Value, slugs: &[String]) -> Vec<Option<Pulls>> {
    let d = &v["data"];
    slugs
        .iter()
        .enumerate()
        .map(|(k, s)| {
            let open = d[format!("r{k}")]["pullRequests"]["totalCount"].as_u64()?;
            let review = d[format!("v{k}")]["issueCount"].as_u64().unwrap_or(0);
            Some(Pulls { open: open as usize, review: review as usize, slug: s.clone() })
        })
        .collect()
}

/// The pull requests of every repository on GitHub, one query per `PULLS_BATCH`. Without `gh`,
/// signed out or outside GitHub, nothing: the switcher then shows no pull request badge.
fn read_pulls(paths: &[PathBuf]) -> HashMap<PathBuf, Pulls> {
    let mut out = HashMap::new();
    let Some(exe) = find_gh() else { return out };
    let repos: Vec<(PathBuf, String)> = paths.iter().filter_map(|p| github_slug(p).map(|s| (p.clone(), s))).collect();
    for chunk in repos.chunks(PULLS_BATCH) {
        let slugs: Vec<String> = chunk.iter().map(|(_, s)| s.clone()).collect();
        let query = format!("query={}", pulls_query(&slugs));
        // A repository that is gone or private to someone else makes GitHub answer with errors
        // beside the data of the others, and gh then fails: its answer is read all the same.
        let Ok(o) = std::process::Command::new(&exe).args(["api", "graphql", "-f", &query]).env("GH_PROMPT_DISABLED", "1").env("NO_COLOR", "1").output() else { break };
        let Ok(v) = serde_json::from_slice::<Value>(&o.stdout) else { continue };
        for ((p, _), pulls) in chunk.iter().zip(parse_pulls(&v, &slugs)) {
            if let Some(pulls) = pulls {
                out.insert(p.clone(), pulls);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn status_gives_changes_ahead_and_behind() {
        let s = "# branch.oid abc\n# branch.head main\n# branch.upstream origin/main\n# branch.ab +2 -5\n1 .M N... 100644 100644 100644 a b src/x.rs\n? new.txt\n! target/\n";
        assert_eq!(parse_status(s), Local { changes: 2, ahead: 2, behind: 5 });
        // No upstream: no "branch.ab" line.
        assert_eq!(parse_status("# branch.oid abc\n# branch.head topic\n"), Local::default());
    }

    #[test]
    fn query_asks_each_repository_and_its_reviews() {
        let q = pulls_query(&["C2-POLY/fourchette".into(), "me/site".into()]);
        assert!(q.contains("r0:repository(owner:\"C2-POLY\",name:\"fourchette\")"));
        assert!(q.contains("v1:search(type:ISSUE,first:0,query:\"repo:me/site is:pr is:open review-requested:@me\")"));
    }

    #[test]
    fn pulls_skip_repositories_github_could_not_read() {
        let v = json!({"data": {"r0": {"pullRequests": {"totalCount": 3}}, "v0": {"issueCount": 1}, "r1": null, "v1": {"issueCount": 0}}});
        let p = parse_pulls(&v, &["a/b".into(), "c/d".into()]);
        assert_eq!(p[0], Some(Pulls { open: 3, review: 1, slug: "a/b".into() }));
        assert_eq!(p[1], None);
        assert_eq!(p[0].as_ref().unwrap().url(), "https://github.com/a/b/pulls");
    }

    #[test]
    fn slugs_with_odd_characters_stay_out_of_the_query() {
        assert!(safe_slug("C2-POLY/fourchette.site"));
        assert!(!safe_slug("me/x\"){evil}"));
    }

    #[test]
    fn fetch_all_counts_and_keeps_failures() {
        let mut s = States::default();
        s.fetch = Some(FetchAll { total: 2, ..Default::default() });
        s.tx.send(Msg::Fetched("/a".into(), Ok(()))).unwrap();
        assert_eq!(s.poll(), None);
        s.tx.send(Msg::Fetched("/b".into(), Err("Could not resolve host".into()))).unwrap();
        let done = s.poll().expect("finished");
        assert_eq!(done.failures, vec![(PathBuf::from("/b"), "Could not resolve host".to_string())]);
        assert_eq!(s.fetch_error(Path::new("/b")), Some("Could not resolve host"));
        assert_eq!(s.fetch_error(Path::new("/a")), None);
    }
}
