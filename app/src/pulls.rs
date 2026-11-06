//! The pull requests of the open repository and the CI of their last commit, read with
//! `gh api graphql` in the background: every open one, plus the 30 most recently updated among
//! the merged and closed, so a branch not deleted yet can still say "merged". The graph and the
//! sidebar show them as a badge on the branch of the same name. Pull requests from forks are
//! left out, since their branch is not one of this repository's.
use crate::github::{find_gh, gh, parse_time};
use serde_json::Value;
use std::collections::HashMap;

/// Open pull requests read at most, 100 a page.
pub const OPEN_LIMIT: usize = 500;
/// Merged and closed pull requests read, the most recently updated.
pub const RECENT: usize = 30;

/// Where a pull request stands, its review included.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Draft,
    Open,
    Approved,
    ChangesRequested,
    Merged,
    Closed,
}

impl State {
    pub fn is_open(self) -> bool {
        !matches!(self, State::Merged | State::Closed)
    }

    pub fn label(self) -> &'static str {
        match self {
            State::Draft => "Draft",
            State::Open => "Open, waiting for review",
            State::Approved => "Approved",
            State::ChangesRequested => "Changes requested",
            State::Merged => "Merged",
            State::Closed => "Closed without merging",
        }
    }
}

/// The combined state of the checks on a pull request's last commit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ci {
    Success,
    Failure,
    Pending,
}

