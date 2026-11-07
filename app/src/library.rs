//! The person's repositories: where they are, how they're grouped, which are favourites.
//! Kept in `~/Library/Application Support/Fourchette/repos.json`.
use crate::settings::Settings;
use crate::theme::Appearance;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// How many repositories the "Recent" list shows.
pub const RECENT_COUNT: usize = 8;
/// How deep "Scan a folder" looks for repositories.
pub const SCAN_DEPTH: usize = 3;
/// Folder names too common to name a group.
const GENERIC_FOLDERS: [&str; 12] = ["repo", "repos", "src", "code", "dev", "git", "github", "projects", "workspace", "sources", "documents", "developer"];

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub path: PathBuf,
    /// A group the person chose. Without one, the repository sits under its parent folder's name.
    pub group: Option<String>,
    pub favorite: bool,
    /// Unix seconds; 0 when it was added without being opened.
    pub last_opened: i64,
    /// The GitHub project picked for this repository, as "owner#number".
    pub project: Option<String>,
    /// Where the details panel sits for this repository; None until the person moves it.
    pub details: Option<DetailsLayout>,
}

/// Where the commit details panel sits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DetailsPlace {
    #[default]
    Right,
    Bottom,
}

impl DetailsPlace {
    pub const ALL: [DetailsPlace; 2] = [DetailsPlace::Right, DetailsPlace::Bottom];

    /// How it is written in repos.json.
    pub fn key(self) -> &'static str {
        match self {
            DetailsPlace::Right => "right",
            DetailsPlace::Bottom => "bottom",
        }
    }

    pub fn parse(s: &str) -> Option<DetailsPlace> {
        DetailsPlace::ALL.into_iter().find(|p| p.key() == s.trim().to_lowercase())
    }

    pub fn label(self) -> &'static str {
        match self {
            DetailsPlace::Right => "Right",
            DetailsPlace::Bottom => "Bottom",
        }
    }
}

/// The details panel's place, and whether it is hidden. A hidden panel keeps its place,
/// so ⌘J brings it back where it was.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DetailsLayout {
    pub place: DetailsPlace,
    pub hidden: bool,
}

impl DetailsLayout {
    pub fn shown(place: DetailsPlace) -> DetailsLayout {
        DetailsLayout { place, hidden: false }
    }

    /// ⌘J: hide it, or show it again at its last place.
    pub fn toggled(self) -> DetailsLayout {
        DetailsLayout { hidden: !self.hidden, ..self }
    }

    /// A click on the status bar button: from the right to the bottom and back, or shown
    /// again at its last place when it is hidden.
    pub fn clicked(self) -> DetailsLayout {
        if self.hidden {
            DetailsLayout::shown(self.place)
        } else {
            DetailsLayout::shown(match self.place {
                DetailsPlace::Right => DetailsPlace::Bottom,
                DetailsPlace::Bottom => DetailsPlace::Right,
            })
        }
    }

    pub fn hide(self) -> DetailsLayout {
        DetailsLayout { hidden: true, ..self }
    }

    /// "right", "bottom" or "hidden" (hidden on the right), as FOURCHETTE_DETAILS takes it.
    pub fn parse(s: &str) -> Option<DetailsLayout> {
        match s.trim().to_lowercase().as_str() {
            "hidden" => Some(DetailsLayout::default().hide()),
            p => DetailsPlace::parse(p).map(DetailsLayout::shown),
        }
    }

    pub fn label(self) -> &'static str {
        if self.hidden {
            "Hidden"
        } else {
            self.place.label()
        }
    }

    /// Read from a repos.json entry: "details" is the place, "details_hidden" the flag.
    /// A missing or unknown place means the right.
    fn from_json(e: &Value) -> Option<DetailsLayout> {
        let place = e["details"].as_str()?;
        Some(DetailsLayout { place: DetailsPlace::parse(place).unwrap_or_default(), hidden: e["details_hidden"].as_bool().unwrap_or(false) })
    }
}

impl Entry {
    pub fn name(&self) -> String {
        self.path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| self.path.display().to_string())
    }

    /// The chosen group, or the nearest parent folder whose name says something
    /// (".../c2-poly/repo/api" goes under "c2-poly", not "repo").
    pub fn group_name(&self) -> String {
        if let Some(g) = &self.group {
            return g.clone();
        }
        self.path
            .ancestors()
            .skip(1)
            .filter_map(|p| p.file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .find(|n| !GENERIC_FOLDERS.contains(&n.to_lowercase().as_str()))
            .unwrap_or_else(|| "Other".into())
    }
}

#[derive(Default)]
pub struct Library {
    pub entries: Vec<Entry>,
    /// System, dark or light, chosen in Settings, the palette or the status bar.
    pub appearance: Appearance,
    /// Everything else chosen in the Settings window.
    pub settings: Settings,
    /// The repositories open in tabs, in order, brought back at launch.
    pub tabs: Vec<PathBuf>,
    /// The tab on screen, an index of `tabs`.
    pub active_tab: usize,
    /// The home page's proposals the person dismissed (`home::proposal_key`).
    pub dismissed: Vec<String>,
    file: Option<PathBuf>,
}

