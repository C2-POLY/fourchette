//! The person's repositories: where they are, how they're grouped, which are favourites.
//! Kept in `~/Library/Application Support/Fourchette/repos.json`.
use crate::settings::Settings;
use crate::theme::{Appearance, Variant};
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
    /// The app's colours and icon, chosen in Settings or the palette.
    pub variant: Variant,
    /// The repositories open in tabs, in order, brought back at launch.
    pub tabs: Vec<PathBuf>,
    /// The tab on screen, an index of `tabs`.
    pub active_tab: usize,
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

fn support_dir() -> Option<PathBuf> {
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
        let variant = legacy_setting(&v, "variant", Variant::parse).unwrap_or_default();
        // "tabs": {"open": [paths], "active": n}; files written before tabs (issue #59) have none.
        let tabs = v["tabs"]["open"].as_array().map(|a| a.iter().filter_map(|p| p.as_str()).map(PathBuf::from).collect()).unwrap_or_default();
        let active_tab = v["tabs"]["active"].as_u64().unwrap_or(0) as usize;
        Library { entries, appearance, variant, settings, tabs, active_tab, file: Some(file.to_path_buf()) }
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
        // The appearance and the variant sit under "settings" with the rest of the Settings window.
        let mut settings = self.settings.to_json();
        settings["appearance"] = json!(self.appearance.key());
        settings["variant"] = json!(self.variant.key());
        let tabs = json!({"open": self.tabs.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(), "active": self.active_tab});
        let _ = std::fs::write(file, serde_json::to_string_pretty(&json!({"version": 1, "settings": settings, "repos": repos, "tabs": tabs})).unwrap_or_default());
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

    /// Most recently opened first.
    pub fn recent(&self) -> Vec<&Entry> {
        let mut v: Vec<&Entry> = self.entries.iter().filter(|e| e.last_opened > 0).collect();
        v.sort_by(|a, b| b.last_opened.cmp(&a.last_opened));
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
    }

    #[test]
    fn variant_survives_a_save_and_defaults_to_classic() {
        let d = tmp("variant");
        let file = d.join("repos.json");
        assert_eq!(Library::read(&file).variant, Variant::Classic);
        let mut lib = Library::read(&file);
        lib.add(&d.join("a"));
        lib.appearance = Appearance::Dark;
        lib.variant = Variant::Neon;
        lib.save();
        let back = Library::read(&file);
        assert_eq!((back.variant, back.appearance), (Variant::Neon, Appearance::Dark));
        assert_eq!(back.entries.len(), 1);
        std::fs::write(&file, r#"{"version": 1, "variant": "sepia", "repos": []}"#).unwrap();
        assert_eq!(Library::read(&file).variant, Variant::Classic);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn an_old_file_moves_appearance_and_variant_under_settings_without_loss() {
        let d = tmp("migrate");
        let file = d.join("repos.json");
        // The layout before issue #52: appearance and variant at the top level.
        std::fs::write(
            &file,
            r#"{"version": 1, "appearance": "light", "variant": "neon", "settings": {"auto_fetch": false, "fetch_minutes": 30},
                "repos": [{"path": "/code/work/api", "group": "Work", "favorite": true, "last_opened": 7, "project": "acme#3", "details": "bottom", "details_hidden": true},
                          {"path": "/code/perso/blog"}]}"#,
        )
        .unwrap();
        let lib = Library::read(&file);
        let check = |lib: &Library| {
            assert_eq!((lib.appearance, lib.variant), (Appearance::Light, Variant::Neon));
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
                "variant": "neon",
                "ui_font": "inter",
                "code_font": "jetbrains",
                "auto_fetch": false,
                "fetch_minutes": 30,
                "pull_rebase": false,
                "pull_autostash": false,
                "delete_stash_after_apply": false,
                "diff_context": 3,
                "subject_low": 50,
                "subject_high": 70,
                "max_commits": 100_000,
                "claude_model": "default",
                "commit_language": "auto",
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
            (lib.appearance, lib.variant)
        };
        // Both places: the new one wins.
        assert_eq!(read(r#""appearance": "light", "variant": "forest", "settings": {"appearance": "dark", "variant": "gold"},"#), (Appearance::Dark, Variant::Gold));
        // Only the new place.
        assert_eq!(read(r#""settings": {"appearance": "light", "variant": "forest"},"#), (Appearance::Light, Variant::Forest));
        // An invalid value in the new place falls back to a valid one at the top level.
        assert_eq!(read(r#""appearance": "light", "variant": "forest", "settings": {"appearance": "sepia", "variant": 3},"#), (Appearance::Light, Variant::Forest));
        // Invalid or missing everywhere: the defaults.
        assert_eq!(read(r#""appearance": 1, "variant": "sepia", "settings": {"appearance": null},"#), (Appearance::System, Variant::Classic));
        assert_eq!(read(""), (Appearance::System, Variant::Classic));
        // "settings" of the wrong shape: the top level still counts, the other settings take their defaults.
        for bad in [r#""fast""#, "[1, 2]", "null"] {
            assert_eq!(read(&format!(r#""appearance": "dark", "variant": "gold", "settings": {bad},"#)), (Appearance::Dark, Variant::Gold), "{bad}");
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
}
