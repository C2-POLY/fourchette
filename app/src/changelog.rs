//! The changelog Claude writes for one audience: every tag up to HEAD, one section per version,
//! newest first, and "Unreleased" with the commits since the last tag and the open pull requests.
//! Claude reads the commit subjects and the titles of the pull requests and issues they mention
//! (#12), which `gh` gives when the repository is on GitHub. Nothing is written until the person
//! copies the text, writes CHANGELOG.md or replaces a release's notes.
use crate::git::Repo;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub const UNRELEASED: &str = "Unreleased";
pub const FILE: &str = "CHANGELOG.md";
/// Most commit subjects sent per version; the oldest beyond are only counted.
pub const MAX_COMMITS: usize = 150;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Audience {
    #[default]
    Developers,
    Product,
    Leadership,
    Users,
}

impl Audience {
    pub const ALL: [Audience; 4] = [Audience::Developers, Audience::Product, Audience::Leadership, Audience::Users];

    pub fn label(self) -> &'static str {
        match self {
            Audience::Developers => "Developers",
            Audience::Product => "Product & team",
            Audience::Leadership => "Leadership & clients",
            Audience::Users => "End users",
        }
    }

    /// What the person gets, under the choice.
    pub fn line(self) -> &'static str {
        match self {
            Audience::Developers => "Every change, grouped by type, with its references",
            Audience::Product => "Features and fixes in terms of use, no internals",
            Audience::Leadership => "A few lines: the highlights and what they bring",
            Audience::Users => "Public release notes, simple and friendly",
        }
    }

    /// How Claude writes for them.
    fn brief(self) -> &'static str {
        match self {
            Audience::Developers => {
                "developers of the project. Be complete and technical: list every meaningful change, grouped under \
                 `### Added`, `### Changed`, `### Fixed` and `### Removed` (only the groups that have entries), one \
                 bullet each, and keep the references such as (#12). Leave out merge commits and pure noise."
            }
            Audience::Product => {
                "the product team. Describe new features, improvements and fixes in terms of what people can now do, \
                 in short bullets under `### New`, `### Improved` and `### Fixed` (only those that have entries). \
                 No code, file or function names; mention an issue number only when it helps to follow up."
            }
            Audience::Leadership => {
                "leadership and clients. Two to four sentences or bullets per version: the highlights and what they \
                 bring (value, risk removed, time saved). No technical terms and no issue numbers."
            }
            Audience::Users => {
                "the people who use the app. Friendly public release notes: what is new and what was fixed, in plain \
                 words, addressed to the reader (\"you can now…\"). Short bullets, no jargon, no issue numbers, and \
                 leave out internal work (refactoring, tooling, tests, docs)."
            }
        }
    }
}

/// One section of the changelog, before Claude writes it.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Version {
    /// The tag, or `UNRELEASED`.
    pub name: String,
    /// The tag's date (yyyy-mm-dd); empty for Unreleased.
    pub date: String,
    /// Commit subjects, newest first.
    pub commits: Vec<String>,
    /// Commits beyond `MAX_COMMITS`, only counted.
    pub more: usize,
}

/// What GitHub adds: the open pull requests, the titles of issues and pull requests by number,
/// and the tags that have a release.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct GitHub {
    pub slug: String,
    pub open_prs: Vec<(u64, String)>,
    pub titles: BTreeMap<u64, String>,
    pub releases: Vec<String>,
}

/// `git tag --format=%(refname:short)%09%(creatordate:short)` read as (tag, date).
pub fn parse_tags(out: &str) -> Vec<(String, String)> {
    out.lines()
        .filter_map(|l| {
            let (t, d) = l.split_once('\t').unwrap_or((l, ""));
            let t = t.trim();
            (!t.is_empty()).then(|| (t.to_string(), d.trim().to_string()))
        })
        .collect()
}