/// The saved tabs whose folder `exists`, without repeats, and where the saved active tab
/// lands among them: on its repository, else on the tab before it, else on the first.
pub fn restore_tabs(tabs: &[PathBuf], active: usize, exists: impl Fn(&Path) -> bool) -> (Vec<PathBuf>, usize) {
    let mut out: Vec<PathBuf> = Vec::new();
    let mut at = None;
    for (k, p) in tabs.iter().enumerate() {
        let kept = exists(p);
        if kept && !out.contains(p) {
            out.push(p.clone());
        }
        if k == active {
            at = Some(if kept { out.iter().position(|o| o == p).unwrap_or(0) } else { out.len().saturating_sub(1) });
        }
    }
    let last = out.len().saturating_sub(1);
    (out, at.unwrap_or(last))
}

/// Whether a repository whose folder is not there counts as missing: yes, unless it sits on an
/// external disk that is not mounted (`/Volumes/<disk>/…` with `/Volumes/<disk>` itself gone),
/// which may come back when the disk is plugged in.
pub fn is_missing(path: &Path, exists: impl Fn(&Path) -> bool) -> bool {
    if exists(path) {
        return false;
    }
    let mut parts = path.components();
    let disk = match (parts.next(), parts.next(), parts.next()) {
        (Some(std::path::Component::RootDir), Some(std::path::Component::Normal(v)), Some(std::path::Component::Normal(d))) if v == "Volumes" => Some(Path::new("/Volumes").join(d)),
        _ => None,
    };
    disk.is_none_or(|d| exists(&d))
}

/// The temporary folders: a repository opened there (a demo made for a moment) is not kept in
/// the list nor in the tabs reopened at launch.
pub fn temp_roots() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = ["/tmp", "/private/tmp", "/var/folders", "/private/var/folders"].into_iter().map(PathBuf::from).collect();
    v.push(std::env::temp_dir());
    v
}

/// Whether `path` is inside one of the temporary folders `roots` (`temp_roots`).
pub fn is_temporary(path: &Path, roots: &[PathBuf]) -> bool {
    roots.iter().any(|r| !r.as_os_str().is_empty() && path.starts_with(r))
}

/// Entries taken off the list, each with its place in it, so that `Library::put_back` restores
/// them as they were.
pub type Removed = Vec<(usize, Entry)>;

pub(crate) fn support_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support/Fourchette"))
}

/// A choice kept under `"settings"`, or at the top level where files written before the Settings
/// sections (issue #52) kept it. The new place wins when both hold a valid value; an invalid value
/// in one place falls back to the other.
fn legacy_setting<T>(v: &Value, key: &str, parse: fn(&str) -> Option<T>) -> Option<T> {
    [&v["settings"][key], &v[key]].into_iter().find_map(|x| x.as_str().and_then(parse))
}

pub fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

impl Library {
    /// Reads repos.json, or the older recent.txt the first time.
    pub fn load() -> Library {
        let Some(dir) = support_dir() else { return Library::default() };
        let mut lib = Library::read(&dir.join("repos.json"));
        if lib.entries.is_empty() {
            if let Ok(s) = std::fs::read_to_string(dir.join("recent.txt")) {
                let t = now();
                for (k, l) in s.lines().filter(|l| !l.is_empty()).enumerate() {
                    lib.entries.push(Entry { path: PathBuf::from(l), group: None, favorite: false, last_opened: t - k as i64, project: None, details: None });
                }
            }
        }
        lib
    }

