//! Cloning a repository (#204): the person's GitHub repositories and their organisations', read
//! with one `gh api graphql` query; what the search field takes (a URL, `owner/name`); where the
//! clone goes and whether that folder can take it; and `git clone --progress` in a thread, its
//! progress read from what Git writes on stderr. The window is `app/clone.rs`.
use crate::github::find_gh;
use serde_json::Value;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Receiver};

/// A repository on GitHub that the person can clone.
#[derive(Clone, Debug, PartialEq)]
pub struct Remote {
    /// "owner/name".
    pub slug: String,
    pub owner: String,
    pub name: String,
    pub description: String,
    pub private: bool,
    pub fork: bool,
    /// Unix seconds of its last update.
    pub updated: i64,
    pub https: String,
    pub ssh: String,
}

impl Remote {
    /// The address to clone it from, in the protocol `gh` is set to.
    pub fn url(&self, ssh: bool) -> String {
        if ssh { self.ssh.clone() } else { self.https.clone() }
    }
}

/// What `gh` gave: the person, their organisations and the repositories of all of them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Listing {
    pub login: String,
    pub orgs: Vec<String>,
    /// The most recently updated first.
    pub repos: Vec<Remote>,
    /// `gh config get git_protocol` is ssh: clone over SSH.
    pub ssh: bool,
}

/// The 100 most recently updated repositories of each account (the person, each organisation).
const QUERY: &str = "fragment R on Repository{nameWithOwner name owner{login} description isPrivate isFork updatedAt url sshUrl} \
query{viewer{login \
repositories(first:100,ownerAffiliations:[OWNER],orderBy:{field:UPDATED_AT,direction:DESC}){nodes{...R}} \
organizations(first:50){nodes{login repositories(first:100,orderBy:{field:UPDATED_AT,direction:DESC}){nodes{...R}}}}}}";

/// The person's repositories and their organisations', through `gh`, in the background.
pub fn list() -> Result<Listing, String> {
    let exe = find_gh().ok_or(NO_GH)?;
    let out = Command::new(&exe)
        .args(["api", "graphql", "-f", &format!("query={QUERY}")])
        .env("GH_PROMPT_DISABLED", "1")
        .env("NO_COLOR", "1")
        .output()
        .map_err(|e| format!("Couldn't start the GitHub command line: {e}"))?;
    // An organisation that refuses the app makes `gh` fail while the rest still came: keep it.
    let v: Value = serde_json::from_slice(&out.stdout).unwrap_or(Value::Null);
    if v["data"]["viewer"]["login"].is_null() {
        let err = String::from_utf8_lossy(&out.stderr).to_lowercase();
        return Err(if err.contains("gh auth login") || err.contains("not logged") || err.contains("authentication") {
            NOT_SIGNED_IN.into()
        } else {
            format!("GitHub: {}", String::from_utf8_lossy(&out.stderr).trim().chars().take(300).collect::<String>())
        });
    }
    let mut listing = parse_listing(&v);
    let proto = Command::new(&exe).args(["config", "get", "git_protocol", "-h", "github.com"]).output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    listing.ssh = proto == "ssh";
    Ok(listing)
}

pub const NOT_SIGNED_IN: &str = "The GitHub command line isn't signed in, so your repositories can't be listed. Sign in below, or paste a repository's address above.";
pub const NO_GH: &str = "The GitHub command line (gh) isn't installed, so your repositories can't be listed. Install it from Terminal, then open this window again — or paste a repository's address above.";
/// What installs `gh`, which the dialog copies (it never installs anything itself).
pub const INSTALL_GH: &str = "brew install gh";

fn remote(n: &Value) -> Option<Remote> {
    let slug = n["nameWithOwner"].as_str()?.to_string();
    let (owner, name) = slug.split_once('/')?;
    Some(Remote {
        owner: n["owner"]["login"].as_str().unwrap_or(owner).to_string(),
        name: n["name"].as_str().unwrap_or(name).to_string(),
        description: n["description"].as_str().unwrap_or("").trim().to_string(),
        private: n["isPrivate"].as_bool().unwrap_or(false),
        fork: n["isFork"].as_bool().unwrap_or(false),
        updated: crate::github::parse_time(n["updatedAt"].as_str().unwrap_or("")),
        https: n["url"].as_str().map(|u| format!("{u}.git")).unwrap_or_else(|| format!("https://github.com/{slug}.git")),
        ssh: n["sshUrl"].as_str().map(String::from).unwrap_or_else(|| format!("git@github.com:{slug}.git")),
        slug,
    })
}