/// The `git log` range of each section, newest first: Unreleased is the last tag up to HEAD, each
/// tag starts after the one before it, and the oldest tag goes back to the first commit.
pub fn ranges(tags: &[(String, String)]) -> Vec<(String, String, String)> {
    let mut v = vec![(UNRELEASED.to_string(), String::new(), match tags.first() {
        Some((t, _)) => format!("{t}..HEAD"),
        None => "HEAD".to_string(),
    })];
    for (i, (t, d)) in tags.iter().enumerate() {
        let range = match tags.get(i + 1) {
            Some((older, _)) => format!("{older}..{t}"),
            None => t.clone(),
        };
        v.push((t.clone(), d.clone(), range));
    }
    v
}

/// The sections of the repository, newest first. Tags not reachable from HEAD are left out.
pub fn versions(repo: &Repo) -> Vec<Version> {
    let tags = parse_tags(&repo.git(&["tag", "--merged", "HEAD", "--sort=-creatordate", "--format=%(refname:short)%09%(creatordate:short)"]).unwrap_or_default());
    ranges(&tags)
        .into_iter()
        .map(|(name, date, range)| {
            let max = format!("-n{}", MAX_COMMITS);
            let commits: Vec<String> = repo.git(&["log", "--no-merges", "--format=%s", &max, &range, "--"]).unwrap_or_default().lines().map(str::to_string).collect();
            let total: usize = repo.git(&["rev-list", "--no-merges", "--count", &range, "--"]).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(commits.len());
            Version { name, date, more: total.saturating_sub(commits.len()), commits }
        })
        .collect()
}

/// Issue and pull request numbers mentioned in the subjects ("(#94)", "Closes #12").
pub fn mentioned(versions: &[Version]) -> Vec<u64> {
    let mut v: Vec<u64> = versions
        .iter()
        .flat_map(|ver| ver.commits.iter())
        .flat_map(|s| s.split('#').skip(1).filter_map(|rest| rest.chars().take_while(char::is_ascii_digit).collect::<String>().parse().ok()))
        .collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// Reads the repository's open pull requests, the titles of its issues and pull requests, and
/// its releases through `gh`. Blocks: call it from a thread.
pub fn github(slug: &str) -> Result<GitHub, String> {
    let exe = crate::github::find_gh().ok_or("The GitHub command line (gh) isn't installed.")?;
    let list = |args: &[&str]| -> Result<Vec<Value>, String> {
        let out = crate::github::gh(&exe, args)?;
        serde_json::from_str::<Vec<Value>>(&out).map_err(|_| "GitHub's answer wasn't in the expected form.".to_string())
    };
    let number_title = |v: &Value| Some((v["number"].as_u64()?, v["title"].as_str()?.to_string()));
    let mut gh = GitHub { slug: slug.to_string(), ..Default::default() };
    for pr in list(&["pr", "list", "-R", slug, "--state", "all", "--limit", "500", "--json", "number,title,state"])? {
        if let Some((n, t)) = number_title(&pr) {
            if pr["state"].as_str() == Some("OPEN") {
                gh.open_prs.push((n, t.clone()));
            }
            gh.titles.insert(n, t);
        }
    }
    for issue in list(&["issue", "list", "-R", slug, "--state", "all", "--limit", "500", "--json", "number,title"])? {
        if let Some((n, t)) = number_title(&issue) {
            gh.titles.insert(n, t);
        }
    }
    gh.releases = list(&["release", "list", "-R", slug, "--limit", "100", "--json", "tagName"])?.iter().filter_map(|r| r["tagName"].as_str().map(str::to_string)).collect();
    Ok(gh)
}

/// Replaces the notes of the release of `tag` on GitHub. Blocks: call it from a thread.
pub fn publish(slug: &str, tag: &str, notes: &str) -> Result<(), String> {
    let exe = crate::github::find_gh().ok_or("The GitHub command line (gh) isn't installed.")?;
    let file = std::env::temp_dir().join(format!("fourchette-notes-{}.md", std::process::id()));
    std::fs::write(&file, notes).map_err(|e| format!("Couldn't prepare the notes: {e}"))?;
    let res = crate::github::gh(&exe, &["release", "edit", tag, "-R", slug, "--notes-file", &file.to_string_lossy()]);
    let _ = std::fs::remove_file(&file);
    res.map(|_| ())
}

/// The question for Claude. Without `gh`, it works from the commits only.
pub fn prompt(versions: &[Version], gh: Option<&GitHub>, audience: Audience) -> String {
    let mut s = format!(
        "Write the changelog of this Git repository for {}\n\n\
         Return one entry per version below, in the same order, with `version` exactly as given. Each entry's \
         `notes` are Markdown without a heading for the version itself (the app adds it). Write in the language \
         of the commit messages. Only say what the commits and titles support; never invent. If a version has \
         nothing worth telling this audience, say so in one short line.\n",
        audience.brief()
    );
    if gh.is_none() {
        s.push_str("Only the commit subjects are known: there are no pull request or issue titles.\n");
    }
    let title = |n: u64| gh.and_then(|g| g.titles.get(&n));
    for v in versions.iter().filter(|v| shown(v, gh)) {
        s.push_str(&format!("\n## Version {}\n", v.name));
        if !v.date.is_empty() {
            s.push_str(&format!("Released on {}\n", v.date));
        }
        for c in &v.commits {
            let refs: Vec<String> = mentioned(std::slice::from_ref(&Version { commits: vec![c.clone()], ..Default::default() }))
                .into_iter()
                .filter_map(|n| title(n).map(|t| format!("#{n}: {t}")))
                .collect();
            if refs.is_empty() {
                s.push_str(&format!("- {c}\n"));
            } else {
                s.push_str(&format!("- {c}  [{}]\n", refs.join("; ")));
            }
        }
        if v.more > 0 {
            s.push_str(&format!("- …and {} older commits\n", v.more));
        }
        if v.name == UNRELEASED {
            if let Some(g) = gh.filter(|g| !g.open_prs.is_empty()) {
                s.push_str("Open pull requests (not merged yet, say they are coming):\n");
                for (n, t) in &g.open_prs {
                    s.push_str(&format!("- #{n} {t}\n"));
                }
            }
        }
    }
    s
}

pub fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "versions": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": { "version": { "type": "string" }, "notes": { "type": "string" } },
                    "required": ["version", "notes"]
                }
            }
        },
        "required": ["versions"]
    })
}

