//! The home page's model (issue #109): the repository list in sections, favourites and recent
//! ones first, then the groups made by hand, then one section per GitHub organisation (read from
//! the remote) and the repositories that are not on GitHub. Inside a section, repositories whose
//! names start with the same word are proposed together, to keep as a group or dismiss.
//! `app/home.rs` draws it.
use crate::library::Entry;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// How many repositories "Recent" shows, favourites left out.
pub const RECENT_SHOWN: usize = 6;
/// How many repositories must share a first word before they are proposed together.
pub const PROPOSAL_MIN: usize = 2;

/// What the home page reads from a repository's folder, in the background, beyond the list.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RepoInfo {
    /// The GitHub owner (organisation or person) of its remote, "origin" first.
    pub org: Option<String>,
}

impl RepoInfo {
    /// Reads `path`'s remotes (`git remote -v`); a folder that is gone or not a repository has none.
    pub fn read(path: &Path) -> RepoInfo {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(path)
            .args(["remote", "-v"])
            .env("GIT_OPTIONAL_LOCKS", "0")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default();
        RepoInfo { org: org_of(&out) }
    }
}

/// The GitHub owner in `git remote -v`'s output: origin's when it is on GitHub, else the first
/// GitHub remote's.
pub fn org_of(remote_v: &str) -> Option<String> {
    let remotes: Vec<(&str, &str)> = remote_v
        .lines()
        .filter_map(|l| {
            let mut w = l.split_whitespace();
            Some((w.next()?, w.next()?))
        })
        .collect();
    let owner = |url: &str| crate::github::slug(url).and_then(|s| s.split('/').next().map(String::from));
    remotes.iter().filter(|r| r.0 == "origin").chain(remotes.iter()).find_map(|r| owner(r.1))
}

/// A name's first word, lower-cased: up to its first "-", "_" or "." ("fourchette-site" →
/// "fourchette"); the whole name when it has none.
pub fn first_word(name: &str) -> String {
    let w = name.split(['-', '_', '.']).next().unwrap_or(name);
    if w.is_empty() { name } else { w }.to_lowercase()
}

/// What a proposal is remembered by once dismissed: its section and its word ("C2-POLY/fourchette",
/// "/scripts" outside GitHub).
pub fn proposal_key(org: Option<&str>, word: &str) -> String {
    format!("{}/{word}", org.unwrap_or(""))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Favorites,
    Recent,
    /// A group made by hand (the switcher's "Move to group").
    Group,
    /// A GitHub organisation, or person.
    Org,
    /// Repositories without a GitHub remote, or not read yet.
    Other,
    /// What a search finds.
    Matches,
}

/// Repositories proposed together because their names start with the same word.
#[derive(Clone, Debug, PartialEq)]
pub struct Proposal {
    /// The group they would go to: the word as the first of them spells it.
    pub name: String,
    pub key: String,
}

/// Cards drawn together: a proposal's, or the section's other repositories.
#[derive(Clone, Debug, PartialEq)]
pub struct Cluster {
    pub proposal: Option<Proposal>,
    pub entries: Vec<Entry>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub title: String,
    pub kind: Kind,
    pub clusters: Vec<Cluster>,
}

impl Section {
    pub fn len(&self) -> usize {
        self.clusters.iter().map(|c| c.entries.len()).sum()
    }
}

fn by_name(v: &mut [&Entry]) {
    v.sort_by_key(|e| (e.name().to_lowercase(), e.path.clone()));
}

fn section(title: impl Into<String>, kind: Kind, entries: Vec<&Entry>) -> Section {
    Section { title: title.into(), kind, clusters: vec![Cluster { proposal: None, entries: entries.into_iter().cloned().collect() }] }
}