/// The answer of `QUERY`: every repository once, the most recently updated first.
pub fn parse_listing(v: &Value) -> Listing {
    let viewer = &v["data"]["viewer"];
    let mut repos: Vec<Remote> = viewer["repositories"]["nodes"].as_array().into_iter().flatten().filter_map(remote).collect();
    let mut orgs = Vec::new();
    for o in viewer["organizations"]["nodes"].as_array().into_iter().flatten() {
        let Some(login) = o["login"].as_str() else { continue };
        orgs.push(login.to_string());
        repos.extend(o["repositories"]["nodes"].as_array().into_iter().flatten().filter_map(remote));
    }
    let mut seen = std::collections::HashSet::new();
    repos.retain(|r| seen.insert(r.slug.to_lowercase()));
    repos.sort_by_key(|r| std::cmp::Reverse(r.updated));
    Listing { login: viewer["login"].as_str().unwrap_or("").to_string(), orgs, repos, ssh: false }
}

/// The repositories of `account` (all when None) that match every word of `query`, in order.
pub fn filter<'a>(repos: &'a [Remote], query: &str, account: Option<&str>) -> Vec<&'a Remote> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    repos
        .iter()
        .filter(|r| account.is_none_or(|a| r.owner.eq_ignore_ascii_case(a)))
        .filter(|r| {
            let hay = format!("{} {}", r.slug, r.description).to_lowercase();
            words.iter().all(|w| hay.contains(w.as_str()))
        })
        .collect()
}

/// What the search field holds when it names a repository to clone.
#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    pub url: String,
    /// The folder `git clone` would make.
    pub name: String,
    /// "owner/name" when it is on GitHub.
    pub slug: Option<String>,
}

/// A URL (https, ssh, git, `git@host:path`, `github.com/owner/name`) or `owner/name`, which is on
/// GitHub, cloned over SSH when `ssh`. Anything else is a search.
pub fn parse_source(input: &str, ssh: bool) -> Option<Source> {
    let s = input.trim();
    if s.is_empty() || s.chars().any(char::is_whitespace) {
        return None;
    }
    let lower = s.to_lowercase();
    let scheme = ["https://", "http://", "ssh://", "git://", "file://"].iter().any(|p| lower.starts_with(p));
    let scp = !scheme && s.split_once(':').is_some_and(|(host, path)| host.contains('@') && !host.contains('/') && !path.is_empty());
    if scheme || scp {
        let name = folder_name(s)?;
        return Some(Source { url: s.to_string(), name, slug: crate::github::slug(s) });
    }
    let rest = lower.strip_prefix("github.com/").map(|_| &s["github.com/".len()..]).or_else(|| lower.strip_prefix("www.github.com/").map(|_| &s["www.github.com/".len()..]));
    let path = rest.unwrap_or(s).trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let (owner, name) = path.split_once('/')?;
    let ok = |p: &str| !p.is_empty() && !p.starts_with('.') && p.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c));
    if !ok(owner) || !ok(name) || owner.starts_with('-') {
        return None;
    }
    let slug = format!("{owner}/{name}");
    let url = if ssh { format!("git@github.com:{slug}.git") } else { format!("https://github.com/{slug}.git") };
    Some(Source { url, name: name.to_string(), slug: Some(slug) })
}

/// The folder `git clone` makes for `url`: its last part without ".git".
pub fn folder_name(url: &str) -> Option<String> {
    let u = url.trim().trim_end_matches('/');
    let u = u.strip_suffix(".git").unwrap_or(u).trim_end_matches('/');
    let last = u.rsplit(['/', ':']).next()?;
    (!last.is_empty() && last != "." && last != "..").then(|| last.to_string())
}

/// Why `dest` can't take the clone, if it can't: it may not exist yet, or be an empty folder.
pub fn check_dest(dest: &Path) -> Result<(), String> {
    match std::fs::symlink_metadata(dest) {
        Err(_) => Ok(()),
        Ok(m) if !m.is_dir() => Err(format!("{} is already there and isn't a folder: choose another folder.", tilde(dest))),
        Ok(_) => match std::fs::read_dir(dest).map(|mut d| d.next().is_none()) {
            Ok(true) => Ok(()),
            Ok(false) => Err(format!("{} already exists and isn't empty: choose another folder.", tilde(dest))),
            Err(e) => Err(format!("{} can't be read: {e}", tilde(dest))),
        },
    }
}

