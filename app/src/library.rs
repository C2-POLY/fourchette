//! The person's repositories: where they are, how they're grouped, which are favourites.
//! Kept in `~/Library/Application Support/Fourchette/repos.json`.
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
    file: Option<PathBuf>,
}

fn support_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support/Fourchette"))
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
                    lib.entries.push(Entry { path: PathBuf::from(l), group: None, favorite: false, last_opened: t - k as i64, project: None });
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
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Library { entries, file: Some(file.to_path_buf()) }
    }

    pub fn save(&self) {
        let Some(file) = self.file.clone().or_else(|| support_dir().map(|d| d.join("repos.json"))) else { return };
        let repos: Vec<Value> = self
            .entries
            .iter()
            .map(|e| json!({"path": e.path.display().to_string(), "group": e.group, "favorite": e.favorite, "last_opened": e.last_opened, "project": e.project}))
            .collect();
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(file, serde_json::to_string_pretty(&json!({"version": 1, "repos": repos})).unwrap_or_default());
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
        self.entries.push(Entry { path: path.to_path_buf(), group: None, favorite: false, last_opened: 0, project: None });
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
        let e = |p: &str| Entry { path: PathBuf::from(p), group: None, favorite: false, last_opened: 0, project: None };
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
}
