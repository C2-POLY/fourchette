//! GitHub Projects, read only: the board of the project linked to the open repository.
//! Everything goes through the GitHub command line (`gh`), so its sign-in is used and the app
//! never keeps a token.
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Items read from one project (a page of `gh project item-list`).
pub const ITEM_LIMIT: usize = 500;
/// What items without a status are shown under.
pub const NO_STATUS: &str = "No status";

const NOT_SIGNED_IN: &str = "The GitHub command line isn't signed in. Open Terminal and run `gh auth login`.";
const NO_PROJECT_SCOPE: &str = "GitHub needs your OK to read projects. Open Terminal and run `gh auth refresh -s project`.";

/// Apps opened from the Finder get a short PATH, so the usual places are checked too.
pub fn find_gh() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = vec![PathBuf::from("/opt/homebrew/bin/gh"), PathBuf::from("/usr/local/bin/gh")];
    if let Some(p) = std::env::var_os("PATH") {
        candidates.extend(std::env::split_paths(&p).map(|d| d.join("gh")));
    }
    candidates.into_iter().find(|p| p.is_file())
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProjectRef {
    pub owner: String,
    pub number: u64,
    pub title: String,
    pub url: String,
}

impl ProjectRef {
    /// How the choice is remembered in the repository list.
    pub fn key(&self) -> String {
        format!("{}#{}", self.owner, self.number)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Issue,
    PullRequest,
    Draft,
}

#[derive(Clone, Debug)]
pub struct Item {
    pub title: String,
    pub status: String,
    pub number: Option<u64>,
    pub kind: Kind,
    pub url: String,
    pub assignees: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Board {
    /// Projects linked to the repository.
    pub projects: Vec<ProjectRef>,
    pub chosen: usize,
    /// Status columns in the project's own order, each with its items.
    pub columns: Vec<(String, Vec<Item>)>,
    /// The signed-in GitHub login, for "Mine only".
    pub me: String,
}

impl Board {
    pub fn project(&self) -> Option<&ProjectRef> {
        self.projects.get(self.chosen)
    }
}

/// "owner/name" from a GitHub remote address (SSH or HTTPS).
pub fn slug(remote_url: &str) -> Option<String> {
    let u = remote_url.trim();
    let rest = u
        .strip_prefix("git@github.com:")
        .or_else(|| u.strip_prefix("ssh://git@github.com/"))
        .or_else(|| u.strip_prefix("https://github.com/"))
        .or_else(|| u.strip_prefix("http://github.com/"))?;
    let rest = rest.trim_end_matches('/');
    let rest = rest.strip_suffix(".git").unwrap_or(rest);
    let mut parts = rest.split('/');
    let (owner, name) = (parts.next()?, parts.next()?);
    (!owner.is_empty() && !name.is_empty() && parts.next().is_none()).then(|| format!("{owner}/{name}"))
}

/// Statuses that are finished, so their column starts folded.
pub fn is_done(status: &str) -> bool {
    let s = status.to_lowercase();
    s == "done" || s.contains("closed") || s.contains("shipped") || s.contains("merged") || s.contains("termin")
}

/// A branch name for an item: "38-update-validation-process".
pub fn branch_name(item: &Item) -> String {
    let words: String = item.title.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { ' ' }).collect();
    let words: Vec<&str> = words.split_whitespace().take(6).collect();
    let mut name = words.join("-");
    if let Some(n) = item.number {
        name = if name.is_empty() { n.to_string() } else { format!("{n}-{name}") };
    }
    name
}

fn gh(exe: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new(exe).args(args).env("GH_PROMPT_DISABLED", "1").env("NO_COLOR", "1").output().map_err(|e| format!("Couldn't start the GitHub command line: {e}"))?;
    if out.status.success() {
        return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
    }
    let err = String::from_utf8_lossy(&out.stderr).into_owned();
    let m = err.to_lowercase();
    Err(if m.contains("gh auth login") || m.contains("not logged") {
        NOT_SIGNED_IN.into()
    } else if m.contains("read:project") || m.contains("scope") {
        NO_PROJECT_SCOPE.into()
    } else {
        format!("GitHub: {}", err.trim().chars().take(300).collect::<String>())
    })
}

fn json(exe: &Path, args: &[&str]) -> Result<Value, String> {
    let s = gh(exe, args)?;
    serde_json::from_str(&s).map_err(|_| "GitHub's answer wasn't in the expected form.".to_string())
}

/// Projects linked to the repository, and those its recent issues and pull requests are in:
/// many projects hold a repository's issues without being linked to it.
const PROJECTS_QUERY: &str = "fragment P on ProjectV2{number title url closed owner{... on Organization{login} ... on User{login}}} \
query($owner:String!,$name:String!){repository(owner:$owner,name:$name){\
projectsV2(first:20){nodes{...P}} \
issues(first:50,orderBy:{field:UPDATED_AT,direction:DESC}){nodes{projectItems(first:5){nodes{project{...P}}}}} \
pullRequests(first:30,orderBy:{field:UPDATED_AT,direction:DESC}){nodes{projectItems(first:5){nodes{project{...P}}}}}}}";

/// Open projects for the repository, from the GraphQL answer: linked ones first, then the
/// others in the order its issues and pull requests mention them, each once.
pub fn parse_projects(v: &Value) -> Vec<ProjectRef> {
    let repo = &v["data"]["repository"];
    let mut nodes: Vec<&Value> = repo["projectsV2"]["nodes"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
    for kind in ["issues", "pullRequests"] {
        for n in repo[kind]["nodes"].as_array().into_iter().flatten() {
            nodes.extend(n["projectItems"]["nodes"].as_array().into_iter().flatten().map(|i| &i["project"]));
        }
    }
    let mut out: Vec<ProjectRef> = Vec::new();
    for n in nodes.into_iter().filter(|n| !n["closed"].as_bool().unwrap_or(false)) {
        let (Some(owner), Some(number)) = (n["owner"]["login"].as_str(), n["number"].as_u64()) else { continue };
        if !out.iter().any(|p| p.owner == owner && p.number == number) {
            out.push(ProjectRef { owner: owner.into(), number, title: n["title"].as_str().unwrap_or("").into(), url: n["url"].as_str().unwrap_or("").into() });
        }
    }
    out
}

/// The Status options in the project's order (from `gh project field-list`).
pub fn parse_status_order(v: &Value) -> Vec<String> {
    v["fields"]
        .as_array()
        .and_then(|a| a.iter().find(|f| f["name"].as_str() == Some("Status")))
        .and_then(|f| f["options"].as_array())
        .map(|o| o.iter().filter_map(|x| x["name"].as_str().map(String::from)).collect())
        .unwrap_or_default()
}

/// Items of the project that belong to this repository (from `gh project item-list`).
pub fn parse_items(v: &Value, slug: &str) -> Vec<Item> {
    let want = format!("https://github.com/{slug}").to_lowercase();
    v["items"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|i| i["repository"].as_str().map(|r| r.to_lowercase() == want).unwrap_or(false))
                .map(|i| {
                    let c = &i["content"];
                    Item {
                        title: i["title"].as_str().or(c["title"].as_str()).unwrap_or("").into(),
                        status: i["status"].as_str().filter(|s| !s.is_empty()).unwrap_or(NO_STATUS).into(),
                        number: c["number"].as_u64(),
                        kind: match c["type"].as_str() {
                            Some("PullRequest") => Kind::PullRequest,
                            Some("Issue") => Kind::Issue,
                            _ => Kind::Draft,
                        },
                        url: c["url"].as_str().unwrap_or("").into(),
                        assignees: i["assignees"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default(),
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Puts items in their status column: the project's order first, then any status it doesn't
/// list, then items without a status. Empty columns are left out.
pub fn columns(order: &[String], items: Vec<Item>) -> Vec<(String, Vec<Item>)> {
    let mut cols: Vec<(String, Vec<Item>)> = order.iter().map(|s| (s.clone(), Vec::new())).collect();
    for it in items {
        match cols.iter_mut().find(|c| c.0 == it.status) {
            Some(c) => c.1.push(it),
            None => {
                let at = if it.status == NO_STATUS { cols.len() } else { cols.iter().position(|c| c.0 == NO_STATUS).unwrap_or(cols.len()) };
                cols.insert(at, (it.status.clone(), vec![it]));
            }
        }
    }
    cols.retain(|c| !c.1.is_empty());
    cols
}

/// Reads the board for a repository. `preferred` is the "owner#number" chosen last time.
/// Runs in a background thread: it makes three or four calls to GitHub.
pub fn load(slug: &str, preferred: Option<&str>) -> Result<Board, String> {
    let exe = find_gh().ok_or("The GitHub command line (gh) isn't installed. Install it with `brew install gh`, then run `gh auth login`.")?;
    let (owner, name) = slug.split_once('/').ok_or("Not a GitHub repository")?;
    let v = json(&exe, &["api", "graphql", "-f", &format!("query={PROJECTS_QUERY}"), "-F", &format!("owner={owner}"), "-F", &format!("name={name}")])?;
    let projects = parse_projects(&v);
    let me = gh(&exe, &["api", "user", "--jq", ".login"]).map(|s| s.trim().to_string()).unwrap_or_default();
    let mut board = Board { projects, me, ..Default::default() };
    if board.projects.is_empty() {
        return Ok(board);
    }
    board.chosen = preferred.and_then(|k| board.projects.iter().position(|p| p.key() == k)).unwrap_or(0);
    let p = board.projects[board.chosen].clone();
    let num = p.number.to_string();
    let fields = json(&exe, &["project", "field-list", &num, "--owner", &p.owner, "--format", "json"])?;
    let items = json(&exe, &["project", "item-list", &num, "--owner", &p.owner, "--format", "json", "-L", &ITEM_LIMIT.to_string()])?;
    board.columns = columns(&parse_status_order(&fields), parse_items(&items, slug));
    Ok(board)
}

/// A small board for the sample project.
pub fn sample() -> Board {
    let it = |n: u64, title: &str, status: &str, kind: Kind, who: &str| Item {
        title: title.into(),
        status: status.into(),
        number: Some(n),
        kind,
        url: format!("https://github.com/example/sample-project/issues/{n}"),
        assignees: if who.is_empty() { vec![] } else { vec![who.into()] },
    };
    let order: Vec<String> = ["Backlog", "Ready", "In progress", "In review", "Done"].iter().map(|s| s.to_string()).collect();
    let items = vec![
        it(51, "Group repositories in the switcher", "In progress", Kind::Issue, "me"),
        it(52, "Show the linked issue in the toolbar", "Ready", Kind::Issue, ""),
        it(48, "Draw curved lines between lanes", "In review", Kind::PullRequest, "me"),
        it(44, "Search commits by author", "Backlog", Kind::Issue, "sami"),
        it(45, "Keyboard shortcut for stash", "Backlog", Kind::Issue, ""),
        it(42, "Give every branch line its own colour", "Done", Kind::Issue, "camille"),
    ];
    Board {
        projects: vec![ProjectRef { owner: "example".into(), number: 1, title: "Fourchette roadmap".into(), url: "https://github.com/orgs/example/projects/1".into() }],
        chosen: 0,
        columns: columns(&order, items),
        me: "me".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_github_remotes() {
        assert_eq!(slug("git@github.com:NovaMoonMedia/site.git").as_deref(), Some("NovaMoonMedia/site"));
        assert_eq!(slug("https://github.com/a/b").as_deref(), Some("a/b"));
        assert_eq!(slug("https://github.com/a/b.git/").as_deref(), Some("a/b"));
        assert_eq!(slug("ssh://git@github.com/a/b.git").as_deref(), Some("a/b"));
        assert_eq!(slug("git@gitlab.com:a/b.git"), None);
        assert_eq!(slug("https://github.com/a"), None);
    }

    #[test]
    fn keeps_this_repository_and_orders_columns() {
        let fields = json!({"fields": [{"name": "Title"}, {"name": "Status", "options": [{"name": "Backlog"}, {"name": "In progress"}, {"name": "Done"}]}]});
        let items = json!({"items": [
            {"title": "A", "status": "Done", "repository": "https://github.com/o/r", "content": {"type": "Issue", "number": 1, "url": "u1"}},
            {"title": "B", "status": "In progress", "repository": "https://github.com/O/R", "content": {"type": "PullRequest", "number": 2}, "assignees": ["me"]},
            {"title": "C", "status": "Backlog", "repository": "https://github.com/o/other", "content": {"type": "Issue", "number": 3}},
            {"title": "D", "content": {"type": "DraftIssue"}},
            {"title": "E", "repository": "https://github.com/o/r", "content": {"type": "Issue", "number": 5}},
            {"title": "F", "status": "Blocked", "repository": "https://github.com/o/r", "content": {"type": "Issue", "number": 6}}
        ]});
        let parsed = parse_items(&items, "o/r");
        assert_eq!(parsed.iter().map(|i| i.title.as_str()).collect::<Vec<_>>(), ["A", "B", "E", "F"]);
        assert_eq!(parsed[1].kind, Kind::PullRequest);
        assert_eq!(parsed[1].assignees, ["me"]);
        let cols = columns(&parse_status_order(&fields), parsed);
        assert_eq!(cols.iter().map(|c| c.0.as_str()).collect::<Vec<_>>(), ["In progress", "Done", "Blocked", NO_STATUS]);
    }

    #[test]
    fn reads_linked_projects_without_closed_ones() {
        let v = json!({"data": {"repository": {"projectsV2": {"nodes": [
            {"number": 2, "title": "Platform", "url": "u", "closed": false, "owner": {"login": "Nova"}},
            {"number": 1, "title": "Old", "url": "u", "closed": true, "owner": {"login": "Nova"}}
        ]}, "issues": {"nodes": [
            {"projectItems": {"nodes": []}},
            {"projectItems": {"nodes": [{"project": {"number": 9, "title": "Tools", "closed": false, "owner": {"login": "Acme"}}}]}},
            {"projectItems": {"nodes": [{"project": {"number": 2, "title": "Platform", "closed": false, "owner": {"login": "Nova"}}}]}}
        ]}, "pullRequests": {"nodes": [
            {"projectItems": {"nodes": [{"project": {"number": 9, "title": "Tools", "closed": false, "owner": {"login": "Acme"}}}]}}
        ]}}}});
        let p = parse_projects(&v);
        assert_eq!(p.iter().map(|p| p.key()).collect::<Vec<_>>(), ["Nova#2", "Acme#9"]);
    }

    #[test]
    fn branch_names_and_done_columns() {
        let it = Item { title: "Update validation process (front)!".into(), status: "Ready".into(), number: Some(38), kind: Kind::Issue, url: String::new(), assignees: vec![] };
        assert_eq!(branch_name(&it), "38-update-validation-process-front");
        assert!(is_done("Done") && is_done("Terminé") && !is_done("In progress"));
    }
}

/// Reads a real project through `gh`: `cargo test live_board -- --ignored --nocapture`.
#[cfg(test)]
mod live {
    #[test]
    #[ignore]
    fn live_board() {
        let b = super::load("ET-Technology-Consulting/ipop-tools-ee-front", None).expect("board");
        println!("projects: {:?}", b.projects.iter().map(|p| p.key()).collect::<Vec<_>>());
        for (s, items) in &b.columns {
            println!("{s}: {}", items.iter().map(|i| format!("#{}", i.number.unwrap_or(0))).collect::<Vec<_>>().join(" "));
        }
        assert!(!b.projects.is_empty());
        assert!(!b.me.is_empty());
    }
}