/// Where clones go the first time: `~/dev` when there is one, else the home folder.
pub fn default_parent(home: &Path, is_dir: impl Fn(&Path) -> bool) -> PathBuf {
    let dev = home.join("dev");
    if is_dir(&dev) { dev } else { home.to_path_buf() }
}

/// "~/dev/acme" rather than "/Users/me/dev/acme".
pub fn tilde(p: &Path) -> String {
    let s = p.display().to_string();
    match std::env::var("HOME") {
        Ok(h) if !h.is_empty() && s.starts_with(&h) => format!("~{}", &s[h.len()..]),
        _ => s,
    }
}

/// The GitHub "owner/name" of every remote in `git remote -v`'s output, lower-cased, once each.
pub fn slugs_of(remote_v: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for url in remote_v.lines().filter_map(|l| l.split_whitespace().nth(1)) {
        if let Some(s) = crate::github::slug(url).map(|s| s.to_lowercase()) {
            if !out.contains(&s) {
                out.push(s);
            }
        }
    }
    out
}

/// The GitHub repositories of each repository of the list whose folder is there.
pub fn known_slugs(paths: &[PathBuf]) -> Vec<(PathBuf, Vec<String>)> {
    paths
        .iter()
        .filter(|p| p.is_dir())
        .filter_map(|p| {
            let o = Command::new("git").arg("-C").arg(p).args(["remote", "-v"]).env("GIT_OPTIONAL_LOCKS", "0").output().ok().filter(|o| o.status.success())?;
            let s = slugs_of(&String::from_utf8_lossy(&o.stdout));
            (!s.is_empty()).then(|| (p.clone(), s))
        })
        .collect()
}

/// The repository of the list that is already a clone of `slug`, if one is.
pub fn already_cloned(slug: &str, known: &[(PathBuf, Vec<String>)]) -> Option<PathBuf> {
    let slug = slug.to_lowercase();
    known.iter().find(|(_, s)| s.contains(&slug)).map(|(p, _)| p.clone())
}

/// Where a clone stands, from a line Git wrote.
#[derive(Clone, Debug, PartialEq)]
pub struct Progress {
    /// Git's line ("Receiving objects:  45% (450/1000), 1.20 MiB | 2.00 MiB/s").
    pub text: String,
    /// The whole clone, from 0 to 1, when the line says.
    pub fraction: Option<f32>,
}

/// Each phase's share of the bar: the server counting and compressing, receiving, resolving
/// deltas, then writing the files.
const PHASES: [(&str, f32, f32); 6] = [
    ("Enumerating objects", 0.0, 0.02),
    ("Counting objects", 0.02, 0.04),
    ("Compressing objects", 0.04, 0.08),
    ("Receiving objects", 0.08, 0.85),
    ("Resolving deltas", 0.85, 0.95),
    ("Updating files", 0.95, 1.0),
];

/// A progress line of `git clone --progress` ("remote: " lines included); None for the others.
pub fn parse_progress(line: &str) -> Option<Progress> {
    let l = line.trim();
    let l = l.strip_prefix("remote:").map(str::trim).unwrap_or(l);
    if l.starts_with("Cloning into") {
        return Some(Progress { text: "Connecting…".into(), fraction: Some(0.0) });
    }
    let (phase, rest) = l.split_once(':')?;
    let &(_, from, to) = PHASES.iter().find(|p| p.0 == phase.trim())?;
    let pct = rest.split_whitespace().find_map(|w| w.strip_suffix('%')?.parse::<f32>().ok());
    let text = l.strip_suffix(", done.").unwrap_or(l).split_whitespace().collect::<Vec<_>>().join(" ");
    Some(Progress { text, fraction: Some(from + (to - from) * pct.unwrap_or(0.0).clamp(0.0, 100.0) / 100.0) })
}

/// What went wrong, from what Git wrote: its fatal and error lines, else its last line.
pub fn error_of(lines: &[String], code: Option<i32>) -> String {
    let bad: Vec<&str> = lines.iter().map(|l| l.trim()).filter(|l| ["fatal:", "error:", "ERROR:"].iter().any(|p| l.starts_with(p))).collect();
    let text = if bad.is_empty() { lines.iter().rev().map(|l| l.trim()).find(|l| !l.is_empty()).unwrap_or("").to_string() } else { bad.join("\n") };
    if text.is_empty() {
        format!("git clone stopped with code {}.", code.map_or("?".into(), |c| c.to_string()))
    } else {
        text
    }
}

