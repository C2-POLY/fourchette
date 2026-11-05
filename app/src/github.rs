//! GitHub Projects: the board of the project linked to the open repository, and the moves of
//! its cards (the Status field and the position, nothing else). Everything goes through the GitHub command line (`gh`), so its sign-in is used and the app
//! never keeps a token.
use serde_json::Value;
use std::borrow::Borrow;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Items read from one project, 100 a page.
pub const ITEM_LIMIT: usize = 500;
/// What items without a status are shown under.
pub const NO_STATUS: &str = "No status";

const NOT_SIGNED_IN: &str = "The GitHub command line isn't signed in. Open Terminal and run `gh auth login`.";
const NO_PROJECT_SCOPE: &str = "GitHub needs your OK to read projects. Open Terminal and run `gh auth refresh -s project`.";
const NO_WRITE_SCOPE: &str = "GitHub needs your OK to change projects. Open Terminal and run `gh auth refresh -s project`.";

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

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Kind {
    Issue,
    PullRequest,
    #[default]
    Draft,
}

/// A label with its GitHub colour ("1cadf8").
#[derive(Clone, Debug, PartialEq)]
pub struct Label {
    pub name: String,
    pub color: String,
}

/// A project field value other than Status and Title: priority, size, estimate, iteration…
/// `color` is the option's colour name ("RED") for single-select fields.
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub name: String,
    pub value: String,
    pub color: Option<String>,
}

/// A sub-issue or a linked pull request.
#[derive(Clone, Debug, PartialEq)]
pub struct Linked {
    pub number: u64,
    pub title: String,
    /// OPEN, CLOSED or MERGED.
    pub state: String,
    pub url: String,
}

#[derive(Clone, Debug, Default)]
pub struct Item {
    /// The project item's id.
    pub id: String,
    pub title: String,
    pub status: String,
    pub number: Option<u64>,
    pub kind: Kind,
    pub url: String,
    pub assignees: Vec<String>,
    /// "owner/name"; empty for drafts.
    pub repo: String,
    /// OPEN, CLOSED or MERGED; empty for drafts.
    pub state: String,
    /// The organisation's issue type (Feature, Task, Bug) and its colour name.
    pub issue_type: Option<(String, String)>,
    pub labels: Vec<Label>,
    pub fields: Vec<Field>,
    pub body: String,
    /// Unix seconds.
    pub created: i64,
    pub updated: i64,
    /// Sub-issues (completed, total), from GitHub's summary.
    pub sub_progress: (u32, u32),
    pub sub_issues: Vec<Linked>,
    pub pull_requests: Vec<Linked>,
    /// The issue this one is a sub-issue of.
    pub parent: Option<Linked>,
}

impl Item {
    /// Progress to show: the sub-issues, else the body's checkboxes. None when it has neither.
    pub fn progress(&self) -> Option<(u32, u32)> {
        if self.sub_progress.1 > 0 {
            return Some(self.sub_progress);
        }
        let c = checkboxes(&self.body);
        (c.1 > 0).then_some(c)
    }

    pub fn is_closed(&self) -> bool {
        self.state == "CLOSED" || self.state == "MERGED"
    }
}

/// The ids a write needs: the project, its Status field and each option's (name, id).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ids {
    pub project: String,
    pub status_field: String,
    pub options: Vec<(String, String)>,
}

impl Ids {
    pub fn option(&self, status: &str) -> Option<&str> {
        self.options.iter().find(|o| o.0 == status).map(|o| o.1.as_str())
    }
}

/// A card moved on the board. `status` is None when it stays in its column; with `place`, it
/// goes right after the item `after` in the project's order, or to the top without one.
#[derive(Clone, Debug, PartialEq)]
pub struct Move {
    pub item: String,
    pub status: Option<String>,
    pub place: bool,
    pub after: Option<String>,
}

/// What takes a move back: where the item was in the project's order, and its status.
#[derive(Clone, Debug, PartialEq)]
pub struct Undo {
    pub item: String,
    pub index: usize,
    pub status: String,
}

#[derive(Clone, Debug, Default)]
pub struct Board {
    /// Projects linked to the repository.
    pub projects: Vec<ProjectRef>,
    pub chosen: usize,
    /// The repository's "owner/name".
    pub slug: String,
    /// Status options in the project's order, with their colour name ("GREEN").
    pub statuses: Vec<(String, String)>,
    /// Every item of the project, all repositories and drafts included, in the project's order.
    pub items: Vec<Item>,
    /// Empty for the sample project, whose moves stay on screen.
    pub ids: Ids,
    /// The signed-in GitHub login, for "Mine only".
    pub me: String,
}

impl Board {
    pub fn project(&self) -> Option<&ProjectRef> {
        self.projects.get(self.chosen)
    }

    /// Whether an item is an issue or pull request of the open repository.
    pub fn in_repo(&self, it: &Item) -> bool {
        it.repo.eq_ignore_ascii_case(&self.slug)
    }