/// Claude's notes, one per section in the sections' order (empty when Claude left one out).
pub fn read_notes(v: &Value, versions: &[Version]) -> Vec<String> {
    let answers = v["versions"].as_array().cloned().unwrap_or_default();
    // The version as given, or followed by more ("v1.0.0 (2026-11-05)").
    let names = |a: &Value, name: &str, exact: bool| {
        let v = a["version"].as_str().unwrap_or("").trim();
        v == name || (!exact && v.strip_prefix(name).is_some_and(|rest| rest.starts_with([' ', '(', '-', ':'])))
    };
    versions
        .iter()
        .map(|ver| {
            let name = ver.name.as_str();
            answers
                .iter()
                .find(|a| names(a, name, true))
                .or_else(|| answers.iter().find(|a| names(a, name, false)))
                .and_then(|a| a["notes"].as_str())
                .unwrap_or("")
                .trim()
                .to_string()
        })
        .collect()
}

/// Whether a section has anything to tell: Unreleased with no commit and no open pull request
/// is left out.
pub fn shown(v: &Version, gh: Option<&GitHub>) -> bool {
    v.name != UNRELEASED || !v.commits.is_empty() || gh.is_some_and(|g| !g.open_prs.is_empty())
}

/// The whole changelog in Markdown, newest first.
pub fn markdown(versions: &[Version], notes: &[String], gh: Option<&GitHub>) -> String {
    let mut s = String::from("# Changelog\n");
    for (v, n) in versions.iter().zip(notes) {
        if !shown(v, gh) {
            continue;
        }
        if v.date.is_empty() {
            s.push_str(&format!("\n## {}\n\n", v.name));
        } else {
            s.push_str(&format!("\n## {} - {}\n\n", v.name, v.date));
        }
        s.push_str(n.trim());
        s.push('\n');
    }
    s
}