/// What the clone's thread says.
pub enum Msg {
    Progress(Progress),
    /// The folder of the new repository, or why there is none.
    Done(Result<PathBuf, String>),
}

/// A clone under way.
pub struct Running {
    pub rx: Receiver<Msg>,
    pid: u32,
}

impl Running {
    /// Stops Git (SIGTERM, so it removes what it wrote); the thread then sends `Done(Err)`.
    pub fn stop(&self) {
        let _ = Command::new("kill").arg(self.pid.to_string()).status();
    }
}

/// Runs `git clone --progress <url> <dest>` in a thread, in English like the app's other Git
/// commands so its progress lines can be read; `repaint` is called at each message.
/// A GitHub address over HTTPS also asks `gh` for the person's credentials, after their own
/// helpers, so a private repository clones without a prompt.
pub fn start(url: &str, dest: &Path, repaint: impl Fn() + Send + 'static) -> Result<Running, String> {
    check_dest(dest)?;
    let existed = dest.exists();
    let mut cmd = Command::new("git");
    if url.to_lowercase().starts_with("https://github.com/") {
        if let Some(gh) = find_gh() {
            cmd.args(["-c", &format!("credential.https://github.com.helper=!'{}' auth git-credential", gh.display())]);
        }
    }
    cmd.args(["clone", "--progress", "--", url]).arg(dest).env("GIT_TERMINAL_PROMPT", "0").env("LC_ALL", "C").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("Couldn't start git: {e}"))?;
    let pid = child.id();
    let mut err = child.stderr.take().ok_or("Couldn't read git's output.")?;
    let (tx, rx) = channel();
    let dest = dest.to_path_buf();
    std::thread::spawn(move || {
        let mut lines: Vec<String> = Vec::new();
        read_lines(&mut err, |l| match parse_progress(&l) {
            Some(p) => {
                let _ = tx.send(Msg::Progress(p));
                repaint();
            }
            None => lines.push(l),
        });
        let status = child.wait();
        let res = match status {
            Ok(s) if s.success() => Ok(dest),
            Ok(s) => {
                // Whatever a stopped clone left in a folder it made goes too.
                if !existed && dest.exists() {
                    let _ = std::fs::remove_dir_all(&dest);
                }
                Err(if s.code().is_none() { "The clone was stopped.".into() } else { error_of(&lines, s.code()) })
            }
            Err(e) => Err(format!("git: {e}")),
        };
        let _ = tx.send(Msg::Done(res));
        repaint();
    });
    Ok(Running { rx, pid })
}

/// Calls `f` with each line `r` gives, a line ending at "\n" or at "\r" (Git and `gh` redraw
/// their progress with "\r"), until it ends.
fn read_lines(r: &mut impl Read, mut f: impl FnMut(String)) {
    let mut cur = Vec::new();
    let mut buf = [0u8; 4096];
    let mut flush = |cur: &mut Vec<u8>| {
        if !cur.is_empty() {
            f(String::from_utf8_lossy(cur).into_owned());
            cur.clear();
        }
    };
    while let Ok(n) = r.read(&mut buf) {
        if n == 0 {
            break;
        }
        for &b in &buf[..n] {
            if b == b'\r' || b == b'\n' {
                flush(&mut cur);
            } else {
                cur.push(b);
            }
        }
    }
    flush(&mut cur);
}

/// Where GitHub's device sign-in page is, when `gh` doesn't say.
pub const DEVICE_URL: &str = "https://github.com/login/device";

/// The one-time code in a line of `gh auth login --web`: "! One-time code (82F4-C47F) copied to
/// clipboard" without a terminal, "! First copy your one-time code: 82F4-C47F" with one.
pub fn login_code(line: &str) -> Option<String> {
    if !line.to_lowercase().contains("one-time code") {
        return None;
    }
    line.split(|c: char| c.is_whitespace() || "():.,".contains(c))
        .find(|w| w.len() == 9 && w.as_bytes()[4] == b'-' && w.chars().filter(|&c| c != '-').all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()))
        .map(String::from)
}