    pub fn order(&self) -> Vec<String> {
        self.statuses.iter().map(|s| s.0.clone()).collect()
    }

    /// The colour name GitHub gives a status option.
    pub fn status_color(&self, status: &str) -> Option<&str> {
        self.statuses.iter().find(|s| s.0 == status).map(|s| s.1.as_str())
    }

    /// Moves an item as GitHub will once the move is written: the new status, then the place
    /// right after `after` (the top without one). None when the item isn't there.
    pub fn apply(&mut self, m: &Move) -> Option<Undo> {
        let from = self.items.iter().position(|i| i.id == m.item)?;
        let mut it = self.items.remove(from);
        let undo = Undo { item: it.id.clone(), index: from, status: it.status.clone() };
        if let Some(s) = &m.status {
            it.status = s.clone();
        }
        let to = match (&m.place, &m.after) {
            (false, _) => from,
            (true, None) => 0,
            (true, Some(a)) => self.items.iter().position(|i| &i.id == a).map(|p| p + 1).unwrap_or(from),
        };
        self.items.insert(to.min(self.items.len()), it);
        Some(undo)
    }

    /// Puts an item back where `apply` found it.
    pub fn revert(&mut self, u: &Undo) {
        let Some(at) = self.items.iter().position(|i| i.id == u.item) else { return };
        let mut it = self.items.remove(at);
        it.status = u.status.clone();
        self.items.insert(u.index.min(self.items.len()), it);
    }
}