    pub fn read(file: &Path) -> Library {
        let v: Value = std::fs::read_to_string(file).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null);
        let entries = v["repos"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|e| {
                        Some(Entry {
                            path: PathBuf::from(e["path"].as_str()?),
                            group: e["group"].as_str().map(String::from),
                            favorite: e["favorite"].as_bool().unwrap_or(false),
                            last_opened: e["last_opened"].as_i64().unwrap_or(0),
                            project: e["project"].as_str().map(String::from),
                            details: DetailsLayout::from_json(e),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let appearance = legacy_setting(&v, "appearance", Appearance::parse).unwrap_or_default();
        let settings = Settings::from_json(&v["settings"]);
        // "tabs": {"open": [paths], "active": n}; files written before tabs (issue #59) have none.
        let tabs = v["tabs"]["open"].as_array().map(|a| a.iter().filter_map(|p| p.as_str()).map(PathBuf::from).collect()).unwrap_or_default();
        let active_tab = v["tabs"]["active"].as_u64().unwrap_or(0) as usize;
        // "home": {"dismissed": [keys]}; files written before the home page (issue #109) have none.
        let dismissed = v["home"]["dismissed"].as_array().map(|a| a.iter().filter_map(|k| k.as_str()).map(String::from).collect()).unwrap_or_default();
        Library { entries, appearance, settings, tabs, active_tab, dismissed, file: Some(file.to_path_buf()) }
    }

    /// The same settings with `entries` instead of the person's list, saved (if ever) to a
    /// temporary file rather than repos.json: screenshots show it.
    pub fn sample(&self, entries: Vec<Entry>) -> Library {
        Library { entries, appearance: self.appearance, settings: self.settings, tabs: Vec::new(), active_tab: 0, dismissed: Vec::new(), file: Some(std::env::temp_dir().join("fourchette-sample-repos.json")) }
    }

    pub fn save(&self) {
        let Some(file) = self.file.clone().or_else(|| support_dir().map(|d| d.join("repos.json"))) else { return };
        let repos: Vec<Value> = self
            .entries
            .iter()
            .map(|e| {
                let mut v = json!({"path": e.path.display().to_string(), "group": e.group, "favorite": e.favorite, "last_opened": e.last_opened, "project": e.project});
                if let Some(d) = e.details {
                    v["details"] = json!(d.place.key());
                    v["details_hidden"] = json!(d.hidden);
                }
                v
            })
            .collect();
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        // The appearance sits under "settings" with the rest of the Settings window. The "variant"
        // of older files (issue #125 removed the variants) is not written back.
        let mut settings = self.settings.to_json();
        settings["appearance"] = json!(self.appearance.key());
        let tabs = json!({"open": self.tabs.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(), "active": self.active_tab});
        let home = json!({"dismissed": self.dismissed});
        let _ = std::fs::write(file, serde_json::to_string_pretty(&json!({"version": 1, "settings": settings, "repos": repos, "tabs": tabs, "home": home})).unwrap_or_default());
    }

    pub fn get(&self, path: &Path) -> Option<&Entry> {
        self.entries.iter().find(|e| e.path == path)
    }

    fn get_mut(&mut self, path: &Path) -> Option<&mut Entry> {
        self.entries.iter_mut().find(|e| e.path == path)
    }

    /// Adds the repository if it's new; returns true when it was.
    pub fn add(&mut self, path: &Path) -> bool {
        if self.get(path).is_some() {
            return false;
        }
        self.entries.push(Entry { path: path.to_path_buf(), group: None, favorite: false, last_opened: 0, project: None, details: None });
        true
    }

    pub fn touch(&mut self, path: &Path) {
        self.add(path);
        if let Some(e) = self.get_mut(path) {
            e.last_opened = now();
        }
    }

    pub fn toggle_favorite(&mut self, path: &Path) {
        if let Some(e) = self.get_mut(path) {
            e.favorite = !e.favorite;
        }
    }

    pub fn set_group(&mut self, path: &Path, group: Option<String>) {
        if let Some(e) = self.get_mut(path) {
            e.group = group.map(|g| g.trim().to_string()).filter(|g| !g.is_empty());
        }
    }

    /// The home page's "Keep as a group": these repositories go to group `name`, as with
    /// "Move to group".
    pub fn keep_group(&mut self, paths: &[PathBuf], name: &str) {
        for p in paths {
            self.set_group(p, Some(name.to_string()));
        }
    }

    /// The home page's "Dismiss": that proposal is not made again.
    pub fn dismiss(&mut self, key: &str) {
        if !self.dismissed.iter().any(|k| k == key) {
            self.dismissed.push(key.to_string());
        }
    }

    pub fn set_project(&mut self, path: &Path, project: Option<String>) {
        if let Some(e) = self.get_mut(path) {
            e.project = project;
        }
    }

    /// The saved tabs that can still be opened, and the active one among them.
    pub fn restored_tabs(&self) -> (Vec<PathBuf>, usize) {
        restore_tabs(&self.tabs, self.active_tab, |p| p.exists())
    }

    /// The details panel layout of a repository: on the right until the person moves it.
    pub fn details(&self, path: &Path) -> DetailsLayout {
        self.get(path).and_then(|e| e.details).unwrap_or_default()
    }

    pub fn set_details(&mut self, path: &Path, details: DetailsLayout) {
        if let Some(e) = self.get_mut(path) {
            e.details = Some(details);
        }
    }

    pub fn forget(&mut self, path: &Path) {
        self.entries.retain(|e| e.path != path);
    }

    /// The entries whose folder is gone (`is_missing`): what "Remove N missing" takes off.
    pub fn missing(&self) -> Vec<PathBuf> {
        self.missing_with(|p| p.exists())
    }

    pub fn missing_with(&self, exists: impl Fn(&Path) -> bool) -> Vec<PathBuf> {
        self.entries.iter().filter(|e| is_missing(&e.path, &exists)).map(|e| e.path.clone()).collect()
    }

    /// Takes these repositories off the list (never anything on disk), and gives them back with
    /// their place for `put_back`.
    pub fn remove(&mut self, paths: &[PathBuf]) -> Removed {
        let mut removed = Vec::new();
        let mut k = 0;
        self.entries.retain(|e| {
            let gone = paths.contains(&e.path);
            if gone {
                removed.push((k, e.clone()));
            }
            k += 1;
            !gone
        });
        removed
    }

    /// Puts entries taken off by `remove` back where they were, with all they held. One added
    /// again since keeps its new entry.
    pub fn put_back(&mut self, removed: Removed) {
        for (k, e) in removed {
            if self.get(&e.path).is_none() {
                let at = k.min(self.entries.len());
                self.entries.insert(at, e);
            }
        }
    }

    /// Most recently opened first.
    pub fn recent(&self) -> Vec<&Entry> {
        let mut v: Vec<&Entry> = self.entries.iter().filter(|e| e.last_opened > 0).collect();
        v.sort_by_key(|e| std::cmp::Reverse(e.last_opened));
        v.truncate(RECENT_COUNT);
        v
    }

    /// Favourites in name order — ⌘1…⌘9 open them in this order.
    pub fn favorites(&self) -> Vec<&Entry> {
        let mut v: Vec<&Entry> = self.entries.iter().filter(|e| e.favorite).collect();
        v.sort_by_key(|e| e.name().to_lowercase());
        v
    }

    /// Every repository under its group, groups and repositories in name order.
    pub fn groups(&self) -> Vec<(String, Vec<&Entry>)> {
        let mut out: Vec<(String, Vec<&Entry>)> = Vec::new();
        for e in &self.entries {
            let g = e.group_name();
            match out.iter_mut().find(|x| x.0 == g) {
                Some(x) => x.1.push(e),
                None => out.push((g, vec![e])),
            }
        }
        out.sort_by_key(|x| x.0.to_lowercase());
        for x in &mut out {
            x.1.sort_by_key(|e| e.name().to_lowercase());
        }
        out
    }

    /// Groups the person named themselves (for "Move to group").
    pub fn custom_groups(&self) -> Vec<String> {
        let mut v: Vec<String> = self.entries.iter().filter_map(|e| e.group.clone()).collect();
        v.sort_by_key(|g| g.to_lowercase());
        v.dedup();
        v
    }
}

/// Finds Git repositories under `dir`, without going inside them or into hidden folders.
pub fn scan(dir: &Path, depth: usize) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if dir.join(".git").exists() {
        out.push(dir.to_path_buf());
        return out;
    }
    if depth == 0 {
        return out;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return out };
    let mut subs: Vec<PathBuf> = rd
        .flatten()
        .filter(|d| d.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter(|d| {
            let n = d.file_name().to_string_lossy().into_owned();
            !n.starts_with('.') && n != "node_modules" && n != "target"
        })
        .map(|d| d.path())
        .collect();
    subs.sort();
    for s in subs {
        out.extend(scan(&s, depth - 1));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("fourchette-lib-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn groups_default_to_parent_folder_and_survive_a_save() {
        let d = tmp("groups");
        let file = d.join("repos.json");
        let mut lib = Library::read(&file);
        lib.touch(Path::new("/code/work/api"));
        lib.add(Path::new("/code/work/web"));
        lib.add(Path::new("/code/perso/blog"));
        lib.set_group(Path::new("/code/perso/blog"), Some("  Side projects ".into()));
        lib.toggle_favorite(Path::new("/code/work/web"));
        lib.set_project(Path::new("/code/work/api"), Some("acme#3".into()));
        lib.save();

        let lib = Library::read(&file);
        let g = lib.groups();
        // "/code" is a generic folder, so "/code/work/api" goes under "work".
        assert_eq!(g.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), ["Side projects", "work"]);
        assert_eq!(g[1].1.iter().map(|e| e.name()).collect::<Vec<_>>(), ["api", "web"]);
        assert_eq!(lib.favorites().iter().map(|e| e.name()).collect::<Vec<_>>(), ["web"]);
        assert_eq!(lib.recent().iter().map(|e| e.name()).collect::<Vec<_>>(), ["api"]);
        assert_eq!(lib.custom_groups(), ["Side projects"]);
        assert_eq!(lib.get(Path::new("/code/work/api")).unwrap().project.as_deref(), Some("acme#3"));
        assert!(!Library::read(&file).add(Path::new("/code/work/api")), "no duplicates");
    }

    #[test]
    fn empty_group_name_goes_back_to_the_folder() {
        let mut lib = Library::default();
        lib.add(Path::new("/x/y/z"));
        lib.set_group(Path::new("/x/y/z"), Some("   ".into()));
        assert_eq!(lib.entries[0].group, None);
        assert_eq!(lib.entries[0].group_name(), "y");
    }

    #[test]
    fn generic_folders_are_skipped_for_the_group() {
        let e = |p: &str| Entry { path: PathBuf::from(p), group: None, favorite: false, last_opened: 0, project: None, details: None };
        assert_eq!(e("/Users/me/Documents/c2-poly/repo/api").group_name(), "c2-poly");
        assert_eq!(e("/Users/me/code/src/web").group_name(), "me");
        assert_eq!(e("/repo/x").group_name(), "Other");
    }

    #[test]
    fn scan_finds_repositories_but_not_inside_them() {
        let d = tmp("scan");
        for p in ["a/.git", "b/c/.git", "b/c/sub/.git", ".hidden/r/.git", "node_modules/m/.git", "e/f/g/h/.git"] {
            std::fs::create_dir_all(d.join(p)).unwrap();
        }
        let found: Vec<String> = scan(&d, SCAN_DEPTH).iter().map(|p| p.strip_prefix(&d).unwrap().display().to_string()).collect();
        assert_eq!(found, ["a", "b/c"]);
    }

    #[test]
    fn appearance_survives_a_save_and_defaults_to_system() {
        let d = tmp("appearance");
        let file = d.join("repos.json");
        assert_eq!(Library::read(&file).appearance, Appearance::System);
        let mut lib = Library::read(&file);
        lib.add(&d.join("a"));
        lib.appearance = Appearance::Light;
        lib.save();
        let back = Library::read(&file);
        assert_eq!(back.appearance, Appearance::Light);
        assert_eq!(back.entries.len(), 1);
        std::fs::write(&file, r#"{"version": 1, "appearance": "sepia", "repos": []}"#).unwrap();
        assert_eq!(Library::read(&file).appearance, Appearance::System);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn settings_survive_a_save_next_to_repos_and_appearance() {
        let d = tmp("settings");
        let file = d.join("repos.json");
        assert_eq!(Library::read(&file).settings, Settings::default());
        let mut lib = Library::read(&file);
        lib.add(&d.join("a"));
        lib.appearance = Appearance::Dark;
        lib.settings = Settings { auto_fetch: false, fetch_minutes: 30, ..Settings::default() };
        lib.save();
        let back = Library::read(&file);
        assert_eq!(back.settings, Settings { auto_fetch: false, fetch_minutes: 30, ..Settings::default() });
        assert_eq!(back.appearance, Appearance::Dark);
        assert_eq!(back.entries.len(), 1);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn an_older_or_damaged_file_keeps_its_repos_and_gets_default_settings() {
        let d = tmp("old-settings");
        let file = d.join("repos.json");
        // A file written before the Settings window: no "settings" key.
        std::fs::write(&file, r#"{"version": 1, "appearance": "light", "repos": [{"path": "/code/work/api", "favorite": true, "last_opened": 5}]}"#).unwrap();
        let lib = Library::read(&file);
        assert_eq!(lib.settings, Settings::default());
        assert_eq!(lib.appearance, Appearance::Light);
        assert_eq!(lib.favorites().iter().map(|e| e.name()).collect::<Vec<_>>(), ["api"]);
        // Saving it adds the settings and keeps the rest.
        lib.save();
        let back = Library::read(&file);
        assert_eq!((back.entries.len(), back.appearance), (1, Appearance::Light));
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(v["settings"]["fetch_minutes"], 10);
        // Settings of the wrong shape fall back without touching the repos.
        for bad in [r#""fast""#, "[1, 2]", r#"{"auto_fetch": 1, "fetch_minutes": 42}"#] {
            std::fs::write(&file, format!(r#"{{"version": 1, "appearance": "dark", "settings": {bad}, "repos": [{{"path": "/x/y"}}]}}"#)).unwrap();
            let lib = Library::read(&file);
            assert_eq!(lib.settings, Settings::default(), "{bad}");
            assert_eq!((lib.entries.len(), lib.appearance), (1, Appearance::Dark), "{bad}");
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn details_layout_is_kept_per_repository_and_defaults_to_the_right() {
        let d = tmp("details");
        let file = d.join("repos.json");
        let (a, b, c) = (d.join("a"), d.join("b"), d.join("c"));
        let mut lib = Library::read(&file);
        for p in [&a, &b, &c] {
            lib.add(p);
        }
        lib.set_details(&a, DetailsLayout::shown(DetailsPlace::Bottom));
        lib.set_details(&b, DetailsLayout::shown(DetailsPlace::Bottom).hide());
        lib.save();

        let back = Library::read(&file);
        assert_eq!(back.details(&a), DetailsLayout { place: DetailsPlace::Bottom, hidden: false });
        assert_eq!(back.details(&b), DetailsLayout { place: DetailsPlace::Bottom, hidden: true });
        assert_eq!(back.details(&c), DetailsLayout { place: DetailsPlace::Right, hidden: false }, "never set");
        assert_eq!(back.get(&c).unwrap().details, None, "nothing written for a repository never changed");
        assert_eq!(back.details(&d.join("unknown")), DetailsLayout::default());

        // An older repos.json without the fields, and an unknown place, still read.
        std::fs::write(
            &file,
            r#"{"version": 1, "repos": [{"path": "/x/old", "favorite": true}, {"path": "/x/odd", "details": "left", "details_hidden": "yes"}]}"#,
        )
        .unwrap();
        let old = Library::read(&file);
        assert_eq!(old.entries.len(), 2);
        assert!(old.entries[0].favorite);
        assert_eq!(old.details(Path::new("/x/old")), DetailsLayout::default());
        assert_eq!(old.details(Path::new("/x/odd")), DetailsLayout::default());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn details_layout_keys_and_toggle() {
        for p in DetailsPlace::ALL {
            assert_eq!(DetailsPlace::parse(p.key()), Some(p));
            assert_eq!(DetailsLayout::parse(p.key()), Some(DetailsLayout::shown(p)));
        }
        assert_eq!(DetailsPlace::parse(" Bottom "), Some(DetailsPlace::Bottom));
        assert_eq!(DetailsPlace::parse("left"), None);
        assert_eq!(DetailsLayout::parse("hidden"), Some(DetailsLayout { place: DetailsPlace::Right, hidden: true }));
        assert_eq!(DetailsLayout::parse("sideways"), None);
        // ⌘J hides the panel, then brings it back where it was.
        let bottom = DetailsLayout::shown(DetailsPlace::Bottom);
        assert!(bottom.toggled().hidden);
        assert_eq!(bottom.toggled().toggled(), bottom);
        assert_eq!(bottom.hide().label(), "Hidden");
        assert_eq!(bottom.label(), "Bottom");
        // A click on the status bar button moves it, or shows it again where it was.
        let right = DetailsLayout::shown(DetailsPlace::Right);
        assert_eq!(right.clicked(), bottom);
        assert_eq!(bottom.clicked(), right);
        assert_eq!(bottom.hide().clicked(), bottom);
        assert_eq!(right.hide().clicked(), right);
    }

    #[test]
    fn an_old_file_with_a_variant_loads_and_drops_it_on_save() {
        let d = tmp("variant");
        let file = d.join("repos.json");
        // Written while the variants existed (issues #14, #52): one at the top level, one under "settings".
        std::fs::write(
            &file,
            r#"{"version": 1, "variant": "gold", "settings": {"appearance": "dark", "variant": "neon", "ui_font": "sf", "auto_fetch": false},
                "repos": [{"path": "/code/a", "favorite": true}], "tabs": {"open": ["/code/a"], "active": 0}}"#,
        )
        .unwrap();
        let lib = Library::read(&file);
        let check = |lib: &Library| {
            assert_eq!(lib.appearance, Appearance::Dark);
            assert_eq!(lib.settings, Settings { ui_font: crate::settings::UiFont::SanFrancisco, auto_fetch: false, ..Settings::default() });
            assert_eq!(lib.entries.len(), 1);
            assert!(lib.entries[0].favorite);
            assert_eq!(lib.tabs, [PathBuf::from("/code/a")]);
        };
        check(&lib);
        lib.save();
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        assert!(v.get("variant").is_none() && v["settings"].get("variant").is_none(), "no variant left: {v}");
        check(&Library::read(&file));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn an_old_file_moves_appearance_under_settings_without_loss() {
        let d = tmp("migrate");
        let file = d.join("repos.json");
        // The layout before issue #52: appearance (and the variant, since removed) at the top level.
        std::fs::write(
            &file,
            r#"{"version": 1, "appearance": "light", "variant": "neon", "settings": {"auto_fetch": false, "fetch_minutes": 30},
                "repos": [{"path": "/code/work/api", "group": "Work", "favorite": true, "last_opened": 7, "project": "acme#3", "details": "bottom", "details_hidden": true},
                          {"path": "/code/perso/blog"}]}"#,
        )
        .unwrap();
        let lib = Library::read(&file);
        let check = |lib: &Library| {
            assert_eq!(lib.appearance, Appearance::Light);
            assert_eq!(lib.settings, Settings { auto_fetch: false, fetch_minutes: 30, ..Settings::default() });
            assert_eq!(lib.entries.len(), 2);
            let api = lib.get(Path::new("/code/work/api")).unwrap();
            assert_eq!((api.group.as_deref(), api.favorite, api.last_opened, api.project.as_deref()), (Some("Work"), true, 7, Some("acme#3")));
            assert_eq!(api.details, Some(DetailsLayout { place: DetailsPlace::Bottom, hidden: true }));
        };
        check(&lib);

        // Saving writes the new layout only, and reading it back loses nothing.
        lib.save();
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(
            v["settings"],
            json!({
                "appearance": "light",
                "ui_font": "inter",
                "code_font": "jetbrains",
                "auto_fetch": false,
                "fetch_minutes": 30,
                "pull_rebase": false,
                "pull_autostash": false,
                "delete_stash_after_apply": false,
                "issue_status_auto": false,
                "diff_context": 3,
                "subject_low": 50,
                "subject_high": 70,
                "max_commits": 100_000,
                "files_tree": false,
                "diff_split": false,
                "diff_whole": false,
                "claude_model": "default",
                "commit_language": "auto",
                "update_channel": "prerelease",
                "app_icon": "teal",
            })
        );
        assert!(v.get("appearance").is_none() && v.get("variant").is_none(), "no top-level copies: {v}");
        assert_eq!(v["repos"].as_array().map(Vec::len), Some(2));
        check(&Library::read(&file));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_settings_place_wins_and_invalid_values_fall_back() {
        let d = tmp("migrate-prefer");
        let file = d.join("repos.json");
        let read = |body: &str| {
            std::fs::write(&file, format!(r#"{{"version": 1, {body} "repos": [{{"path": "/x/y"}}]}}"#)).unwrap();
            let lib = Library::read(&file);
            assert_eq!(lib.entries.len(), 1, "{body}");
            lib.appearance
        };
        // Both places: the new one wins.
        assert_eq!(read(r#""appearance": "light", "settings": {"appearance": "dark"},"#), Appearance::Dark);
        // Only the new place.
        assert_eq!(read(r#""settings": {"appearance": "light"},"#), Appearance::Light);
        // An invalid value in the new place falls back to a valid one at the top level.
        assert_eq!(read(r#""appearance": "light", "settings": {"appearance": "sepia"},"#), Appearance::Light);
        // Invalid or missing everywhere: the default.
        assert_eq!(read(r#""appearance": 1, "settings": {"appearance": null},"#), Appearance::System);
        assert_eq!(read(""), Appearance::System);
        // "settings" of the wrong shape: the top level still counts, the other settings take their defaults.
        for bad in [r#""fast""#, "[1, 2]", "null"] {
            assert_eq!(read(&format!(r#""appearance": "dark", "settings": {bad},"#)), Appearance::Dark, "{bad}");
            assert_eq!(Library::read(&file).settings, Settings::default(), "{bad}");
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn open_tabs_survive_a_save_and_an_old_file_has_none() {
        let d = tmp("tabs");
        let file = d.join("repos.json");
        // A file written before tabs: no "tabs" key, everything else kept.
        std::fs::write(&file, r#"{"version": 1, "settings": {"appearance": "dark"}, "repos": [{"path": "/code/work/api", "favorite": true}]}"#).unwrap();
        let old = Library::read(&file);
        assert_eq!((old.tabs.len(), old.active_tab), (0, 0));
        assert_eq!((old.entries.len(), old.appearance), (1, Appearance::Dark));
        assert_eq!(old.restored_tabs(), (vec![], 0));

        let (a, b, c) = (d.join("a"), d.join("b"), d.join("c"));
        for p in [&a, &b, &c] {
            std::fs::create_dir_all(p).unwrap();
        }
        let mut lib = old;
        lib.tabs = vec![a.clone(), b.clone(), c.clone()];
        lib.active_tab = 2;
        lib.save();
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(v["tabs"]["open"].as_array().map(Vec::len), Some(3));
        assert_eq!(v["tabs"]["active"], 2);
        let back = Library::read(&file);
        assert_eq!((back.tabs.clone(), back.active_tab), (vec![a.clone(), b.clone(), c.clone()], 2));
        assert_eq!(back.entries.len(), 1, "the repos are untouched");
        assert_eq!(back.restored_tabs(), (vec![a.clone(), b.clone(), c.clone()], 2));

        // A folder gone since: skipped, and the active tab moves to the one before it.
        std::fs::remove_dir_all(&c).unwrap();
        assert_eq!(Library::read(&file).restored_tabs(), (vec![a, b], 1));

        // Tabs of the wrong shape read as none.
        for bad in [r#""a""#, "[1, 2]", r#"{"open": "x", "active": "y"}"#] {
            std::fs::write(&file, format!(r#"{{"version": 1, "tabs": {bad}, "repos": [{{"path": "/x/y"}}]}}"#)).unwrap();
            let lib = Library::read(&file);
            assert_eq!((lib.tabs.len(), lib.active_tab, lib.entries.len()), (0, 0, 1), "{bad}");
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn kept_groups_and_dismissed_proposals_survive_a_save() {
        let d = tmp("home");
        let file = d.join("repos.json");
        // A file written before the home page: nothing dismissed, the rest kept.
        std::fs::write(&file, r#"{"version": 1, "repos": [{"path": "/code/fourchette"}, {"path": "/code/fourchette-site", "favorite": true}]}"#).unwrap();
        let mut lib = Library::read(&file);
        assert!(lib.dismissed.is_empty());
        let paths = [PathBuf::from("/code/fourchette"), PathBuf::from("/code/fourchette-site")];
        lib.keep_group(&paths, "fourchette");
        lib.dismiss("acme/nova");
        lib.dismiss("acme/nova");
        lib.save();
        let back = Library::read(&file);
        assert_eq!(back.dismissed, ["acme/nova"]);
        assert_eq!(back.custom_groups(), ["fourchette"]);
        assert!(back.entries.iter().all(|e| e.group.as_deref() == Some("fourchette")));
        assert!(back.get(&paths[1]).unwrap().favorite, "the rest is untouched");
        // Of the wrong shape: nothing dismissed, the repositories still read.
        std::fs::write(&file, r#"{"version": 1, "home": {"dismissed": "x"}, "repos": [{"path": "/x/y"}]}"#).unwrap();
        let lib = Library::read(&file);
        assert_eq!((lib.dismissed.len(), lib.entries.len()), (0, 1));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn restored_tabs_skip_missing_folders_and_repeats() {
        let p = |s: &str| PathBuf::from(s);
        let tabs = [p("/a"), p("/gone"), p("/b"), p("/a"), p("/c")];
        let exists = |x: &Path| x != Path::new("/gone");
        assert_eq!(restore_tabs(&tabs, 0, exists), (vec![p("/a"), p("/b"), p("/c")], 0));
        // The active one is gone: the tab before it.
        assert_eq!(restore_tabs(&tabs, 1, exists).1, 0);
        assert_eq!(restore_tabs(&tabs, 2, exists).1, 1);
        // A repeat of /a was active: the tab of /a.
        assert_eq!(restore_tabs(&tabs, 3, exists).1, 0);
        assert_eq!(restore_tabs(&tabs, 4, exists).1, 2);
        // An index past the end keeps the last tab; nothing left gives nothing.
        assert_eq!(restore_tabs(&tabs, 99, exists).1, 2);
        assert_eq!(restore_tabs(&[p("/gone")], 0, exists), (vec![], 0));
    }

    #[test]
    fn missing_skips_present_folders_and_unmounted_disks() {
        let p = |s: &str| PathBuf::from(s);
        let there = [p("/Users/me/dev/here"), p("/Volumes/Work"), p("/Volumes/Work/here")];
        let exists = |x: &Path| there.iter().any(|t| t == x);
        assert!(!is_missing(&p("/Users/me/dev/here"), exists));
        assert!(is_missing(&p("/Users/me/dev/gone"), exists));
        // On a mounted disk: gone. On a disk that is not plugged in: kept.
        assert!(is_missing(&p("/Volumes/Work/gone"), exists));
        assert!(!is_missing(&p("/Volumes/Backup/repo"), exists));
        assert!(!is_missing(&p("/Volumes/Backup"), exists));
        // Only directly under /Volumes.
        assert!(is_missing(&p("/Users/me/Volumes/Backup/repo"), exists));
        assert!(is_missing(&p("/Volumes"), exists));

        let entry = |s: &str| Entry { path: p(s), group: None, favorite: false, last_opened: 0, project: None, details: None };
        let lib = Library { entries: ["/Users/me/dev/here", "/Users/me/dev/gone", "/Volumes/Backup/repo", "/Volumes/Work/gone"].into_iter().map(entry).collect(), ..Default::default() };
        assert_eq!(lib.missing_with(exists), [p("/Users/me/dev/gone"), p("/Volumes/Work/gone")]);
    }

    #[test]
    fn removed_entries_go_back_as_they_were() {
        let d = tmp("remove");
        let file = d.join("repos.json");
        let p = |s: &str| PathBuf::from(s);
        let entry = |s: &str| Entry { path: p(s), group: None, favorite: false, last_opened: 0, project: None, details: None };
        let mut lib = Library::read(&file);
        lib.entries = ["/a", "/b", "/c", "/d", "/e"].into_iter().map(entry).collect();
        lib.entries[1] = Entry { group: Some("Work".into()), favorite: true, last_opened: 42, project: Some("acme#3".into()), details: Some(DetailsLayout { place: DetailsPlace::Bottom, hidden: true }), ..entry("/b") };
        let before = lib.entries.clone();

        let removed = lib.remove(&[p("/b"), p("/d"), p("/nowhere")]);
        assert_eq!(removed.iter().map(|(k, _)| *k).collect::<Vec<_>>(), [1, 3]);
        assert_eq!(lib.entries.iter().map(|e| e.path.clone()).collect::<Vec<_>>(), [p("/a"), p("/c"), p("/e")]);
        lib.save();
        assert_eq!(Library::read(&file).entries.len(), 3, "saved without them");

        lib.put_back(removed.clone());
        assert_eq!(lib.entries, before, "same places, same favourite, group, project, last opened, layout");

        // One added again in the meantime keeps its new entry, and is not doubled.
        let mut lib2 = Library { entries: before.clone(), ..Default::default() };
        let removed = lib2.remove(&[p("/b"), p("/e")]);
        lib2.add(&p("/b"));
        lib2.put_back(removed);
        assert_eq!(lib2.entries.iter().map(|e| e.path.clone()).collect::<Vec<_>>(), [p("/a"), p("/c"), p("/d"), p("/b"), p("/e")]);
        assert!(!lib2.get(&p("/b")).unwrap().favorite);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn temporary_folders_are_recognised() {
        let roots = [PathBuf::from("/tmp"), PathBuf::from("/private/var/folders"), PathBuf::from("/var/folders/xy/T/"), PathBuf::new()];
        for t in ["/tmp/demo", "/tmp", "/private/var/folders/xy/T/demo", "/var/folders/xy/T/demo/sub"] {
            assert!(is_temporary(Path::new(t), &roots), "{t}");
        }
        for t in ["/tmpfiles/demo", "/Users/me/tmp/demo", "/var/foldersx/a", "/private/tmp2", "/"] {
            assert!(!is_temporary(Path::new(t), &roots), "{t}");
        }
        let defaults = temp_roots();
        for t in ["/private/tmp/claude-501/x", "/tmp/x", "/var/folders/ab/cd/T/x"] {
            assert!(is_temporary(Path::new(t), &defaults), "{t}");
        }
        assert!(is_temporary(&std::env::temp_dir().join("repo"), &defaults));
        assert!(!is_temporary(Path::new("/Users/me/dev/fourchette"), &defaults));
    }
}