/// The address a line of `gh auth login --web` sends to ("Open this URL to continue in your web
/// browser: https://github.com/login/device").
pub fn login_url(line: &str) -> Option<String> {
    line.split_whitespace().find(|w| w.starts_with("https://")).map(|w| w.trim_end_matches(['.', ',', ')']).to_string())
}

/// `gh` asks for Enter before it opens the browser itself (with a terminal).
pub fn login_wants_enter(line: &str) -> bool {
    line.to_lowercase().contains("press enter")
}

/// `gh` says the sign-in went through.
pub fn login_done(line: &str) -> bool {
    let l = line.to_lowercase();
    l.contains("authentication complete") || l.contains("logged in as")
}

/// Why the sign-in failed, from what `gh` wrote: its lines without the code and the address.
pub fn login_error(lines: &[String], code: Option<i32>) -> String {
    let rest: Vec<&str> = lines.iter().map(|l| l.trim()).filter(|l| !l.is_empty() && login_code(l).is_none() && login_url(l).is_none()).collect();
    match (rest.is_empty(), code) {
        (true, None) => "The sign-in was stopped.".into(),
        (true, Some(c)) => format!("gh auth login stopped with code {c}."),
        (false, _) => rest.join("\n"),
    }
}

/// What the sign-in's thread says.
#[derive(Debug, PartialEq)]
pub enum LoginMsg {
    /// The code to type on GitHub's page, and that page.
    Code { code: String, url: String },
    Done(Result<(), String>),
}

/// `gh auth login --web` under way.
pub struct Login {
    pub rx: Receiver<LoginMsg>,
    pid: u32,
}

impl Login {
    /// Stops `gh`; the thread then sends `Done(Err)`.
    pub fn stop(&self) {
        let _ = Command::new("kill").arg(self.pid.to_string()).status();
    }
}

/// Signs `gh` in to github.com through the browser, in a thread: `gh auth login --web`, whose
/// one-time code and page are sent as soon as it writes them (without a terminal, gh 2.102 writes
/// both at once, copies the code to the clipboard and waits for the approval without opening the
/// browser). Git keeps cloning over HTTPS; nothing is asked about SSH keys.
pub fn login(repaint: impl Fn() + Send + Sync + 'static) -> Result<Login, String> {
    let exe = find_gh().ok_or(NO_GH)?;
    let mut child = Command::new(&exe)
        .args(["auth", "login", "--hostname", "github.com", "--web", "--git-protocol", "https", "--skip-ssh-key"])
        .env("NO_COLOR", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Couldn't start the GitHub command line: {e}"))?;
    let pid = child.id();
    let (mut out, mut err) = (child.stdout.take().ok_or("Couldn't read gh's output.")?, child.stderr.take().ok_or("Couldn't read gh's output.")?);
    let mut stdin = child.stdin.take();
    // Both streams into one, in the order they come.
    let (ltx, lrx) = channel::<String>();
    let ltx2 = ltx.clone();
    std::thread::spawn(move || read_lines(&mut out, |l| drop(ltx2.send(l))));
    std::thread::spawn(move || read_lines(&mut err, |l| drop(ltx.send(l))));
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let (mut code, mut url, mut sent, mut done) = (None, None, false, false);
        let mut lines = Vec::new();
        for l in lrx {
            done |= login_done(&l);
            if let Some(c) = login_code(&l) {
                code = Some(c);
            } else if login_wants_enter(&l) {
                if let Some(s) = stdin.as_mut() {
                    let _ = std::io::Write::write_all(s, b"\n");
                }
                url = url.or_else(|| login_url(&l));
            } else if let Some(u) = login_url(&l) {
                url = Some(u);
            } else {
                lines.push(l);
            }
            if let (Some(c), false) = (&code, sent) {
                // The address comes on the next line; without one, GitHub's usual page.
                if url.is_some() || lines.len() > 1 {
                    let _ = tx.send(LoginMsg::Code { code: c.clone(), url: url.clone().unwrap_or_else(|| DEVICE_URL.into()) });
                    sent = true;
                    repaint();
                }
            }
        }
        if let (Some(c), false) = (code, sent) {
            let _ = tx.send(LoginMsg::Code { code: c, url: url.unwrap_or_else(|| DEVICE_URL.into()) });
        }
        drop(stdin);
        let res = match child.wait() {
            // Signed in, even if a later step (setting up Git) failed.
            Ok(s) if s.success() || done => Ok(()),
            Ok(s) => Err(login_error(&lines, s.code())),
            Err(e) => Err(format!("gh: {e}")),
        };
        let _ = tx.send(LoginMsg::Done(res));
        repaint();
    });
    Ok(Login { rx, pid })
}