/// A section's repositories, with those sharing a first word proposed together (unless
/// `propose` is off or the proposal was dismissed), then the others.
fn clustered(org: Option<&str>, mut entries: Vec<&Entry>, propose: bool, dismissed: &[String]) -> Vec<Cluster> {
    by_name(&mut entries);
    let mut words: Vec<(String, Vec<&Entry>)> = Vec::new();
    for e in entries {
        let w = first_word(&e.name());
        match words.iter_mut().find(|x| x.0 == w) {
            Some(x) => x.1.push(e),
            None => words.push((w, vec![e])),
        }
    }
    let mut out = Vec::new();
    let mut rest: Vec<&Entry> = Vec::new();
    for (w, v) in words {
        let key = proposal_key(org, &w);
        if propose && v.len() >= PROPOSAL_MIN && !dismissed.contains(&key) {
            let first = v[0].name();
            let name = first.chars().take(w.chars().count()).collect();
            out.push(Cluster { proposal: Some(Proposal { name, key }), entries: v.into_iter().cloned().collect() });
        } else {
            rest.extend(v);
        }
    }
    if !rest.is_empty() {
        by_name(&mut rest);
        out.push(Cluster { proposal: None, entries: rest.into_iter().cloned().collect() });
    }
    out
}

/// Whether every word of `query` is in the repository's name, group, folder or organisation.
pub fn matches(query: &str, e: &Entry, org: Option<&str>) -> bool {
    let t = format!("{} {} {} {}", e.name(), e.group_name(), e.path.display(), org.unwrap_or("")).to_lowercase();
    query.split_whitespace().all(|w| t.contains(&w.to_lowercase()))
}

/// The home page's sections. With a `query`, one "Matches" section, favourites and the most
/// recently opened first. Without, Favorites and Recent at the top (those repositories also stay
/// in their own section below), then the groups made by hand, the organisations and the rest,
/// each in name order. A repository in a group made by hand is never proposed elsewhere.
pub fn sections(entries: &[Entry], info: &HashMap<PathBuf, RepoInfo>, dismissed: &[String], query: &str) -> Vec<Section> {
    let org = |e: &Entry| info.get(&e.path).and_then(|i| i.org.clone());
    if !query.trim().is_empty() {
        let mut v: Vec<&Entry> = entries.iter().filter(|e| matches(query, e, org(e).as_deref())).collect();
        v.sort_by_key(|e| (std::cmp::Reverse(e.favorite), std::cmp::Reverse(e.last_opened), e.name().to_lowercase()));
        return if v.is_empty() { vec![] } else { vec![section("Matches", Kind::Matches, v)] };
    }
    let mut out = Vec::new();
    let mut favs: Vec<&Entry> = entries.iter().filter(|e| e.favorite).collect();
    by_name(&mut favs);
    if !favs.is_empty() {
        out.push(section("Favorites", Kind::Favorites, favs));
    }
    let mut recent: Vec<&Entry> = entries.iter().filter(|e| !e.favorite && e.last_opened > 0).collect();
    recent.sort_by_key(|e| std::cmp::Reverse(e.last_opened));
    recent.truncate(RECENT_SHOWN);
    if !recent.is_empty() {
        out.push(section("Recent", Kind::Recent, recent));
    }

    let mut groups: Vec<(String, Vec<&Entry>)> = Vec::new();
    let mut orgs: Vec<(String, Vec<&Entry>)> = Vec::new();
    let mut other: Vec<&Entry> = Vec::new();
    for e in entries {
        let (list, name) = match (&e.group, org(e)) {
            (Some(g), _) => (&mut groups, g.clone()),
            (None, Some(o)) => (&mut orgs, o),
            (None, None) => {
                other.push(e);
                continue;
            }
        };
        // Organisations differ in case only in how a remote was typed: "c2-poly" is C2-POLY.
        match list.iter_mut().find(|x| x.0.eq_ignore_ascii_case(&name)) {
            Some(x) => x.1.push(e),
            None => list.push((name, vec![e])),
        }
    }
    groups.sort_by_key(|x| x.0.to_lowercase());
    orgs.sort_by_key(|x| x.0.to_lowercase());
    for (g, v) in groups {
        out.push(Section { title: g, kind: Kind::Group, clusters: clustered(None, v, false, dismissed) });
    }
    for (o, v) in orgs {
        let clusters = clustered(Some(&o), v, true, dismissed);
        out.push(Section { title: o, kind: Kind::Org, clusters });
    }
    if !other.is_empty() {
        out.push(Section { title: "Not on GitHub".into(), kind: Kind::Other, clusters: clustered(None, other, true, dismissed) });
    }
    out
}