/// The item a card dropped at `slot` of a column goes after: the visible card above it, the
/// dragged one left out (None: the top). `column` is the column's cards as shown.
pub fn drop_after(column: &[&str], dragged: &str, slot: usize) -> Option<String> {
    let others: Vec<&str> = column.iter().copied().filter(|id| *id != dragged).collect();
    slot.checked_sub(1).and_then(|i| others.get(i.min(others.len().saturating_sub(1)))).map(|s| s.to_string())
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

/// The project's items with what a card shows, in the project's order (the order of the board's
/// columns, Done aside), and the ids its moves need. The owner can be an organisation or a user.
const ITEMS_QUERY: &str = "fragment V on ProjectV2ItemFieldValue{\
... on ProjectV2ItemFieldSingleSelectValue{name color field{... on ProjectV2FieldCommon{name}}} \
... on ProjectV2ItemFieldTextValue{text field{... on ProjectV2FieldCommon{name}}} \
... on ProjectV2ItemFieldNumberValue{number field{... on ProjectV2FieldCommon{name}}} \
... on ProjectV2ItemFieldDateValue{date field{... on ProjectV2FieldCommon{name}}} \
... on ProjectV2ItemFieldIterationValue{title field{... on ProjectV2FieldCommon{name}}}} \
fragment P on ProjectV2{id field(name:\"Status\"){... on ProjectV2SingleSelectField{id options{id name color}}} \
items(first:100,after:$cursor,orderBy:{field:POSITION,direction:ASC}){pageInfo{hasNextPage endCursor} nodes{id fieldValues(first:20){nodes{...V}} content{__typename \
... on DraftIssue{title body createdAt updatedAt assignees(first:10){nodes{login}}} \
... on Issue{number title url state body createdAt updatedAt issueType{name color} repository{nameWithOwner} \
labels(first:20){nodes{name color}} assignees(first:10){nodes{login}} subIssuesSummary{total completed} \
subIssues(first:20){nodes{number title state url}} parent{number title url state} closedByPullRequestsReferences(first:5){nodes{number title state url}}} \
... on PullRequest{number title url state body createdAt updatedAt repository{nameWithOwner} \
labels(first:20){nodes{name color}} assignees(first:10){nodes{login}}}}}}} \
query($owner:String!,$number:Int!,$cursor:String){repositoryOwner(login:$owner){\
... on Organization{projectV2(number:$number){...P}} ... on User{projectV2(number:$number){...P}}}}";

fn logins(v: &Value) -> Vec<String> {
    v["nodes"].as_array().map(|a| a.iter().filter_map(|x| x["login"].as_str().map(String::from)).collect()).unwrap_or_default()
}

fn linked_one(x: &Value) -> Option<Linked> {
    Some(Linked {
        number: x["number"].as_u64()?,
        title: x["title"].as_str().unwrap_or("").into(),
        state: x["state"].as_str().unwrap_or("").into(),
        url: x["url"].as_str().unwrap_or("").into(),
    })
}

fn linked(v: &Value) -> Vec<Linked> {
    v["nodes"].as_array().map(|a| a.iter().filter_map(linked_one).collect()).unwrap_or_default()
}

/// A number field's value without a useless ".0".
fn number_text(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// One project item from the GraphQL answer.
pub fn parse_item(n: &Value) -> Item {
    let c = &n["content"];
    let kind = match c["__typename"].as_str() {
        Some("Issue") => Kind::Issue,
        Some("PullRequest") => Kind::PullRequest,
        _ => Kind::Draft,
    };
    let mut status = String::new();
    let mut fields = Vec::new();
    for v in n["fieldValues"]["nodes"].as_array().into_iter().flatten() {
        let Some(name) = v["field"]["name"].as_str() else { continue };
        let (value, color) = if let Some(s) = v["name"].as_str() {
            (s.to_string(), v["color"].as_str().map(String::from))
        } else if let Some(t) = v["text"].as_str() {
            (t.to_string(), None)
        } else if let Some(x) = v["number"].as_f64() {
            (number_text(x), None)
        } else if let Some(d) = v["date"].as_str() {
            (d.to_string(), None)
        } else if let Some(t) = v["title"].as_str() {
            (t.to_string(), None)
        } else {
            continue;
        };
        match name {
            "Status" => status = value,
            "Title" => {}
            _ if !value.is_empty() => fields.push(Field { name: name.into(), value, color }),
            _ => {}
        }
    }
    Item {
        id: n["id"].as_str().unwrap_or("").into(),
        title: c["title"].as_str().unwrap_or("").into(),
        status: if status.is_empty() { NO_STATUS.into() } else { status },
        number: c["number"].as_u64(),
        kind,
        url: c["url"].as_str().unwrap_or("").into(),
        assignees: logins(&c["assignees"]),
        repo: c["repository"]["nameWithOwner"].as_str().unwrap_or("").into(),
        state: c["state"].as_str().unwrap_or("").into(),
        issue_type: c["issueType"]["name"].as_str().map(|t| (t.to_string(), c["issueType"]["color"].as_str().unwrap_or("GRAY").to_string())),
        labels: c["labels"]["nodes"]
            .as_array()
            .map(|a| a.iter().filter_map(|l| Some(Label { name: l["name"].as_str()?.into(), color: l["color"].as_str().unwrap_or("8b92a5").into() })).collect())
            .unwrap_or_default(),
        fields,
        body: c["body"].as_str().unwrap_or("").into(),
        created: c["createdAt"].as_str().map(parse_time).unwrap_or(0),
        updated: c["updatedAt"].as_str().map(parse_time).unwrap_or(0),
        sub_progress: (c["subIssuesSummary"]["completed"].as_u64().unwrap_or(0) as u32, c["subIssuesSummary"]["total"].as_u64().unwrap_or(0) as u32),
        sub_issues: linked(&c["subIssues"]),
        pull_requests: linked(&c["closedByPullRequestsReferences"]),
        parent: linked_one(&c["parent"]),
    }
}

/// The ids, the Status options (name, colour), the items and the cursor of the next page.
type ItemsPage = (Ids, Vec<(String, String)>, Vec<Item>, Option<String>);

/// One page of the items query.
pub fn parse_items_page(v: &Value) -> Result<ItemsPage, String> {
    if let Some(e) = v["errors"].as_array().and_then(|a| a.first()) {
        let m = e["message"].as_str().unwrap_or("").to_string();
        let t = e["type"].as_str().unwrap_or("");
        return Err(if t == "INSUFFICIENT_SCOPES" || m.contains("read:project") { NO_PROJECT_SCOPE.into() } else { format!("GitHub: {m}") });
    }
    let p = &v["data"]["repositoryOwner"]["projectV2"];
    if p.is_null() {
        return Err("GitHub didn't return the project: it may have been deleted, or you can't see it.".into());
    }
    let options = p["field"]["options"].as_array().map(|a| a.as_slice()).unwrap_or_default();
    let statuses = options.iter().filter_map(|x| Some((x["name"].as_str()?.to_string(), x["color"].as_str().unwrap_or("GRAY").to_string()))).collect();
    let ids = Ids {
        project: p["id"].as_str().unwrap_or("").into(),
        status_field: p["field"]["id"].as_str().unwrap_or("").into(),
        options: options.iter().filter_map(|x| Some((x["name"].as_str()?.to_string(), x["id"].as_str()?.to_string()))).collect(),
    };
    let items = p["items"]["nodes"].as_array().map(|a| a.iter().map(parse_item).collect()).unwrap_or_default();
    let next = if p["items"]["pageInfo"]["hasNextPage"].as_bool() == Some(true) { p["items"]["pageInfo"]["endCursor"].as_str().map(String::from) } else { None };
    Ok((ids, statuses, items, next))
}

/// Puts items in their status column: the project's order first, then any status it doesn't
/// list, then items without a status. Empty columns are left out unless `keep_empty`.
pub fn columns<T: Borrow<Item>>(order: &[String], items: impl IntoIterator<Item = T>, keep_empty: bool) -> Vec<(String, Vec<T>)> {
    let mut cols: Vec<(String, Vec<T>)> = order.iter().map(|s| (s.clone(), Vec::new())).collect();
    for it in items {
        let status = it.borrow().status.clone();
        match cols.iter_mut().find(|c| c.0 == status) {
            Some(c) => c.1.push(it),
            None => {
                let at = if status == NO_STATUS { cols.len() } else { cols.iter().position(|c| c.0 == NO_STATUS).unwrap_or(cols.len()) };
                cols.insert(at, (status, vec![it]));
            }
        }
    }
    if !keep_empty {
        cols.retain(|c| !c.1.is_empty());
    }
    cols
}

/// Checked and total task-list boxes ("- [x]", "* [ ]") in a Markdown body.
pub fn checkboxes(body: &str) -> (u32, u32) {
    let mut c = (0, 0);
    for l in body.lines() {
        let t = l.trim_start();
        let Some(rest) = t.strip_prefix("- [").or_else(|| t.strip_prefix("* [")).or_else(|| t.strip_prefix("+ [")) else { continue };
        let mut ch = rest.chars();
        let (mark, close) = (ch.next(), ch.next());
        if close != Some(']') {
            continue;
        }
        match mark {
            Some('x') | Some('X') => c = (c.0 + 1, c.1 + 1),
            Some(' ') => c.1 += 1,
            _ => {}
        }
    }
    c
}

/// Unix seconds from GitHub's "2026-11-04T13:48:36Z" (0 when it isn't in that form).
pub fn parse_time(s: &str) -> i64 {
    let n = |a: usize, b: usize| s.get(a..b).and_then(|x| x.parse::<i64>().ok());
    let (Some(y), Some(mo), Some(d)) = (n(0, 4), n(5, 7), n(8, 10)) else { return 0 };
    let (h, mi, se) = (n(11, 13).unwrap_or(0), n(14, 16).unwrap_or(0), n(17, 19).unwrap_or(0));
    // Days from the civil date (Howard Hinnant's algorithm).
    let y = if mo <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (mo + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    days * 86400 + h * 3600 + mi * 60 + se
}

/// Reads the board for a repository. `preferred` is the "owner#number" chosen last time.
/// Runs in a background thread: it makes a call to GitHub per 100 items, plus two.
pub fn load(slug: &str, preferred: Option<&str>) -> Result<Board, String> {
    let exe = find_gh().ok_or("The GitHub command line (gh) isn't installed. Install it with `brew install gh`, then run `gh auth login`.")?;
    let (owner, name) = slug.split_once('/').ok_or("Not a GitHub repository")?;
    let v = json(&exe, &["api", "graphql", "-f", &format!("query={PROJECTS_QUERY}"), "-F", &format!("owner={owner}"), "-F", &format!("name={name}")])?;
    let projects = parse_projects(&v);
    let me = gh(&exe, &["api", "user", "--jq", ".login"]).map(|s| s.trim().to_string()).unwrap_or_default();
    let mut board = Board { projects, me, slug: slug.to_string(), ..Default::default() };
    if board.projects.is_empty() {
        return Ok(board);
    }
    board.chosen = preferred.and_then(|k| board.projects.iter().position(|p| p.key() == k)).unwrap_or(0);
    let p = board.projects[board.chosen].clone();
    let num = p.number.to_string();
    let mut cursor: Option<String> = None;
    while board.items.len() < ITEM_LIMIT {
        let mut args = vec!["api".to_string(), "graphql".into(), "-f".into(), format!("query={ITEMS_QUERY}"), "-F".into(), format!("owner={}", p.owner), "-F".into(), format!("number={num}")];
        if let Some(c) = &cursor {
            args.extend(["-f".into(), format!("cursor={c}")]);
        }
        let a: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let (ids, statuses, items, next) = parse_items_page(&json(&exe, &a)?)?;
        if board.statuses.is_empty() {
            board.statuses = statuses;
            board.ids = ids;
        }
        board.items.extend(items);
        cursor = next;
        if cursor.is_none() {
            break;
        }
    }
    board.items.truncate(ITEM_LIMIT);
    Ok(board)
}

const SET_STATUS: &str = "mutation($project:ID!,$item:ID!,$field:ID!,$option:String!){updateProjectV2ItemFieldValue(input:\
{projectId:$project,itemId:$item,fieldId:$field,value:{singleSelectOptionId:$option}}){projectV2Item{id}}}";
const CLEAR_STATUS: &str = "mutation($project:ID!,$item:ID!,$field:ID!){clearProjectV2ItemFieldValue(input:{projectId:$project,itemId:$item,fieldId:$field}){projectV2Item{id}}}";
const SET_POSITION: &str = "mutation($project:ID!,$item:ID!,$after:ID){updateProjectV2ItemPosition(input:{projectId:$project,itemId:$item,afterId:$after}){clientMutationId}}";

/// The reason a write failed, from `gh`'s error or GitHub's answer; a missing scope says how to
/// give it.
fn write_error(res: Result<Value, String>) -> Result<(), String> {
    let v = res.map_err(|e| if e == NO_PROJECT_SCOPE { NO_WRITE_SCOPE.into() } else { e })?;
    let Some(e) = v["errors"].as_array().and_then(|a| a.first()) else { return Ok(()) };
    let m = e["message"].as_str().unwrap_or("");
    Err(if e["type"].as_str() == Some("INSUFFICIENT_SCOPES") || m.contains("scope") { NO_WRITE_SCOPE.into() } else { format!("GitHub: {m}") })
}

fn mutation(query: &str, vars: &[(&str, &str)]) -> Result<(), String> {
    let exe = find_gh().ok_or("The GitHub command line (gh) isn't installed. Install it with `brew install gh`, then run `gh auth login`.")?;
    let mut args = vec!["api".to_string(), "graphql".into(), "-f".into(), format!("query={query}")];
    for (k, v) in vars {
        args.extend(["-f".into(), format!("{k}={v}")]);
    }
    let a: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    write_error(json(&exe, &a))
}

/// Sets an item's Status field to an option, or clears it with None. Only that field changes:
/// the issue stays open or closed.
pub fn set_status(ids: &Ids, item: &str, option: Option<&str>) -> Result<(), String> {
    let base = [("project", ids.project.as_str()), ("item", item), ("field", ids.status_field.as_str())];
    match option {
        Some(o) => mutation(SET_STATUS, &[&base[..], &[("option", o)]].concat()),
        None => mutation(CLEAR_STATUS, &base),
    }
}

/// Puts an item right after `after` in the project's order, or at the top without one.
pub fn set_position(ids: &Ids, item: &str, after: Option<&str>) -> Result<(), String> {
    let mut vars = vec![("project", ids.project.as_str()), ("item", item)];
    if let Some(a) = after {
        vars.push(("after", a));
    }
    mutation(SET_POSITION, &vars)
}

/// Writes a move: its status, then its place. Runs in a background thread.
pub fn write_move(ids: &Ids, m: &Move) -> Result<(), String> {
    if let Some(s) = &m.status {
        let option = if s == NO_STATUS { None } else { Some(ids.option(s).ok_or_else(|| format!("The project has no status “{s}” any more. Refresh it."))?) };
        set_status(ids, &m.item, option)?;
    }
    if m.place {
        set_position(ids, &m.item, m.after.as_deref())?;
    }
    Ok(())
}

/// A board for the sample project, with something of every kind a card can show.
pub fn sample() -> Board {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    const DAY: i64 = 86400;
    let slug = "example/sample-project";
    let it = |n: u64, title: &str, status: &str, ty: &str, who: &[&str], age_days: i64| Item {
        id: format!("item-{n}"),
        title: title.into(),
        status: status.into(),
        number: Some(n),
        kind: Kind::Issue,
        url: format!("https://github.com/{slug}/issues/{n}"),
        assignees: who.iter().map(|s| s.to_string()).collect(),
        repo: slug.into(),
        state: if is_done(status) { "CLOSED".into() } else { "OPEN".into() },
        issue_type: Some((ty.into(), match ty { "Bug" => "RED", "Task" => "YELLOW", _ => "BLUE" }.into())),
        created: now - age_days * DAY,
        updated: now - age_days * DAY / 3,
        ..Default::default()
    };
    let label = |name: &str, color: &str| Label { name: name.into(), color: color.into() };
    let field = |name: &str, value: &str, color: Option<&str>| Field { name: name.into(), value: value.into(), color: color.map(String::from) };
    let sub = |n: u64, title: &str, closed: bool| Linked { number: n, title: title.into(), state: if closed { "CLOSED" } else { "OPEN" }.into(), url: format!("https://github.com/{slug}/issues/{n}") };
    let pr = |n: u64, state: &str| Linked { number: n, title: String::new(), state: state.into(), url: format!("https://github.com/{slug}/pull/{n}") };

    let mut items = Vec::new();
    let mut a = it(51, "Group repositories in the switcher", "In progress", "Feature", &["me"], 6);
    a.labels = vec![label("zone:sélecteur", "1cadf8"), label("ux", "cb73e1")];
    a.fields = vec![field("Priority", "P1", Some("RED")), field("Size", "M", Some("YELLOW")), field("Iteration", "Sprint 12", None)];
    a.sub_progress = (2, 4);
    a.sub_issues = vec![sub(53, "Remember the group of each repository", true), sub(54, "Drag a repository into a group", true), sub(55, "Rename a group", false), sub(56, "Collapse a group in the list", false)];
    a.body = "**As** someone with many repositories, **I want** to group them, **so that** the list stays short.\n\n## Acceptance criteria\n- [x] Right-click a repository to move it to a group\n- [x] Groups are kept in `repos.json`\n- [ ] Rename a group\n- [ ] Collapse a group\n\n### Notes\nGroups follow the parent folder when none is chosen, like `c2-poly` rather than `repo`.\n\n1. Read the list\n2. Group it\n".into();
    a.pull_requests = vec![pr(57, "OPEN")];
    items.push(a);
    let mut s53 = it(53, "Remember the group of each repository when the list is saved", "Done", "Task", &["me"], 5);
    s53.parent = Some(Linked { number: 51, title: "Group repositories in the switcher".into(), state: "OPEN".into(), url: format!("https://github.com/{slug}/issues/51") });
    s53.labels = vec![label("zone:sélecteur", "1cadf8")];
    items.push(s53);
    let mut b = it(52, "Show the linked issue in the toolbar when the branch name starts with its number", "Ready", "Feature", &[], 3);
    b.labels = vec![label("zone:historique", "64da38")];
    b.fields = vec![field("Priority", "P2", Some("ORANGE")), field("Estimate", "3", None)];
    items.push(b);
    let mut c = it(48, "Draw curved lines between lanes", "In review", "Task", &["me", "camille"], 9);
    c.labels = vec![label("zone:graphe", "ff9502"), label("good first issue", "7057ff")];
    c.body = "Curves instead of straight steps.\n\n- [x] Bezier between lanes\n- [x] Same colour as the branch\n- [ ] Check with 30 lanes\n".into();
    c.pull_requests = vec![pr(49, "OPEN")];
    c.fields = vec![field("Size", "S", Some("GREEN"))];
    items.push(c);
    let mut d = it(44, "Search commits by author", "Backlog", "Feature", &["sami"], 21);
    d.labels = vec![label("zone:historique", "64da38")];
    items.push(d);
    let mut e = it(46, "Crash when a stash message is empty", "In progress", "Bug", &["camille"], 2);
    e.labels = vec![label("bug", "d73a4a"), label("zone:git", "fbca04")];
    e.fields = vec![field("Priority", "P0", Some("RED"))];
    items.push(e);
    let mut f = it(45, "Keyboard shortcut for stash", "Backlog", "Task", &[], 30);
    f.labels = vec![label("zone:palette", "ededed")];
    items.push(f);
    let mut g = it(42, "Give every branch line its own colour", "Done", "Feature", &["camille"], 40);
    g.labels = vec![label("zone:graphe", "ff9502"), label("v0.1.0", "64da38")];
    g.pull_requests = vec![pr(43, "MERGED")];
    g.sub_progress = (3, 3);
    items.push(g);
    let mut h = it(40, "Fix the avatar colours on light screens", "Done", "Bug", &["me"], 50);
    h.pull_requests = vec![pr(41, "CLOSED")];
    items.push(h);
    items.push(Item {
        id: "draft-1".into(),
        title: "Ask design for the empty-state illustrations".into(),
        status: "Backlog".into(),
        assignees: vec!["camille".into()],
        body: "Three illustrations: no project, no access, nothing found.".into(),
        created: now - 4 * DAY,
        updated: now - DAY,
        ..Default::default()
    });
    let mut w = it(12, "Publish the download page", "Ready", "Task", &["sami"], 8);
    w.repo = "example/website".into();
    w.url = "https://github.com/example/website/issues/12".into();
    w.labels = vec![label("docs", "0075ca")];
    items.push(w);

    Board {
        ids: Ids::default(),
        projects: vec![ProjectRef { owner: "example".into(), number: 1, title: "Fourchette roadmap".into(), url: "https://github.com/orgs/example/projects/1".into() }],
        chosen: 0,
        slug: slug.into(),
        statuses: [("Backlog", "GRAY"), ("Ready", "YELLOW"), ("In progress", "BLUE"), ("In review", "PURPLE"), ("Done", "GREEN")].iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(),
        items,
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

    fn page() -> Value {
        json!({"data": {"repositoryOwner": {"projectV2": {"id": "PVT_1",
            "field": {"id": "F_status", "options": [{"id": "o1", "name": "Backlog", "color": "GRAY"}, {"id": "o2", "name": "In progress", "color": "BLUE"}, {"id": "o3", "name": "Done", "color": "GREEN"}]},
            "items": {"pageInfo": {"hasNextPage": true, "endCursor": "abc"}, "nodes": [
                {"id": "I1", "fieldValues": {"nodes": [
                    {},
                    {"text": "A", "field": {"name": "Title"}},
                    {"name": "In progress", "color": "BLUE", "field": {"name": "Status"}},
                    {"name": "P1", "color": "RED", "field": {"name": "Priority"}},
                    {"number": 3.0, "field": {"name": "Estimate"}},
                    {"number": 1.5, "field": {"name": "Hours"}},
                    {"title": "Sprint 4", "field": {"name": "Iteration"}},
                    {"date": "2026-11-20", "field": {"name": "Due"}}
                ]}, "content": {"__typename": "Issue", "number": 1, "title": "A", "url": "u1", "state": "OPEN",
                    "body": "- [x] one\n- [ ] two", "createdAt": "2026-11-04T13:48:36Z", "updatedAt": "2026-11-05T00:00:00Z",
                    "issueType": {"name": "Feature", "color": "BLUE"}, "repository": {"nameWithOwner": "o/r"},
                    "labels": {"nodes": [{"name": "zone:git", "color": "fbca04"}]}, "assignees": {"nodes": [{"login": "me"}]},
                    "subIssuesSummary": {"total": 2, "completed": 1},
                    "subIssues": {"nodes": [{"number": 7, "title": "Sub", "state": "CLOSED", "url": "u7"}]},
                    "closedByPullRequestsReferences": {"nodes": [{"number": 9, "title": "Fix", "state": "MERGED", "url": "u9"}]}}},
                {"id": "I2", "fieldValues": {"nodes": [{"name": "Done", "color": "GREEN", "field": {"name": "Status"}}]},
                    "content": {"__typename": "PullRequest", "number": 2, "title": "B", "state": "MERGED", "repository": {"nameWithOwner": "O/R"}}},
                {"id": "I3", "fieldValues": {"nodes": []}, "content": {"__typename": "DraftIssue", "title": "Draft", "body": "* [X] done"}},
                {"id": "I4", "fieldValues": {"nodes": [{"name": "Blocked", "field": {"name": "Status"}}]},
                    "content": {"__typename": "Issue", "number": 4, "title": "D", "repository": {"nameWithOwner": "o/other"},
                    "parent": {"number": 1, "title": "A", "url": "u1", "state": "OPEN"}}}
            ]}
        }}}})
    }

    #[test]
    fn reads_a_page_of_project_items() {
        let (ids, statuses, items, next) = parse_items_page(&page()).unwrap();
        assert_eq!(statuses[1], ("In progress".to_string(), "BLUE".to_string()));
        assert_eq!((ids.project.as_str(), ids.status_field.as_str(), ids.option("Done"), ids.option("Blocked")), ("PVT_1", "F_status", Some("o3"), None));
        assert_eq!(next.as_deref(), Some("abc"));
        let a = &items[0];
        assert_eq!((a.id.as_str(), a.status.as_str(), a.kind.clone(), a.number), ("I1", "In progress", Kind::Issue, Some(1)));
        assert_eq!(a.issue_type, Some(("Feature".into(), "BLUE".into())));
        assert_eq!(a.labels, [Label { name: "zone:git".into(), color: "fbca04".into() }]);
        assert_eq!(a.fields.iter().map(|f| format!("{}={}", f.name, f.value)).collect::<Vec<_>>(), ["Priority=P1", "Estimate=3", "Hours=1.5", "Iteration=Sprint 4", "Due=2026-11-20"]);
        assert_eq!(a.fields[0].color.as_deref(), Some("RED"));
        assert_eq!(a.created, 1793800116);
        assert_eq!(a.progress(), Some((1, 2)), "sub-issues win over the checkboxes");
        assert_eq!(a.sub_issues[0].state, "CLOSED");
        assert_eq!((a.pull_requests[0].number, a.pull_requests[0].state.as_str()), (9, "MERGED"));
        assert_eq!(items[1].kind, Kind::PullRequest);
        assert!(items[1].is_closed());
        assert_eq!((items[2].kind.clone(), items[2].status.as_str(), items[2].progress()), (Kind::Draft, NO_STATUS, Some((1, 1))));
        assert_eq!(items[3].progress(), None);
        assert_eq!(items[3].parent, Some(Linked { number: 1, title: "A".into(), state: "OPEN".into(), url: "u1".into() }));
        assert_eq!(a.parent, None, "an issue without a parent (null or missing) has none");
    }

    #[test]
    fn keeps_this_repository_and_orders_columns() {
        let (_, statuses, items, _) = parse_items_page(&page()).unwrap();
        let board = Board { slug: "o/r".into(), statuses, items, ..Default::default() };
        let mine: Vec<&Item> = board.items.iter().filter(|i| board.in_repo(i)).collect();
        assert_eq!(mine.iter().map(|i| i.title.as_str()).collect::<Vec<_>>(), ["A", "B"]);
        let cols = columns(&board.order(), board.items.iter(), false);
        assert_eq!(cols.iter().map(|c| c.0.as_str()).collect::<Vec<_>>(), ["In progress", "Done", "Blocked", NO_STATUS]);
        let cols = columns(&board.order(), mine, true);
        assert_eq!(cols.iter().map(|c| (c.0.as_str(), c.1.len())).collect::<Vec<_>>(), [("Backlog", 0), ("In progress", 1), ("Done", 1)]);
        assert_eq!(board.status_color("Done"), Some("GREEN"));
    }

    #[test]
    fn reports_missing_scope_and_project() {
        let v = json!({"errors": [{"type": "INSUFFICIENT_SCOPES", "message": "Your token has not been granted the required scopes"}]});
        assert_eq!(parse_items_page(&v).unwrap_err(), NO_PROJECT_SCOPE);
        assert!(parse_items_page(&json!({"data": {"repositoryOwner": {"projectV2": null}}})).is_err());
        assert_eq!(write_error(Ok(v)).unwrap_err(), NO_WRITE_SCOPE);
        assert_eq!(write_error(Err(NO_PROJECT_SCOPE.into())).unwrap_err(), NO_WRITE_SCOPE, "a write asks for the right to change");
        assert_eq!(write_error(Ok(json!({"errors": [{"message": "Could not resolve to a node"}]}))).unwrap_err(), "GitHub: Could not resolve to a node");
        assert_eq!(write_error(Ok(json!({"data": {"x": {"clientMutationId": null}}}))), Ok(()));
    }

    fn board(cards: &[(&str, &str)]) -> Board {
        Board { items: cards.iter().map(|(id, s)| Item { id: id.to_string(), status: s.to_string(), ..Default::default() }).collect(), ..Default::default() }
    }

    fn order(b: &Board) -> Vec<String> {
        b.items.iter().map(|i| format!("{}:{}", i.id, i.status)).collect()
    }

    #[test]
    fn finds_the_card_a_drop_goes_after() {
        let col = ["a", "b", "c"];
        assert_eq!(drop_after(&col, "x", 0), None, "the top of the column");
        assert_eq!(drop_after(&col, "x", 2).as_deref(), Some("b"));
        assert_eq!(drop_after(&col, "x", 9).as_deref(), Some("c"), "past the end: after the last");
        assert_eq!(drop_after(&col, "a", 1).as_deref(), Some("b"), "the dragged card is left out");
        assert_eq!(drop_after(&col, "c", 2).as_deref(), Some("b"));
        assert_eq!(drop_after(&[], "x", 1), None);
    }

    #[test]
    fn applies_and_takes_back_a_move() {
        let mut b = board(&[("a", "Todo"), ("b", "Doing"), ("c", "Todo"), ("d", "Doing")]);
        let m = Move { item: "c".into(), status: Some("Doing".into()), place: true, after: Some("b".into()) };
        let u = b.apply(&m).unwrap();
        assert_eq!(order(&b), ["a:Todo", "b:Doing", "c:Doing", "d:Doing"]);
        b.revert(&u);
        assert_eq!(order(&b), ["a:Todo", "b:Doing", "c:Todo", "d:Doing"]);
        // To the top, in the same column.
        let u = b.apply(&Move { item: "d".into(), status: None, place: true, after: None }).unwrap();
        assert_eq!(order(&b), ["d:Doing", "a:Todo", "b:Doing", "c:Todo"]);
        b.revert(&u);
        assert_eq!(order(&b), ["a:Todo", "b:Doing", "c:Todo", "d:Doing"]);
        // Done keeps its date order: the status only, the place in the project unchanged.
        b.apply(&Move { item: "a".into(), status: Some("Done".into()), place: false, after: None }).unwrap();
        assert_eq!(order(&b), ["a:Done", "b:Doing", "c:Todo", "d:Doing"]);
        // After a card further down.
        b.apply(&Move { item: "b".into(), status: Some("Todo".into()), place: true, after: Some("c".into()) }).unwrap();
        assert_eq!(order(&b), ["a:Done", "c:Todo", "b:Todo", "d:Doing"]);
        assert_eq!(b.apply(&Move { item: "zz".into(), status: None, place: true, after: None }), None);
        let mut columns = columns(&["Todo".to_string()], b.items.iter(), false).into_iter();
        assert_eq!(columns.next().map(|c| c.1.iter().map(|i| i.id.as_str()).collect::<Vec<_>>()), Some(vec!["c", "b"]), "a column follows the project's order");
    }

    #[test]
    fn counts_checkboxes_and_reads_times() {
        assert_eq!(checkboxes("- [x] a\n  - [ ] b\n* [X] c\n- [y] no\n-[ ] no\ntext [ ]"), (2, 3));
        assert_eq!(parse_time("1970-01-02T00:00:01Z"), 86401);
        assert_eq!(parse_time("2000-03-01T00:00:00Z"), 951868800);
        assert_eq!(parse_time("nope"), 0);
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
        let it = Item { title: "Update validation process (front)!".into(), status: "Ready".into(), number: Some(38), kind: Kind::Issue, ..Default::default() };
        assert_eq!(branch_name(&it), "38-update-validation-process-front");
        assert!(is_done("Done") && is_done("Terminé") && !is_done("In progress"));
    }
}

/// Reads real projects through `gh`: `cargo test live_board -- --ignored --nocapture`.
#[cfg(test)]
mod live {
    #[test]
    #[ignore]
    fn live_board() {
        let b = super::load("C2-POLY/fourchette", None).expect("board");
        println!("projects: {:?}", b.projects.iter().map(|p| p.key()).collect::<Vec<_>>());
        for (s, items) in super::columns(&b.order(), b.items.iter(), false) {
            let line: Vec<String> = items.iter().map(|i| format!("#{} {:?} {:?}", i.number.unwrap_or(0), i.issue_type.as_ref().map(|t| &t.0), i.progress())).collect();
            println!("{s}: {}", line.join(", "));
        }
        assert!(b.projects.iter().any(|p| p.key() == "C2-POLY#1"));
        assert!(!b.statuses.is_empty());
        assert!(b.items.iter().any(|i| i.number == Some(6) && i.issue_type.is_some()));
        assert!(!b.me.is_empty());
    }
}