/// The screenshot mode's repositories, invented.
pub fn sample() -> Listing {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let r = |owner: &str, name: &str, description: &str, private: bool, fork: bool, hours: i64| Remote {
        slug: format!("{owner}/{name}"),
        owner: owner.into(),
        name: name.into(),
        description: description.into(),
        private,
        fork,
        updated: now - hours * 3600,
        https: format!("https://github.com/{owner}/{name}.git"),
        ssh: format!("git@github.com:{owner}/{name}.git"),
    };
    Listing {
        login: "camille".into(),
        orgs: vec!["acme".into(), "nightowl-labs".into()],
        repos: vec![
            r("acme", "storefront", "The shop's web front, in Astro", true, false, 2),
            r("camille", "dotfiles", "Shell, Git and editor settings", false, false, 5),
            r("acme", "payments-api", "Payments service: checkout, refunds and webhooks", true, false, 26),
            r("nightowl-labs", "lantern", "A tiny static site generator for night owls", false, false, 50),
            r("camille", "advent-of-code", "", false, false, 24 * 9),
            r("acme", "design-tokens", "Colours, type and spacing shared by every app", false, false, 24 * 15),
            r("camille", "ripgrep", "ripgrep recursively searches directories for a regex pattern", false, true, 24 * 40),
            r("nightowl-labs", "moth", "Command line for the lantern site generator", false, false, 24 * 200),
        ],
        ssh: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_name_and_urls_become_sources() {
        let s = parse_source("C2-POLY/fourchette", false).unwrap();
        assert_eq!(s, Source { url: "https://github.com/C2-POLY/fourchette.git".into(), name: "fourchette".into(), slug: Some("C2-POLY/fourchette".into()) });
        assert_eq!(parse_source("C2-POLY/fourchette", true).unwrap().url, "git@github.com:C2-POLY/fourchette.git");
        assert_eq!(parse_source("github.com/acme/api.git", false).unwrap().url, "https://github.com/acme/api.git");
        let u = parse_source("https://github.com/acme/web-site", false).unwrap();
        assert_eq!((u.url.as_str(), u.name.as_str(), u.slug.as_deref()), ("https://github.com/acme/web-site", "web-site", Some("acme/web-site")));
        let scp = parse_source("git@github.com:acme/api.git", false).unwrap();
        assert_eq!((scp.name.as_str(), scp.slug.as_deref()), ("api", Some("acme/api")));
        let other = parse_source("https://gitlab.com/group/sub/tool.git", false).unwrap();
        assert_eq!((other.name.as_str(), other.slug), ("tool", None));
    }

    #[test]
    fn searches_are_not_sources() {
        for q in ["", "storefront", "acme storefront", "a/b/c", "/abs", "acme/", "-x/y", "acme/.hidden"] {
            assert_eq!(parse_source(q, false), None, "{q}");
        }
    }

    #[test]
    fn folder_names_drop_git_and_slashes() {
        assert_eq!(folder_name("https://github.com/a/b.git").as_deref(), Some("b"));
        assert_eq!(folder_name("https://github.com/a/b/").as_deref(), Some("b"));
        assert_eq!(folder_name("git@host:b.git").as_deref(), Some("b"));
        assert_eq!(folder_name("https://"), None);
    }

    #[test]
    fn destination_must_be_new_or_empty() {
        let root = std::env::temp_dir().join(format!("fourchette-clone-dest-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("empty")).unwrap();
        std::fs::create_dir_all(root.join("full")).unwrap();
        std::fs::write(root.join("full/README"), "x").unwrap();
        std::fs::write(root.join("file"), "x").unwrap();
        assert!(check_dest(&root.join("new")).is_ok());
        assert!(check_dest(&root.join("empty")).is_ok());
        assert!(check_dest(&root.join("full")).unwrap_err().contains("isn't empty"));
        assert!(check_dest(&root.join("file")).unwrap_err().contains("isn't a folder"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn default_parent_is_dev_when_there_is_one() {
        let home = Path::new("/Users/me");
        assert_eq!(default_parent(home, |_| true), PathBuf::from("/Users/me/dev"));
        assert_eq!(default_parent(home, |_| false), PathBuf::from("/Users/me"));
    }

    #[test]
    fn progress_lines_fill_the_bar_in_order() {
        let p = parse_progress("Receiving objects:  50% (500/1000), 1.20 MiB | 2.00 MiB/s").unwrap();
        assert_eq!(p.text, "Receiving objects: 50% (500/1000), 1.20 MiB | 2.00 MiB/s");
        assert!((p.fraction.unwrap() - (0.08 + 0.77 * 0.5)).abs() < 1e-4);
        let r = parse_progress("remote: Counting objects: 100% (12/12), done.").unwrap();
        assert_eq!(r.text, "Counting objects: 100% (12/12)");
        assert!((r.fraction.unwrap() - 0.04).abs() < 1e-4);
        assert_eq!(parse_progress("Cloning into 'x'...").unwrap().fraction, Some(0.0));
        assert_eq!(parse_progress("Updating files: 100% (3/3), done.").unwrap().fraction, Some(1.0));
        assert_eq!(parse_progress("fatal: repository not found"), None);
        assert_eq!(parse_progress("warning: You appear to have cloned an empty repository."), None);
    }

    #[test]
    fn errors_keep_gits_fatal_lines() {
        let lines = vec!["remote: Repository not found.".to_string(), "fatal: repository 'https://github.com/a/b.git/' not found".to_string()];
        assert_eq!(error_of(&lines, Some(128)), "fatal: repository 'https://github.com/a/b.git/' not found");
        assert_eq!(error_of(&["something odd".into()], Some(1)), "something odd");
        assert_eq!(error_of(&[], Some(128)), "git clone stopped with code 128.");
    }

    #[test]
    fn listing_merges_accounts_newest_first() {
        let v: Value = serde_json::from_str(
            r#"{"data":{"viewer":{"login":"me",
            "repositories":{"nodes":[{"nameWithOwner":"me/old","name":"old","owner":{"login":"me"},"description":null,"isPrivate":false,"isFork":true,"updatedAt":"2024-01-01T00:00:00Z","url":"https://github.com/me/old","sshUrl":"git@github.com:me/old.git"}]},
            "organizations":{"nodes":[{"login":"acme","repositories":{"nodes":[
              {"nameWithOwner":"acme/new","name":"new","owner":{"login":"acme"},"description":" Shop ","isPrivate":true,"isFork":false,"updatedAt":"2026-01-01T00:00:00Z","url":"https://github.com/acme/new","sshUrl":"git@github.com:acme/new.git"},
              {"nameWithOwner":"me/old","name":"old","owner":{"login":"me"},"updatedAt":"2024-01-01T00:00:00Z"}]}}]}}}}"#,
        )
        .unwrap();
        let l = parse_listing(&v);
        assert_eq!(l.login, "me");
        assert_eq!(l.orgs, ["acme"]);
        let slugs: Vec<&str> = l.repos.iter().map(|r| r.slug.as_str()).collect();
        assert_eq!(slugs, ["acme/new", "me/old"]);
        assert_eq!(l.repos[0].description, "Shop");
        assert!(l.repos[0].private && l.repos[1].fork);
        assert_eq!(l.repos[0].url(false), "https://github.com/acme/new.git");
        assert_eq!(l.repos[0].url(true), "git@github.com:acme/new.git");
    }

    #[test]
    fn filter_by_account_and_words() {
        let l = sample();
        assert_eq!(filter(&l.repos, "", None).len(), l.repos.len());
        assert!(filter(&l.repos, "", Some("ACME")).iter().all(|r| r.owner == "acme"));
        let hits: Vec<&str> = filter(&l.repos, "site generator", None).iter().map(|r| r.name.as_str()).collect();
        assert_eq!(hits, ["lantern", "moth"]);
        assert!(filter(&l.repos, "lantern", Some("camille")).is_empty());
    }

    #[test]
    fn already_cloned_matches_any_remote_ignoring_case() {
        let remote_v = "origin\tgit@github.com:Acme/Storefront.git (fetch)\norigin\tgit@github.com:Acme/Storefront.git (push)\nfork\thttps://github.com/me/storefront (fetch)\nlocal\t/tmp/x (fetch)\n";
        assert_eq!(slugs_of(remote_v), ["acme/storefront", "me/storefront"]);
        let known = vec![(PathBuf::from("/dev/storefront"), slugs_of(remote_v))];
        assert_eq!(already_cloned("acme/storefront", &known), Some(PathBuf::from("/dev/storefront")));
        assert_eq!(already_cloned("ACME/StoreFront", &known), Some(PathBuf::from("/dev/storefront")));
        assert_eq!(already_cloned("acme/other", &known), None);
    }

    #[test]
    fn clones_a_local_repository_with_progress() {
        let root = std::env::temp_dir().join(format!("fourchette-clone-run-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let src = root.join("src");
        std::fs::create_dir_all(&src).unwrap();
        let git = |args: &[&str]| assert!(Command::new("git").arg("-C").arg(&src).args(args).output().unwrap().status.success(), "{args:?}");
        git(&["init", "-q"]);
        std::fs::write(src.join("a.txt"), "a").unwrap();
        git(&["add", "."]);
        git(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "a"]);
        let dest = root.join("dest");
        let url = format!("file://{}", src.display());
        let run = start(&url, &dest, || {}).unwrap();
        let mut done = None;
        while let Ok(m) = run.rx.recv() {
            if let Msg::Done(r) = m {
                done = Some(r);
            }
        }
        assert_eq!(done, Some(Ok(dest.clone())));
        assert!(dest.join("a.txt").is_file());
        // The same folder again is refused, and a missing source says why.
        assert!(start(&url, &dest, || {}).is_err());
        let run = start(&format!("file://{}", root.join("nothing").display()), &root.join("dest2"), || {}).unwrap();
        let err = std::iter::from_fn(|| run.rx.recv().ok()).find_map(|m| match m {
            Msg::Done(r) => Some(r),
            _ => None,
        });
        assert!(matches!(err, Some(Err(ref e)) if e.contains("fatal")), "{err:?}");
        assert!(!root.join("dest2").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Live: a small public repository from GitHub, through the same path as the dialog.
    #[test]
    #[ignore]
    fn clones_a_public_repository_from_github() {
        let dest = std::env::temp_dir().join(format!("fourchette-clone-live-{}", std::process::id())).join("Hello-World");
        let src = parse_source("octocat/Hello-World", false).unwrap();
        let run = start(&src.url, &dest, || {}).unwrap();
        let mut seen = 0;
        let mut done = None;
        while let Ok(m) = run.rx.recv() {
            match m {
                Msg::Progress(p) => {
                    seen += 1;
                    eprintln!("{:>5.1}%  {}", p.fraction.unwrap_or(0.0) * 100.0, p.text);
                }
                Msg::Done(r) => done = Some(r),
            }
        }
        assert_eq!(done, Some(Ok(dest.clone())));
        assert!(seen > 0 && dest.join("README").is_file());
        let _ = std::fs::remove_dir_all(dest.parent().unwrap());
    }

    #[test]
    fn sign_in_reads_gh_without_a_terminal() {
        // gh 2.102 without a TTY (stderr), then the lines it writes with one.
        let no_tty = ["", "! One-time code (82F4-C47F) copied to clipboard", "Open this URL to continue in your web browser: https://github.com/login/device"];
        assert_eq!(no_tty.iter().find_map(|l| login_code(l)).as_deref(), Some("82F4-C47F"));
        assert_eq!(no_tty.iter().find_map(|l| login_url(l)).as_deref(), Some("https://github.com/login/device"));
        assert_eq!(login_code("! First copy your one-time code: ABCD-1234"), Some("ABCD-1234".into()));
        assert!(login_wants_enter("Press Enter to open https://github.com/login/device in your browser..."));
        assert_eq!(login_code("Press Enter to open https://github.com/login/device in your browser..."), None);
        assert_eq!(login_code("Logged in as ABCD-1234"), None);
        assert!(login_done("✓ Authentication complete."));
        assert!(login_done("✓ Logged in as octocat"));
        assert!(!login_done("! One-time code (82F4-C47F) copied to clipboard"));
    }

    #[test]
    fn sign_in_errors_leave_out_the_code_and_page() {
        let lines: Vec<String> = ["! One-time code (82F4-C47F) copied to clipboard", "Open this URL to continue in your web browser: https://github.com/login/device", "error: authorization expired"].map(String::from).to_vec();
        assert_eq!(login_error(&lines, Some(1)), "error: authorization expired");
        assert_eq!(login_error(&lines[..2], None), "The sign-in was stopped.");
        assert_eq!(login_error(&[], Some(1)), "gh auth login stopped with code 1.");
    }
}