impl Ci {
    pub fn label(self) -> &'static str {
        match self {
            Ci::Success => "Checks passed",
            Ci::Failure => "Checks failed",
            Ci::Pending => "Checks running",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pull {
    pub number: u64,
    pub title: String,
    pub url: String,
    /// The head branch, as named in this repository.
    pub branch: String,
    pub state: State,
    /// None when the last commit has no check at all.
    pub ci: Option<Ci>,
    /// Seconds since 1970, to keep the latest of a branch's pull requests.
    pub updated: i64,
}

impl Pull {
    /// The hover text: "#123 title", then the state and the checks.
    pub fn describe(&self) -> String {
        let ci = self.ci.map(Ci::label).unwrap_or("No checks");
        format!("#{} {}\n{} · {}", self.number, self.title, self.state.label(), ci)
    }
}

/// The pull requests read, and the one each branch shows.
#[derive(Clone, Debug, Default)]
pub struct Pulls {
    pub list: Vec<Pull>,
    /// The remote they were read from ("origin"): its remote branches show them too.
    pub remote: String,
    by_branch: HashMap<String, usize>,
}

impl Pulls {
    /// A branch shows its open pull request (the latest one if it has several), else its latest
    /// merged or closed one.
    pub fn new(list: Vec<Pull>, remote: &str) -> Pulls {
        let mut by_branch: HashMap<String, usize> = HashMap::new();
        for (i, p) in list.iter().enumerate() {
            let better = match by_branch.get(&p.branch) {
                None => true,
                Some(&j) => {
                    let q = &list[j];
                    (p.state.is_open(), p.updated) > (q.state.is_open(), q.updated)
                }
            };
            if better {
                by_branch.insert(p.branch.clone(), i);
            }
        }
        Pulls { list, remote: remote.to_string(), by_branch }
    }

    /// The pull request of a local branch ("feat/x").
    pub fn for_branch(&self, name: &str) -> Option<&Pull> {
        self.by_branch.get(name).map(|&i| &self.list[i])
    }

    /// The pull request of a remote branch ("origin/feat/x"), when it is on the remote read.
    pub fn for_remote(&self, name: &str) -> Option<&Pull> {
        let (remote, branch) = name.split_once('/')?;
        (remote == self.remote).then(|| self.for_branch(branch)).flatten()
    }
}

const FIELDS: &str = "fragment F on PullRequest{number title url state isDraft reviewDecision updatedAt headRefName \
isCrossRepository headRepositoryOwner{login} commits(last:1){nodes{commit{statusCheckRollup{state}}}}}";

const QUERY: &str = "query($owner:String!,$name:String!,$cursor:String,$first:Boolean!){repository(owner:$owner,name:$name){\
open:pullRequests(states:OPEN,first:100,after:$cursor,orderBy:{field:UPDATED_AT,direction:DESC}){pageInfo{hasNextPage endCursor} nodes{...F}} \
recent:pullRequests(states:[MERGED,CLOSED],first:30,orderBy:{field:UPDATED_AT,direction:DESC}) @include(if:$first){nodes{...F}}}}";

/// One pull request from its GraphQL node; None for one from a fork (or whose fork is gone),
/// whose branch is not this repository's.
pub fn parse_pull(n: &Value, owner: &str) -> Option<Pull> {
    let head_owner = n["headRepositoryOwner"]["login"].as_str()?;
    if n["isCrossRepository"].as_bool().unwrap_or(false) || !head_owner.eq_ignore_ascii_case(owner) {
        return None;
    }
    let state = match n["state"].as_str()? {
        "MERGED" => State::Merged,
        "CLOSED" => State::Closed,
        _ if n["isDraft"].as_bool().unwrap_or(false) => State::Draft,
        _ => match n["reviewDecision"].as_str() {
            Some("APPROVED") => State::Approved,
            Some("CHANGES_REQUESTED") => State::ChangesRequested,
            _ => State::Open,
        },
    };
    let ci = match n["commits"]["nodes"][0]["commit"]["statusCheckRollup"]["state"].as_str() {
        Some("SUCCESS") => Some(Ci::Success),
        Some("FAILURE") | Some("ERROR") => Some(Ci::Failure),
        Some("PENDING") | Some("EXPECTED") => Some(Ci::Pending),
        _ => None,
    };
    Some(Pull {
        number: n["number"].as_u64()?,
        title: n["title"].as_str().unwrap_or("").to_string(),
        url: n["url"].as_str().unwrap_or("").to_string(),
        branch: n["headRefName"].as_str()?.to_string(),
        state,
        ci,
        updated: n["updatedAt"].as_str().map(parse_time).unwrap_or(0),
    })
}

/// One answer of `QUERY`: its pull requests (the recent ones only on the first page), and the
/// cursor of the next page of open ones.
pub fn parse_page(v: &Value, owner: &str) -> Result<(Vec<Pull>, Option<String>), String> {
    if let Some(m) = v["errors"][0]["message"].as_str() {
        return Err(format!("GitHub: {m}"));
    }
    let repo = &v["data"]["repository"];
    if repo.is_null() {
        return Err("GitHub doesn't know this repository.".into());
    }
    let nodes = |k: &str| repo[k]["nodes"].as_array().cloned().unwrap_or_default();
    let pulls = nodes("open").iter().chain(nodes("recent").iter()).filter_map(|n| parse_pull(n, owner)).collect();
    let info = &repo["open"]["pageInfo"];
    let next = if info["hasNextPage"].as_bool().unwrap_or(false) { info["endCursor"].as_str().map(String::from) } else { None };
    Ok((pulls, next))
}

/// Reads the pull requests of "owner/name", found on `remote`. Runs in a background thread: one
/// call to GitHub per 100 open pull requests.
pub fn load(slug: &str, remote: &str) -> Result<Pulls, String> {
    let exe = find_gh().ok_or("The GitHub command line (gh) isn't installed.")?;
    let (owner, name) = slug.split_once('/').ok_or("Not a GitHub repository")?;
    let mut list = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let mut args = vec!["api".to_string(), "graphql".into(), "-f".into(), format!("query={QUERY} {FIELDS}")];
        args.extend(["-F".into(), format!("owner={owner}"), "-F".into(), format!("name={name}"), "-F".into(), format!("first={}", cursor.is_none())]);
        if let Some(c) = &cursor {
            args.extend(["-f".into(), format!("cursor={c}")]);
        }
        let a: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let s = gh(&exe, &a)?;
        let v: Value = serde_json::from_str(&s).map_err(|_| "GitHub's answer wasn't in the expected form.".to_string())?;
        let (page, next) = parse_page(&v, owner)?;
        list.extend(page);
        cursor = next;
        if cursor.is_none() || list.len() >= OPEN_LIMIT + RECENT {
            break;
        }
    }
    Ok(Pulls::new(list, remote))
}

/// Where one check of a pull request stands, in the order the list shows them: what breaks first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CheckState {
    Failure,
    Pending,
    Success,
    /// Skipped, neutral or stale: it neither passed nor failed.
    Skipped,
}

impl CheckState {
    pub fn label(self) -> &'static str {
        match self {
            CheckState::Failure => "Failed",
            CheckState::Pending => "Running",
            CheckState::Success => "Passed",
            CheckState::Skipped => "Skipped",
        }
    }
}