/// For `FOURCHETTE_HOME=1` screenshots: invented repositories, with their organisations.
pub fn sample() -> (Vec<Entry>, HashMap<PathBuf, RepoInfo>) {
    let now = crate::library::now();
    /// Folder, organisation, favourite, opened that many hours ago, group made by hand.
    type Sample = (&'static str, Option<&'static str>, bool, i64, Option<&'static str>);
    const REPOS: [Sample; 14] = [
        ("acme/fourchette", Some("acme-labs"), true, 1, None),
        ("acme/fourchette-site", Some("acme-labs"), false, 3, None),
        ("acme/fourchette-board-drag", Some("acme-labs"), false, 26, None),
        ("acme/pinball-fw", Some("acme-labs"), false, 0, None),
        ("acme/api-gateway", Some("acme-labs"), true, 50, None),
        ("nova/nova-web", Some("novamoon"), false, 5, None),
        ("nova/nova-mobile", Some("novamoon"), false, 120, None),
        ("nova/nova-design-system", Some("novamoon"), false, 0, None),
        ("nova/infra", Some("novamoon"), false, 400, None),
        ("me/dotfiles", Some("jdoe"), false, 30, None),
        ("me/blog", Some("jdoe"), false, 0, Some("Side projects")),
        ("me/recipes", None, false, 0, Some("Side projects")),
        ("me/scripts", None, false, 2, None),
        ("me/scratch", None, false, 0, None),
    ];
    let mut entries = Vec::new();
    let mut info = HashMap::new();
    for (folder, org, favorite, hours, group) in REPOS {
        let path = PathBuf::from("/Users/me/dev").join(folder);
        info.insert(path.clone(), RepoInfo { org: org.map(String::from) });
        let last_opened = if hours > 0 { now - hours * 3600 } else { 0 };
        entries.push(Entry { path, group: group.map(String::from), favorite, last_opened, project: None, details: None });
    }
    (entries, info)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(path: &str) -> Entry {
        Entry { path: PathBuf::from(path), group: None, favorite: false, last_opened: 0, project: None, details: None }
    }

    fn titles(s: &[Section]) -> Vec<(&str, usize)> {
        s.iter().map(|x| (x.title.as_str(), x.len())).collect()
    }

    fn names(c: &Cluster) -> Vec<String> {
        c.entries.iter().map(|e| e.name()).collect()
    }

    #[test]
    fn the_owner_comes_from_origin_then_any_github_remote() {
        let v = "fork\tgit@github.com:me/fourchette.git (fetch)\nfork\tgit@github.com:me/fourchette.git (push)\norigin\thttps://github.com/C2-POLY/fourchette.git (fetch)\n";
        assert_eq!(org_of(v).as_deref(), Some("C2-POLY"));
        let v = "origin\tgit@gitlab.com:team/app.git (fetch)\nupstream\tssh://git@github.com/acme/app (fetch)\n";
        assert_eq!(org_of(v).as_deref(), Some("acme"));
        assert_eq!(org_of("origin\t/srv/git/app.git (fetch)\n"), None);
        assert_eq!(org_of(""), None);
    }

    #[test]
    fn first_word_stops_at_a_separator() {
        assert_eq!(first_word("fourchette-board-drag"), "fourchette");
        assert_eq!(first_word("Nova_Web"), "nova");
        assert_eq!(first_word("site.v2"), "site");
        assert_eq!(first_word("fourchette"), "fourchette");
        assert_eq!(first_word("-odd"), "-odd");
    }

    #[test]
    fn repositories_go_under_their_organisation_and_share_a_word() {
        let entries = [e("/d/fourchette"), e("/d/fourchette-site"), e("/d/Fourchette_board"), e("/d/api"), e("/d/web"), e("/d/notes"), e("/d/notes-old")];
        let mut info = HashMap::new();
        for p in ["/d/fourchette", "/d/fourchette-site", "/d/Fourchette_board", "/d/api"] {
            info.insert(PathBuf::from(p), RepoInfo { org: Some("C2-POLY".into()) });
        }
        // The same organisation typed in another case in a remote.
        info.insert(PathBuf::from("/d/web"), RepoInfo { org: Some("c2-poly".into()) });
        let s = sections(&entries, &info, &[], "");
        assert_eq!(titles(&s), [("C2-POLY", 5), ("Not on GitHub", 2)]);
        let org = &s[0].clusters;
        assert_eq!(org[0].proposal, Some(Proposal { name: "fourchette".into(), key: "C2-POLY/fourchette".into() }));
        assert_eq!(names(&org[0]), ["fourchette", "fourchette-site", "Fourchette_board"]);
        assert_eq!((org[1].proposal.clone(), names(&org[1])), (None, vec!["api".to_string(), "web".to_string()]));
        // Outside GitHub too, under the empty organisation's key.
        assert_eq!(s[1].clusters[0].proposal.as_ref().map(|p| p.key.as_str()), Some("/notes"));

        // Dismissed: the same repositories, without the proposal.
        let s = sections(&entries, &info, &["C2-POLY/fourchette".into()], "");
        assert_eq!(s[0].clusters.len(), 1);
        assert_eq!(s[0].clusters[0].proposal, None);
        assert_eq!(s[0].len(), 5);
    }

    #[test]
    fn groups_made_by_hand_win_over_organisations_and_proposals() {
        let mut a = e("/d/fourchette");
        let b = e("/d/fourchette-site");
        let mut c = e("/d/fourchette-old");
        a.group = Some("Work".into());
        c.group = Some("Work".into());
        let info: HashMap<PathBuf, RepoInfo> = [&a, &b, &c].iter().map(|x| (x.path.clone(), RepoInfo { org: Some("acme".into()) })).collect();
        let s = sections(&[a, b, c], &info, &[], "");
        assert_eq!(titles(&s), [("Work", 2), ("acme", 1)]);
        assert_eq!(s[0].kind, Kind::Group);
        // Neither the group nor the single one left in acme is proposed.
        assert!(s.iter().flat_map(|x| &x.clusters).all(|c| c.proposal.is_none()));
    }

    #[test]
    fn favourites_and_recent_come_first_and_stay_in_their_section() {
        let mut a = e("/d/a");
        let mut b = e("/d/b");
        let mut c = e("/d/c");
        a.favorite = true;
        (b.last_opened, c.last_opened) = (10, 20);
        a.last_opened = 30;
        let s = sections(&[a, b, c], &HashMap::new(), &[], "");
        assert_eq!(titles(&s), [("Favorites", 1), ("Recent", 2), ("Not on GitHub", 3)]);
        // The most recent first; the favourite is not repeated under Recent.
        assert_eq!(names(&s[1].clusters[0]), ["c", "b"]);
    }

    #[test]
    fn a_search_lists_matches_by_name_folder_or_organisation() {
        let mut fav = e("/work/nova-api");
        fav.favorite = true;
        let mut seen = e("/work/nova-web");
        seen.last_opened = 5;
        let entries = [e("/work/blog"), seen, fav];
        let info: HashMap<PathBuf, RepoInfo> = [(PathBuf::from("/work/blog"), RepoInfo { org: Some("NovaMoon".into()) })].into();
        let s = sections(&entries, &info, &[], "nova");
        assert_eq!(titles(&s), [("Matches", 3)]);
        assert_eq!(names(&s[0].clusters[0]), ["nova-api", "nova-web", "blog"]);
        assert_eq!(titles(&sections(&entries, &info, &[], "novamoon blog")), [("Matches", 1)]);
        assert!(sections(&entries, &info, &[], "nothing").is_empty());
    }

    #[test]
    fn the_sample_shows_every_kind_of_section() {
        let (entries, info) = sample();
        let s = sections(&entries, &info, &[], "");
        let kinds: Vec<Kind> = s.iter().map(|x| x.kind).collect();
        assert_eq!(kinds, [Kind::Favorites, Kind::Recent, Kind::Group, Kind::Org, Kind::Org, Kind::Org, Kind::Other]);
        let proposals: Vec<&str> = s.iter().flat_map(|x| &x.clusters).filter_map(|c| c.proposal.as_ref()).map(|p| p.name.as_str()).collect();
        assert_eq!(proposals, ["fourchette", "nova"]);
        assert!(entries.iter().all(|e| e.path.starts_with("/Users/me/dev")), "invented folders only");
    }
}