/// Writes CHANGELOG.md at the root of the repository and returns the Undo step: an existing file
/// goes to the Trash first (Undo brings it back over the new one), a new one is marked to go to
/// the Trash on Undo.
pub fn write(repo: &Repo, text: &str) -> Result<crate::undo::Snapshot, String> {
    let mut snap = repo.snapshot(&format!("Write {FILE}"), false, false);
    let path = repo.root.join(FILE);
    if path.exists() {
        snap.trashed = Some((FILE.to_string(), crate::git::trash(&repo.root, FILE)?));
    } else {
        snap.written = Some(FILE.to_string());
    }
    if let Err(e) = std::fs::write(&path, text) {
        if let Some((_, in_trash)) = &snap.trashed {
            let _ = std::fs::rename(in_trash, &path);
        }
        return Err(format!("Couldn't write {FILE}: {e}"));
    }
    Ok(snap)
}

/// The sample project's sections and notes, for the screenshot mode.
pub fn sample() -> (Vec<Version>, Vec<String>, GitHub) {
    let v = |name: &str, date: &str, commits: &[&str]| Version { name: name.into(), date: date.into(), commits: commits.iter().map(|s| s.to_string()).collect(), more: 0 };
    let versions = vec![
        v(UNRELEASED, "", &["Search commits by message, author or hash"]),
        v("v2.3.0", "2026-11-03", &["Give every branch line its own colour", "Draw curved lines between lanes", "Fix the sign-in timeout on slow networks"]),
        v("v2.2.1", "2026-10-28", &["Stash and pop local changes"]),
    ];
    let notes = vec![
        "### Added\n- Search commits by message, author or hash (#44, open pull request).".to_string(),
        "### Added\n- Every branch gets its own colour in the graph, and merges are drawn as curves (#51).\n\n### Fixed\n- Signing in no longer times out on slow networks (#46).".to_string(),
        "### Added\n- Stash local changes and pop them back from the toolbar.".to_string(),
    ];
    let gh = GitHub { slug: "example/sample-project".into(), open_prs: vec![(57, "Search commits by author".into())], titles: [(44, "Search commits by author"), (46, "Crash when a stash message is empty"), (51, "Group repositories in the switcher")].map(|(n, t)| (n, t.to_string())).into(), releases: vec!["v2.3.0".into(), "v2.2.1".into()] };
    (versions, notes, gh)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn ver(name: &str, commits: &[&str]) -> Version {
        Version { name: name.into(), date: if name == UNRELEASED { String::new() } else { "2026-11-05".into() }, commits: commits.iter().map(|s| s.to_string()).collect(), more: 0 }
    }

    #[test]
    fn ranges_go_from_tag_to_tag_newest_first() {
        let tags = parse_tags("v0.2.0\t2026-11-05\nv0.1.0\t2026-11-04\n");
        let r = ranges(&tags);
        assert_eq!(r.iter().map(|(n, _, r)| (n.as_str(), r.as_str())).collect::<Vec<_>>(), vec![(UNRELEASED, "v0.2.0..HEAD"), ("v0.2.0", "v0.1.0..v0.2.0"), ("v0.1.0", "v0.1.0")]);
        assert_eq!(r[1].1, "2026-11-05");
        assert_eq!(ranges(&[]), vec![(UNRELEASED.to_string(), String::new(), "HEAD".to_string())]);
    }

    #[test]
    fn mentioned_numbers_are_read_once() {
        let v = [ver("v1", &["feat: drag cards (#94)", "fix: closes #12 and #94", "chore: no ref", "docs: #x"])];
        assert_eq!(mentioned(&v), vec![12, 94]);
    }

    #[test]
    fn prompt_carries_the_audience_the_titles_and_open_prs() {
        let vs = [ver(UNRELEASED, &["wip (#7)"]), ver("v1.0.0", &["feat: board (#5)"])];
        let gh = GitHub { slug: "o/r".into(), open_prs: vec![(9, "Changelog".into())], titles: [(5, "Board drag".to_string())].into(), releases: vec![] };
        let p = prompt(&vs, Some(&gh), Audience::Leadership);
        assert!(p.contains("leadership and clients"));
        assert!(p.contains("- feat: board (#5)  [#5: Board drag]"));
        assert!(p.contains("- wip (#7)\n"), "an unknown number has no title");
        assert!(p.contains("- #9 Changelog"));
        assert!(p.contains("## Version v1.0.0\nReleased on 2026-11-05\n"));
        let p = prompt(&vs, None, Audience::Users);
        assert!(p.contains("Only the commit subjects are known"));
        assert!(!p.contains("Open pull requests"));
    }

    #[test]
    fn notes_follow_the_sections_and_markdown_skips_an_empty_unreleased() {
        let vs = [ver(UNRELEASED, &[]), ver("v1.0.0", &["a"]), ver("v0.9.0", &["b"])];
        let v = json!({"versions": [{"version": "v0.9.0 (2026-11-01)", "notes": "old"}, {"version": " v1.0.0 ", "notes": " new \n"}, {"version": "v1.0.0.1", "notes": "not v1.0.0"}]});
        let notes = read_notes(&v, &vs);
        assert_eq!(notes, vec!["", "new", "old"]);
        let md = markdown(&vs, &notes, None);
        assert_eq!(md, "# Changelog\n\n## v1.0.0 - 2026-11-05\n\nnew\n\n## v0.9.0 - 2026-11-05\n\nold\n");
        let gh = GitHub { open_prs: vec![(1, "x".into())], ..Default::default() };
        assert!(markdown(&vs, &notes, Some(&gh)).starts_with("# Changelog\n\n## Unreleased\n"));
    }

    fn repo() -> (std::path::PathBuf, Repo) {
        let dir = std::env::temp_dir().join(format!("fourchette-changelog-{}-{}", std::process::id(), rand_suffix()));
        std::fs::create_dir_all(&dir).unwrap();
        let g = |a: &[&str]| crate::git::run_in(&dir, a).unwrap();
        g(&["init", "-q", "-b", "main"]);
        g(&["config", "user.email", "t@example.com"]);
        g(&["config", "user.name", "T"]);
        g(&["commit", "-q", "--allow-empty", "-m", "first"]);
        g(&["tag", "v0.1.0"]);
        g(&["commit", "-q", "--allow-empty", "-m", "feat: second (#2)"]);
        let r = Repo { root: dir.clone() };
        (dir, r)
    }

    fn rand_suffix() -> u128 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    }

    #[test]
    fn versions_read_the_tags_of_a_repository() {
        let (dir, r) = repo();
        let vs = versions(&r);
        assert_eq!(vs.len(), 2);
        assert_eq!((vs[0].name.as_str(), vs[0].commits.clone()), (UNRELEASED, vec!["feat: second (#2)".to_string()]));
        assert_eq!((vs[1].name.as_str(), vs[1].commits.clone()), ("v0.1.0", vec!["first".to_string()]));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Calls GitHub and Claude for real on this repository: `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn live_changelog() {
        let r = Repo::open(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        let vs = versions(&r);
        let gh = github("C2-POLY/fourchette").unwrap();
        assert!(gh.titles.len() > 10 && !gh.releases.is_empty(), "{gh:?}");
        let v = crate::ai::ask(&prompt(&vs, Some(&gh), Audience::Leadership), crate::ai::SYSTEM, Some(&schema()), Default::default()).unwrap();
        let notes = read_notes(&v, &vs);
        let md = markdown(&vs, &notes, Some(&gh));
        println!("{md}");
        assert!(vs.iter().zip(&notes).filter(|(v, _)| v.name != UNRELEASED).all(|(_, n)| !n.is_empty()), "every tag has notes");
    }

    #[test]
    fn writing_the_file_is_undone_both_when_new_and_when_replaced() {
        let (dir, r) = repo();
        let path = dir.join(FILE);
        // New file: Undo sends it to the Trash.
        let snap = write(&r, "one").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "one");
        r.undo(&snap).unwrap();
        assert!(!path.exists());
        // Existing file: Undo puts the old one back.
        std::fs::write(&path, "mine").unwrap();
        let snap = write(&r, "two").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "two");
        r.undo(&snap).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "mine");
        let _ = std::fs::remove_dir_all(dir);
    }
}