/// One check on a pull request's last commit: a GitHub Actions job (or another app's check run),
/// or a commit status set by an outside service.
#[derive(Clone, Debug, PartialEq)]
pub struct Check {
    pub name: String,
    /// The workflow of an Actions job ("CI"); empty for the others.
    pub workflow: String,
    pub state: CheckState,
    /// Its log on GitHub, or the page the outside service gave.
    pub url: String,
    /// Seconds since 1970.
    pub started: Option<i64>,
    pub finished: Option<i64>,
}

impl Check {
    /// How long it took, or has been running at `now`; None when it never started.
    pub fn duration(&self, now: i64) -> Option<i64> {
        let start = self.started?;
        let end = match (self.state, self.finished) {
            (CheckState::Pending, _) => now,
            (_, Some(f)) => f,
            _ => return None,
        };
        Some((end - start).max(0))
    }
}

/// "42s", "3m 12s", "1h 04m".
pub fn format_duration(secs: i64) -> String {
    match secs {
        s if s < 60 => format!("{s}s"),
        s if s < 3600 => format!("{}m {:02}s", s / 60, s % 60),
        s => format!("{}h {:02}m", s / 3600, s % 3600 / 60),
    }
}

/// "2 failing · 1 running · 5 passed", leaving out the kinds there are none of.
pub fn checks_summary(checks: &[Check]) -> String {
    let n = |s: CheckState| checks.iter().filter(|c| c.state == s).count();
    [(CheckState::Failure, "failing"), (CheckState::Pending, "running"), (CheckState::Success, "passed"), (CheckState::Skipped, "skipped")]
        .iter()
        .filter(|(s, _)| n(*s) > 0)
        .map(|(s, w)| format!("{} {w}", n(*s)))
        .collect::<Vec<_>>()
        .join(" · ")
}

const CHECKS_QUERY: &str = "query($owner:String!,$name:String!,$number:Int!){repository(owner:$owner,name:$name){pullRequest(number:$number){\
commits(last:1){nodes{commit{statusCheckRollup{contexts(first:100){nodes{__typename \
...on CheckRun{name status conclusion detailsUrl startedAt completedAt checkSuite{workflowRun{workflow{name}}}} \
...on StatusContext{context state targetUrl createdAt}}}}}}}}}}";

/// One check from its GraphQL node: a `CheckRun` or a `StatusContext`.
pub fn parse_check(n: &Value) -> Option<Check> {
    let time = |k: &str| n[k].as_str().map(parse_time).filter(|&t| t > 0);
    let text = |v: &Value| v.as_str().unwrap_or("").to_string();
    if n["__typename"] == "StatusContext" {
        let state = match n["state"].as_str()? {
            "SUCCESS" => CheckState::Success,
            "FAILURE" | "ERROR" => CheckState::Failure,
            _ => CheckState::Pending,
        };
        return Some(Check { name: text(&n["context"]), workflow: String::new(), state, url: text(&n["targetUrl"]), started: time("createdAt"), finished: None });
    }
    let state = if n["status"].as_str()? != "COMPLETED" {
        CheckState::Pending
    } else {
        match n["conclusion"].as_str().unwrap_or("") {
            "SUCCESS" => CheckState::Success,
            "NEUTRAL" | "SKIPPED" | "STALE" => CheckState::Skipped,
            // Failure, timed out, cancelled, startup failure, action required.
            _ => CheckState::Failure,
        }
    };
    Some(Check {
        name: text(&n["name"]),
        workflow: text(&n["checkSuite"]["workflowRun"]["workflow"]["name"]),
        state,
        url: text(&n["detailsUrl"]),
        started: time("startedAt"),
        finished: time("completedAt"),
    })
}

/// The checks of one answer of `CHECKS_QUERY`, the failing ones first, then by name.
pub fn parse_checks(v: &Value) -> Result<Vec<Check>, String> {
    if let Some(m) = v["errors"][0]["message"].as_str() {
        return Err(format!("GitHub: {m}"));
    }
    let pr = &v["data"]["repository"]["pullRequest"];
    if pr.is_null() {
        return Err("GitHub doesn't know this pull request.".into());
    }
    let nodes = pr["commits"]["nodes"][0]["commit"]["statusCheckRollup"]["contexts"]["nodes"].as_array().cloned().unwrap_or_default();
    let mut checks: Vec<Check> = nodes.iter().filter_map(parse_check).collect();
    checks.sort_by(|a, b| (a.state, &a.workflow, &a.name).cmp(&(b.state, &b.workflow, &b.name)));
    Ok(checks)
}

/// Reads the checks of pull request `number` of "owner/name". Runs in a background thread.
pub fn load_checks(slug: &str, number: u64) -> Result<Vec<Check>, String> {
    let exe = find_gh().ok_or("The GitHub command line (gh) isn't installed.")?;
    let (owner, name) = slug.split_once('/').ok_or("Not a GitHub repository")?;
    let query = format!("query={CHECKS_QUERY}");
    let (owner, name, number) = (format!("owner={owner}"), format!("name={name}"), format!("number={number}"));
    let s = gh(&exe, &["api", "graphql", "-f", &query, "-F", &owner, "-F", &name, "-F", &number])?;
    let v: Value = serde_json::from_str(&s).map_err(|_| "GitHub's answer wasn't in the expected form.".to_string())?;
    parse_checks(&v)
}

/// The checks of a sample pull request, for screenshots: failing ones when its CI fails, one
/// still running when it runs, all passed otherwise. `now` is seconds since 1970.
pub fn sample_checks(pull: &Pull, now: i64) -> Vec<Check> {
    let c = |name: &str, workflow: &str, state: CheckState, secs: Option<i64>| Check {
        name: name.into(),
        workflow: workflow.into(),
        state,
        url: format!("https://github.com/me/sample-project/actions/runs/{}", pull.number),
        started: secs.map(|s| now - 600 - s),
        finished: secs.filter(|_| state != CheckState::Pending).map(|_| now - 600),
    };
    let (test, lint) = match pull.ci {
        Some(Ci::Failure) => (CheckState::Failure, CheckState::Failure),
        Some(Ci::Pending) => (CheckState::Pending, CheckState::Success),
        _ => (CheckState::Success, CheckState::Success),
    };
    let mut checks = vec![
        c("test (macos-latest)", "CI", test, Some(192)),
        c("lint", "CI", lint, Some(41)),
        c("build (ubuntu-latest)", "CI", CheckState::Success, Some(125)),
        c("typos", "Spelling", CheckState::Success, Some(9)),
        c("Deploy preview", "Pages", CheckState::Skipped, None),
    ];
    if test == CheckState::Pending {
        // Running for a minute and a half.
        checks[0].started = Some(now - 95);
    }
    checks.sort_by(|a, b| (a.state, &a.workflow, &a.name).cmp(&(b.state, &b.workflow, &b.name)));
    checks
}

/// The sample project's pull requests, for screenshots: one of each kind of badge.
pub fn sample() -> Pulls {
    let p = |number: u64, title: &str, branch: &str, state: State, ci: Option<Ci>| Pull {
        number,
        title: title.into(),
        url: format!("https://github.com/me/sample-project/pull/{number}"),
        branch: branch.into(),
        state,
        ci,
        updated: 1_790_000_000 + number as i64,
    };
    Pulls::new(
        vec![
            p(58, "Colour the graph lanes by branch", "feature/graph-colours", State::Open, Some(Ci::Pending)),
            p(57, "Fix the login timeout on slow networks", "hotfix/login-timeout", State::Approved, Some(Ci::Success)),
            p(55, "Search commits by author and hash", "feature/search", State::Draft, Some(Ci::Failure)),
            p(51, "Remember the last opened repository per window", "feature/remember-the-last-opened-repository-per-window", State::Merged, Some(Ci::Success)),
        ],
        "origin",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn node(number: u64, branch: &str, state: &str, draft: bool, review: Value, rollup: Value) -> Value {
        json!({
            "number": number, "title": format!("PR {number}"), "url": format!("https://github.com/o/r/pull/{number}"),
            "state": state, "isDraft": draft, "reviewDecision": review, "updatedAt": "2026-11-05T10:00:00Z",
            "headRefName": branch, "isCrossRepository": false, "headRepositoryOwner": {"login": "o"},
            "commits": {"nodes": [{"commit": {"statusCheckRollup": rollup}}]}
        })
    }

    #[test]
    fn states_and_checks_are_read() {
        let p = |n: Value| parse_pull(&n, "o").unwrap();
        let none = node(1, "a", "OPEN", false, Value::Null, Value::Null);
        assert_eq!((p(none.clone()).state, p(none).ci), (State::Open, None));
        let draft = p(node(2, "b", "OPEN", true, json!("REVIEW_REQUIRED"), json!({"state": "PENDING"})));
        assert_eq!((draft.state, draft.ci), (State::Draft, Some(Ci::Pending)));
        let ok = p(node(3, "c", "OPEN", false, json!("APPROVED"), json!({"state": "SUCCESS"})));
        assert_eq!((ok.state, ok.ci), (State::Approved, Some(Ci::Success)));
        let changes = p(node(4, "d", "OPEN", false, json!("CHANGES_REQUESTED"), json!({"state": "ERROR"})));
        assert_eq!((changes.state, changes.ci), (State::ChangesRequested, Some(Ci::Failure)));
        let merged = p(node(5, "e", "MERGED", false, json!("APPROVED"), json!({"state": "FAILURE"})));
        assert_eq!((merged.state, merged.ci), (State::Merged, Some(Ci::Failure)));
        assert_eq!(p(node(6, "f", "CLOSED", true, Value::Null, Value::Null)).state, State::Closed);
        assert_eq!(merged.branch, "e");
        assert_eq!(merged.number, 5);
        assert!(merged.updated > 0);
    }

    #[test]
    fn forks_are_left_out() {
        let mut fork = node(7, "main", "OPEN", false, Value::Null, Value::Null);
        fork["headRepositoryOwner"]["login"] = json!("someone");
        assert!(parse_pull(&fork, "o").is_none());
        let mut cross = node(8, "main", "OPEN", false, Value::Null, Value::Null);
        cross["isCrossRepository"] = json!(true);
        assert!(parse_pull(&cross, "o").is_none());
        let mut gone = node(9, "main", "OPEN", false, Value::Null, Value::Null);
        gone["headRepositoryOwner"] = Value::Null;
        assert!(parse_pull(&gone, "o").is_none());
        // The owner's case doesn't matter.
        assert!(parse_pull(&node(10, "x", "OPEN", false, Value::Null, Value::Null), "O").is_some());
    }

    #[test]
    fn a_page_gives_its_pulls_and_the_next_cursor() {
        let v = json!({"data": {"repository": {
            "open": {"pageInfo": {"hasNextPage": true, "endCursor": "abc"}, "nodes": [node(1, "a", "OPEN", false, Value::Null, Value::Null)]},
            "recent": {"nodes": [node(2, "b", "MERGED", false, Value::Null, Value::Null)]}
        }}});
        let (pulls, next) = parse_page(&v, "o").unwrap();
        assert_eq!(pulls.iter().map(|p| p.number).collect::<Vec<_>>(), vec![1, 2]);
        assert_eq!(next.as_deref(), Some("abc"));
        // A later page has no "recent" part, and no next page.
        let v = json!({"data": {"repository": {"open": {"pageInfo": {"hasNextPage": false, "endCursor": "z"}, "nodes": []}}}});
        assert_eq!(parse_page(&v, "o").unwrap(), (vec![], None));
        assert!(parse_page(&json!({"errors": [{"message": "Bad credentials"}]}), "o").unwrap_err().contains("Bad credentials"));
        assert!(parse_page(&json!({"data": {"repository": null}}), "o").is_err());
    }

    #[test]
    fn a_branch_shows_its_open_pull_else_its_latest() {
        let p = |number: u64, branch: &str, state: State, updated: i64| Pull { number, title: String::new(), url: String::new(), branch: branch.into(), state, ci: None, updated };
        let pulls = Pulls::new(
            vec![p(1, "a", State::Merged, 30), p(2, "a", State::Open, 10), p(3, "b", State::Closed, 5), p(4, "b", State::Merged, 9), p(5, "c", State::Draft, 1), p(6, "c", State::Approved, 2)],
            "origin",
        );
        assert_eq!(pulls.for_branch("a").map(|p| p.number), Some(2));
        assert_eq!(pulls.for_branch("b").map(|p| p.number), Some(4));
        assert_eq!(pulls.for_branch("c").map(|p| p.number), Some(6));
        assert!(pulls.for_branch("d").is_none());
        assert_eq!(pulls.for_remote("origin/a").map(|p| p.number), Some(2));
        assert!(pulls.for_remote("upstream/a").is_none());
        assert!(pulls.for_remote("a").is_none());
    }

    #[test]
    fn the_sample_has_one_of_each_badge() {
        let s = sample();
        assert_eq!(s.for_branch("feature/search").map(|p| p.state), Some(State::Draft));
        assert_eq!(s.for_remote("origin/feature/graph-colours").and_then(|p| p.ci), Some(Ci::Pending));
        assert!(s.list[0].describe().starts_with("#58 Colour the graph lanes by branch\n"));
    }

    #[test]
    fn checks_are_read_failing_first() {
        let run = |name: &str, status: &str, conclusion: Value| {
            json!({"__typename": "CheckRun", "name": name, "status": status, "conclusion": conclusion,
                "detailsUrl": format!("https://github.com/o/r/actions/runs/1/job/{name}"),
                "startedAt": "2026-11-05T10:00:00Z", "completedAt": if status == "COMPLETED" { json!("2026-11-05T10:03:12Z") } else { Value::Null },
                "checkSuite": {"workflowRun": {"workflow": {"name": "CI"}}}})
        };
        let status = json!({"__typename": "StatusContext", "context": "ci/circle", "state": "ERROR", "targetUrl": "https://circleci.com/x", "createdAt": "2026-11-05T10:00:00Z"});
        let v = json!({"data": {"repository": {"pullRequest": {"commits": {"nodes": [{"commit": {"statusCheckRollup": {"contexts": {"nodes": [
            run("build", "COMPLETED", json!("SUCCESS")), run("docs", "COMPLETED", json!("SKIPPED")), run("test", "IN_PROGRESS", Value::Null),
            run("lint", "COMPLETED", json!("TIMED_OUT")), status
        ]}}}}]}}}}});
        let checks = parse_checks(&v).unwrap();
        let got: Vec<(&str, CheckState)> = checks.iter().map(|c| (c.name.as_str(), c.state)).collect();
        assert_eq!(
            got,
            vec![("ci/circle", CheckState::Failure), ("lint", CheckState::Failure), ("test", CheckState::Pending), ("build", CheckState::Success), ("docs", CheckState::Skipped)]
        );
        let build = &checks[3];
        assert_eq!((build.workflow.as_str(), build.url.as_str()), ("CI", "https://github.com/o/r/actions/runs/1/job/build"));
        assert_eq!(build.duration(0), Some(192));
        // A running check counts up to now; a status has no end.
        let start = checks[2].started.unwrap();
        assert_eq!(checks[2].duration(start + 30), Some(30));
        assert_eq!(checks[0].duration(start + 30), None);
        assert_eq!(checks_summary(&checks), "2 failing · 1 running · 1 passed · 1 skipped");
        // No checks at all, an unknown pull request, an error.
        let empty = json!({"data": {"repository": {"pullRequest": {"commits": {"nodes": [{"commit": {"statusCheckRollup": null}}]}}}}});
        assert_eq!(parse_checks(&empty).unwrap(), vec![]);
        assert!(parse_checks(&json!({"data": {"repository": {"pullRequest": null}}})).is_err());
        assert!(parse_checks(&json!({"errors": [{"message": "Bad credentials"}]})).unwrap_err().contains("Bad credentials"));
    }

    #[test]
    fn durations_read_short() {
        assert_eq!(format_duration(9), "9s");
        assert_eq!(format_duration(192), "3m 12s");
        assert_eq!(format_duration(3600 + 4 * 60 + 59), "1h 04m");
    }

    #[test]
    fn sample_checks_follow_the_badge() {
        let s = sample();
        let of = |b: &str| sample_checks(s.for_branch(b).unwrap(), 2_000_000_000);
        assert_eq!(of("feature/search")[0].state, CheckState::Failure);
        let running = of("feature/graph-colours");
        assert_eq!((running[0].state, running[0].duration(2_000_000_000)), (CheckState::Pending, Some(95)));
        assert!(of("hotfix/login-timeout").iter().all(|c| c.state != CheckState::Failure && c.state != CheckState::Pending));
    }

    /// A busy public repository with CI: some of its open pull requests have checks.
    #[test]
    #[ignore]
    fn reads_a_real_repository() {
        let pulls = load("cli/cli", "origin").expect("gh signed in");
        assert!(!pulls.list.is_empty());
        assert!(pulls.list.iter().any(|p| p.ci.is_some()), "no pull request with checks");
        assert!(pulls.list.iter().any(|p| !p.state.is_open()), "no merged or closed pull request");
        assert!(pulls.list.iter().all(|p| !p.branch.is_empty()));
    }

    /// The checks of a pull request of the same repository that has some.
    #[test]
    #[ignore]
    fn reads_real_checks() {
        let pulls = load("cli/cli", "origin").expect("gh signed in");
        let pr = pulls.list.iter().find(|p| p.ci.is_some()).expect("a pull request with checks");
        let checks = load_checks("cli/cli", pr.number).expect("checks read");
        assert!(!checks.is_empty());
        assert!(checks.iter().all(|c| !c.name.is_empty()));
        assert!(checks.windows(2).all(|w| w[0].state <= w[1].state), "failing first");
    }
}
