//! Updates: the releases of C2-POLY/fourchette on GitHub, read and installed in the background.
//!
//! The check reads the public releases API with `curl` (no `gh`, no token: `gh` is optional for
//! people who use the app), and offers the releases of the channel chosen in Settings. A release is newer when its version is higher, or when it is the same
//! version built from another commit, since a day's version is published again after each merge.
//! The install downloads the release's .dmg, checks its SHA-256, copies the app out of it next to
//! the running one, then swaps the two by renaming, so a failure leaves the installed app as it was.
//!
//! What's new (#137): each release's notes carry a "What's new" section generated from the commit
//! subjects (`scripts/release-notes.sh`). The check gathers it from every release at or above this
//! version and keeps the items whose pull request this build doesn't hold, since a day's version
//! is published again after each merge. After an install, those items are kept in
//! `whats-new.json` and shown once at the next launch.
use crate::settings::UpdateChannel;
use serde_json::Value;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const REPO: &str = "C2-POLY/fourchette";
/// The version and commit this binary is built from (`build.rs`); the commit is empty outside Git.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const COMMIT: &str = env!("FOURCHETTE_COMMIT");
/// The commit, or None when the build was made outside a Git checkout. A function, so a build
/// without Git still reads as one to the compiler and to Clippy.
pub fn commit() -> Option<&'static str> {
    Some(COMMIT).filter(|c| !c.is_empty())
}
/// The pull requests this build holds (`build.rs`): comma-separated numbers, empty outside Git.
pub const PRS: &str = env!("FOURCHETTE_PRS");
pub fn prs() -> Vec<u32> {
    PRS.split(',').filter_map(|n| n.trim().parse().ok()).collect()
}
/// A check at launch, then every hour.
pub const CHECK_EVERY: Duration = Duration::from_secs(3600);
/// The API calls give up after this; the download after `DOWNLOAD_TIMEOUT`.
const API_TIMEOUT: &str = "20";
const DOWNLOAD_TIMEOUT: &str = "900";

// macOS's own tools, by path: an app opened from the Finder has a short PATH.
const CURL: &str = "/usr/bin/curl";
const SHASUM: &str = "/usr/bin/shasum";
const HDIUTIL: &str = "/usr/bin/hdiutil";
const DITTO: &str = "/usr/bin/ditto";
const CODESIGN: &str = "/usr/bin/codesign";

/// `MAJOR.MINOR.PATCH`, compared field by field.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub u64, pub u64, pub u64);

impl Version {
    /// `0.2.0` or a tag `v0.2.0`; anything else (a suffix, a missing part) is not a version.
    pub fn parse(s: &str) -> Option<Version> {
        let s = s.trim();
        let s = s.strip_prefix('v').unwrap_or(s);
        let mut parts = s.split('.').map(|p| if !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()) { p.parse::<u64>().ok() } else { None });
        let v = Version(parts.next()??, parts.next()??, parts.next()??);
        parts.next().is_none().then_some(v)
    }

    /// This binary's version.
    pub fn current() -> Version {
        Version::parse(VERSION).unwrap_or(Version(0, 0, 0))
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

/// A published release with a .dmg of the app.
#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub tag: String,
    pub version: Version,
    /// Marked as a pre-release on GitHub: only offered on the Pre-release channel.
    pub prerelease: bool,
    pub notes: String,
    /// The release's page, for a manual download when the install fails.
    pub page: String,
    pub dmg_url: String,
    pub dmg_size: u64,
    /// Lowercase hex of the .dmg's SHA-256, from the asset's `digest`; None when GitHub gave none.
    pub sha256: Option<String>,
    /// The commit the tag points at, read by `check`.
    pub commit: String,
    /// What this build doesn't have yet, from the "What's new" sections of the releases at or
    /// above its version (`news_for`, filled by `check`). None when no release has the section:
    /// the dialog then shows the plain notes.
    pub news: Option<Vec<NewsItem>>,
}

/// The part of "What's new" an item sits in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Section {
    New,
    Fixed,
    Improved,
}

impl Section {
    pub const ALL: [Section; 3] = [Section::New, Section::Fixed, Section::Improved];

    /// The heading in the notes and the label in the dialog.
    pub fn label(self) -> &'static str {
        match self {
            Section::New => "New",
            Section::Fixed => "Fixed",
            Section::Improved => "Improved",
        }
    }

    fn parse(s: &str) -> Option<Section> {
        Section::ALL.into_iter().find(|x| x.label().eq_ignore_ascii_case(s.trim()))
    }
}

/// One line of "What's new": its text without the pull request, and that pull request's number.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewsItem {
    pub section: Section,
    pub text: String,
    pub pr: Option<u32>,
}

/// `Show a diff (#136)` → ("Show a diff", Some(136)); a text without `(#n)` at its end is kept whole.
pub fn split_pr(s: &str) -> (String, Option<u32>) {
    let s = s.trim();
    let found = s.strip_suffix(')').and_then(|r| r.rsplit_once("(#")).and_then(|(text, n)| Some((text.trim_end(), n.parse::<u32>().ok()?)));
    match found {
        Some((text, n)) => (text.to_string(), Some(n)),
        None => (s.to_string(), None),
    }
}

/// The items of a release body's "What's new" section (a `#` or `##` heading), under its `###`
/// headings New, Fixed and Improved (items before them, or under another heading, count as New).
/// The section ends at the next `#` or `##` heading. None when the body has no such section.
pub fn parse_news(md: &str) -> Option<Vec<NewsItem>> {
    let mut found = false;
    let mut inside = false;
    let mut section = Section::New;
    let mut items = Vec::new();
    for line in md.lines() {
        let t = line.trim();
        let level = t.bytes().take_while(|&b| b == b'#').count();
        if level > 0 && t[level..].starts_with(' ') {
            let title = t[level..].trim();
            if level <= 2 {
                inside = title.replace('\u{2019}', "'").eq_ignore_ascii_case("what's new");
                found |= inside;
                section = Section::New;
            } else if inside {
                section = Section::parse(title).unwrap_or(Section::New);
            }
            continue;
        }
        if !inside {
            continue;
        }
        if let Some(rest) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
            let (text, pr) = split_pr(&rest.replace("**", "").replace('`', ""));
            if !text.is_empty() {
                items.push(NewsItem { section, text, pr });
            }
        }
    }
    found.then_some(items)
}

/// What a build of `current` holding the pull requests `build_prs` doesn't have yet, from the
/// releases of its channel: the "What's new" items of every release at or above `current`
/// (versions skipped included), newest release first, each pull request once. An item is new
/// when its pull request isn't in the build and isn't older than all of the build's (the build
/// only lists its last 500 commits); an item without a number only when its release's version
/// is above the build's, since a same-version item may be in it already. A build without pull
/// requests (made outside Git) gets every item. None when no release has the section.
pub fn news_for(releases: &[Release], current: Version, build_prs: &[u32]) -> Option<Vec<NewsItem>> {
    let mut rs: Vec<&Release> = releases.iter().filter(|r| r.version >= current).collect();
    rs.sort_by_key(|r| std::cmp::Reverse(r.version));
    let oldest = build_prs.iter().min().copied();
    let mut found = false;
    let mut out: Vec<NewsItem> = Vec::new();
    for r in rs {
        let Some(items) = parse_news(&r.notes) else { continue };
        found = true;
        for it in items {
            let new = match (it.pr, oldest) {
                (_, None) => true,
                (Some(n), Some(low)) => n > low && !build_prs.contains(&n),
                (None, Some(_)) => r.version > current,
            };
            let seen = out.iter().any(|o| match (o.pr, it.pr) {
                (Some(a), Some(b)) => a == b,
                (None, None) => o.text == it.text,
                _ => false,
            });
            if new && !seen {
                out.push(it);
            }
        }
    }
    found.then_some(out)
}

/// The items of `news` in `section`, in their order.
pub fn in_section(news: &[NewsItem], section: Section) -> impl Iterator<Item = &NewsItem> {
    news.iter().filter(move |n| n.section == section)
}

// ---------------------------------------------------------------- what's new after an update

/// What an update brought, kept from the install to the next launch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shown {
    pub version: Version,
    pub items: Vec<NewsItem>,
}

/// `whats-new.json`, next to repos.json.
pub fn shown_file() -> Option<PathBuf> {
    crate::library::support_dir().map(|d| d.join("whats-new.json"))
}

/// Writes what the update brings, for the next launch to show.
pub fn save_shown(file: &Path, version: Version, items: &[NewsItem]) -> std::io::Result<()> {
    let items: Vec<Value> = items.iter().map(|i| serde_json::json!({"section": i.section.label(), "text": i.text, "pr": i.pr})).collect();
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(file, serde_json::to_string_pretty(&serde_json::json!({"version": version.to_string(), "items": items}))?)
}

/// Reads and removes the file `save_shown` wrote, so it shows once. None when there is none,
/// when it is unreadable or empty, or when it was written for another version than `current`
/// (the install didn't take, or another copy of the app was opened).
pub fn take_shown(file: &Path, current: Version) -> Option<Shown> {
    let s = std::fs::read_to_string(file).ok()?;
    let _ = std::fs::remove_file(file);
    let v: Value = serde_json::from_str(&s).ok()?;
    let version = Version::parse(v["version"].as_str()?)?;
    let items: Vec<NewsItem> = v["items"]
        .as_array()?
        .iter()
        .filter_map(|i| {
            Some(NewsItem {
                section: Section::parse(i["section"].as_str()?)?,
                text: i["text"].as_str()?.to_string(),
                pr: i["pr"].as_u64().and_then(|n| u32::try_from(n).ok()),
            })
        })
        .collect();
    (version == current && !items.is_empty()).then_some(Shown { version, items })
}

/// `"sha256:<64 hex>"` → the lowercase hex.
pub fn parse_digest(s: &str) -> Option<String> {
    let hex = s.trim().strip_prefix("sha256:")?;
    (hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit())).then(|| hex.to_ascii_lowercase())
}

/// The releases that can be installed, from `GET /repos/{repo}/releases`: published (pre-releases
/// included, drafts left out), tagged with a version and carrying a `Fourchette-*.dmg`.
pub fn parse_releases(v: &Value) -> Vec<Release> {
    let s = |x: &Value, k: &str| x.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
    v.as_array()
        .into_iter()
        .flatten()
        .filter(|r| !r.get("draft").and_then(Value::as_bool).unwrap_or(false))
        .filter_map(|r| {
            let tag = s(r, "tag_name");
            let version = Version::parse(&tag)?;
            let dmg = r.get("assets")?.as_array()?.iter().find(|a| {
                let n = s(a, "name");
                n.starts_with("Fourchette-") && n.ends_with(".dmg")
            })?;
            Some(Release {
                tag,
                version,
                prerelease: r.get("prerelease").and_then(Value::as_bool).unwrap_or(false),
                notes: s(r, "body"),
                page: s(r, "html_url"),
                dmg_url: s(dmg, "browser_download_url"),
                dmg_size: dmg.get("size").and_then(Value::as_u64).unwrap_or(0),
                sha256: dmg.get("digest").and_then(Value::as_str).and_then(parse_digest),
                commit: String::new(),
                news: None,
            })
        })
        .filter(|r| !r.dmg_url.is_empty())
        .collect()
}

/// The release with the highest version on `channel`: in Stable, pre-releases are left out even
/// when they are higher; in Pre-release, every kind counts. (`check` does the same on its list.)
#[cfg(test)]
pub fn latest(v: &Value, channel: UpdateChannel) -> Option<Release> {
    on_channel(v, channel).into_iter().max_by_key(|r| r.version)
}

/// The installable releases `channel` offers: in Stable, the full releases only.
pub fn on_channel(v: &Value, channel: UpdateChannel) -> Vec<Release> {
    parse_releases(v).into_iter().filter(|r| channel == UpdateChannel::Prerelease || !r.prerelease).collect()
}

/// What `GET /repos/{repo}/git/ref/tags/{tag}` (or `git/tags/{sha}`) points at: `(sha, type)`,
/// the type being `commit`, or `tag` for an annotated tag to read once more.
pub fn parse_ref_object(v: &Value) -> Option<(String, String)> {
    let o = v.get("object")?;
    let sha = o.get("sha")?.as_str()?.to_string();
    let kind = o.get("type")?.as_str()?.to_string();
    Some((sha, kind))
}

/// A release is newer than this build when its version is higher, or when it is the same
/// version built from another commit. Without the build's commit only the version counts.
pub fn is_newer(release: Version, release_commit: &str, current: Version, current_commit: &str) -> bool {
    if release != current {
        return release > current;
    }
    !current_commit.is_empty() && !release_commit.is_empty() && !release_commit.eq_ignore_ascii_case(current_commit)
}

/// The `.app` that holds `exe` (`…/X.app/Contents/MacOS/bin`); None for a binary outside a bundle.
pub fn bundle_of(exe: &Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let app = contents.parent()?;
    let is_app = app.extension().is_some_and(|e| e == "app");
    (macos.file_name()? == "MacOS" && contents.file_name()? == "Contents" && is_app).then(|| app.to_path_buf())
}

/// The .app this process runs from. None under `cargo run`: then the app never checks.
pub fn running_bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    bundle_of(&exe.canonicalize().unwrap_or(exe))
}

// ---------------------------------------------------------------- network

/// HTTPS only; the tests also read a local disk image through file://.
const PROTOCOLS: &str = if cfg!(test) { "=https,file" } else { "=https" };

/// curl with the options every call shares: no ~/.curlrc, fail on HTTP errors, follow redirects
/// (GitHub's downloads redirect to its storage), HTTPS only.
fn curl() -> Command {
    let mut c = Command::new(CURL);
    c.args(["-q", "-fsSL", "--proto", PROTOCOLS, "--proto-redir", "=https", "-A", &format!("Fourchette/{VERSION}")]);
    c.stdin(Stdio::null());
    c
}

fn api(path: &str) -> Result<Value, String> {
    let url = format!("https://api.github.com/repos/{REPO}/{path}");
    let out = curl()
        .args(["--max-time", API_TIMEOUT, "-H", "Accept: application/vnd.github+json", "-H", "X-GitHub-Api-Version: 2022-11-28", &url])
        .output()
        .map_err(|e| format!("Couldn't start curl: {e}"))?;
    if !out.status.success() {
        return Err(format!("curl {url}: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("{url}: {e}"))
}

/// The commit a tag points at, following an annotated tag to its commit.
fn tag_commit(tag: &str) -> Result<String, String> {
    let (mut sha, mut kind) = parse_ref_object(&api(&format!("git/ref/tags/{tag}"))?).ok_or("No tag object")?;
    for _ in 0..4 {
        if kind != "tag" {
            break;
        }
        (sha, kind) = parse_ref_object(&api(&format!("git/tags/{sha}"))?).ok_or("No tag object")?;
    }
    if kind != "commit" {
        return Err(format!("Tag {tag} points at a {kind}"));
    }
    Ok(sha)
}

/// The newest release of `channel` when it is newer than this build, None when this build is up
/// to date, with what it brings (`news_for`). Runs on a background thread; an error (no network,
/// rate limit) is shown in Settings.
pub fn check(channel: UpdateChannel) -> Result<Option<Release>, String> {
    let all = on_channel(&api("releases?per_page=20")?, channel);
    let Some(mut r) = all.iter().max_by_key(|r| r.version).cloned() else { return Ok(None) };
    r.commit = tag_commit(&r.tag)?;
    r.news = news_for(&all, Version::current(), &prs());
    Ok(is_newer(r.version, &r.commit, Version::current(), COMMIT).then_some(r))
}

// ---------------------------------------------------------------- install

/// How far the install got, sent to the window as it goes.
#[derive(Clone, Debug, PartialEq)]
pub enum Progress {
    /// Reading the release on GitHub again before downloading it.
    Checking,
    /// The release as GitHub gives it now: the one installed, shown in place of the one checked.
    Found(Box<Release>),
    /// GitHub has nothing newer any more (the release is gone, or this build is it): nothing done.
    UpToDate,
    /// The part of the .dmg downloaded, 0 to 1 (0 while the size is unknown).
    Downloading(f32),
    Installing,
    /// The new app is in place; the window relaunches it and quits.
    Done,
    Failed(String),
}

/// A folder removed when dropped.
struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fresh_temp_dir() -> Result<TempDir, String> {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let p = std::env::temp_dir().join(format!("fourchette-update-{}-{nanos}", std::process::id()));
    std::fs::create_dir(&p).map_err(|e| format!("Couldn't make a temporary folder: {e}"))?;
    Ok(TempDir(p))
}

/// Runs a tool and returns its output, or its error output as the message.
fn run(cmd: &mut Command, what: &str) -> Result<String, String> {
    let out = cmd.stdin(Stdio::null()).output().map_err(|e| format!("{what}: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        Err(format!("{what}: {}", err.trim()))
    }
}

/// Downloads `url` to `to`, reporting the part done against `size`.
fn download(url: &str, to: &Path, size: u64, progress: &dyn Fn(Progress)) -> Result<(), String> {
    progress(Progress::Downloading(0.0));
    let mut child = curl()
        .args(["--max-time", DOWNLOAD_TIMEOUT, "-o"])
        .arg(to)
        .arg(url)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Couldn't start curl: {e}"))?;
    let mut last = Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait().map_err(|e| e.to_string())? {
            break s;
        }
        std::thread::sleep(Duration::from_millis(100));
        if size > 0 && last.elapsed() >= Duration::from_millis(200) {
            last = Instant::now();
            let got = std::fs::metadata(to).map(|m| m.len()).unwrap_or(0);
            progress(Progress::Downloading((got as f32 / size as f32).min(1.0)));
        }
    };
    if !status.success() {
        let mut err = String::new();
        if let Some(mut e) = child.stderr.take() {
            let _ = e.read_to_string(&mut err);
        }
        return Err(format!("The download failed: {}", err.trim()));
    }
    progress(Progress::Downloading(1.0));
    Ok(())
}

/// The SHA-256 of a file, as lowercase hex.
fn sha256(path: &Path) -> Result<String, String> {
    let out = run(Command::new(SHASUM).args(["-a", "256"]).arg(path), "shasum")?;
    out.split_whitespace().next().map(str::to_ascii_lowercase).ok_or_else(|| "shasum gave no sum".into())
}

/// The app inside a mounted disk image: Fourchette.app, or else its only .app.
fn app_in(mount: &Path) -> Result<PathBuf, String> {
    let named = mount.join("Fourchette.app");
    if named.is_dir() {
        return Ok(named);
    }
    let apps: Vec<PathBuf> = std::fs::read_dir(mount)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "app") && p.is_dir())
        .collect();
    match apps.as_slice() {
        [one] => Ok(one.clone()),
        _ => Err("The disk image has no Fourchette.app".into()),
    }
}

/// Mounts `dmg` read-only on `mount`, copies its app to `to`, and always unmounts it.
fn copy_from_dmg(dmg: &Path, mount: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir(mount).map_err(|e| e.to_string())?;
    run(Command::new(HDIUTIL).args(["attach", "-nobrowse", "-readonly", "-noautoopen", "-mountpoint"]).arg(mount).arg(dmg), "Couldn't open the disk image")?;
    let copied = app_in(mount).and_then(|app| run(Command::new(DITTO).arg(&app).arg(to), "Couldn't copy the new app").map(|_| ()));
    if run(Command::new(HDIUTIL).arg("detach").arg(mount), "hdiutil detach").is_err() {
        let _ = run(Command::new(HDIUTIL).args(["detach", "-force"]).arg(mount), "hdiutil detach");
    }
    copied
}

/// "Update and Restart" (#163): reads the release on GitHub again, since its tag may have moved
/// and the release been published again since the check (a new .dmg, a new SHA-256), then
/// installs the release found then.
pub fn install_latest(channel: UpdateChannel, bundle: &Path, progress: &dyn Fn(Progress)) -> Result<(), String> {
    progress(Progress::Checking);
    let found = check(channel).map_err(|e| format!("Couldn't read the release on GitHub: {e}"))?;
    let Some(release) = found else {
        progress(Progress::UpToDate);
        return Ok(());
    };
    progress(Progress::Found(Box::new(release.clone())));
    install(&release, bundle, progress)
}

/// Replaces the app at `bundle` with `release`'s, reporting progress as it goes. On any error
/// the installed app is left as it was and the error says what went wrong.
pub fn install(release: &Release, bundle: &Path, progress: &dyn Fn(Progress)) -> Result<(), String> {
    let Some(want) = &release.sha256 else {
        return Err("GitHub gave no checksum for this download, so it can't be checked. Download it from the release page instead.".into());
    };
    if bundle.components().any(|c| c.as_os_str() == "AppTranslocation") {
        return Err("macOS runs Fourchette from a temporary copy. Move Fourchette.app to Applications, open it from there, then update.".into());
    }
    let dir = bundle.parent().ok_or("The app has no folder")?;
    let tmp = fresh_temp_dir()?;
    let dmg = tmp.0.join("Fourchette.dmg");
    download(&release.dmg_url, &dmg, release.dmg_size, progress)?;
    let got = sha256(&dmg)?;
    if &got != want {
        return Err(format!("The download is damaged: its SHA-256 is {got}, the release says {want}."));
    }

    progress(Progress::Installing);
    let name = bundle.file_name().ok_or("The app has no name")?.to_string_lossy().into_owned();
    let pid = std::process::id();
    // Hidden names next to the app, on its volume, so the swap is two renames.
    let staged = dir.join(format!(".{name}.update-{pid}"));
    let backup = dir.join(format!(".{name}.old-{pid}"));
    let _ = std::fs::remove_dir_all(&staged);
    let _ = std::fs::remove_dir_all(&backup);
    let staged_guard = TempDir(staged.clone());
    if let Err(e) = std::fs::create_dir(&staged).and_then(|_| std::fs::remove_dir(&staged)) {
        return Err(format!("Fourchette can't write to {}: {e}. Download the new version from the release page instead.", dir.display()));
    }
    copy_from_dmg(&dmg, &tmp.0.join("mount"), &staged)?;
    if !staged.join("Contents/MacOS").is_dir() {
        return Err("The new app is incomplete".into());
    }
    run(Command::new(CODESIGN).args(["--verify", "--deep", "--strict"]).arg(&staged), "The new app's signature is not valid")?;

    std::fs::rename(bundle, &backup).map_err(|e| format!("Couldn't move the current app aside: {e}"))?;
    if let Err(e) = std::fs::rename(&staged, bundle) {
        let restored = std::fs::rename(&backup, bundle);
        return Err(match restored {
            Ok(()) => format!("Couldn't put the new app in place: {e}"),
            Err(r) => format!("Couldn't put the new app in place ({e}) nor restore the old one ({r}); it is at {}", backup.display()),
        });
    }
    drop(staged_guard);
    let _ = std::fs::remove_dir_all(&backup);
    progress(Progress::Done);
    Ok(())
}

/// Opens `bundle` again once this process has quit (or after 20 seconds), from a detached shell.
pub fn relaunch_after_exit(bundle: &Path) -> std::io::Result<()> {
    use std::os::unix::process::CommandExt;
    const SCRIPT: &str = r#"i=0; while kill -0 "$1" 2>/dev/null && [ "$i" -lt 100 ]; do sleep 0.2; i=$((i+1)); done; exec /usr/bin/open -n "$2""#;
    Command::new("/bin/sh")
        .args(["-c", SCRIPT, "sh"])
        .arg(std::process::id().to_string())
        .arg(bundle)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map(|_| ())
}

/// A release to show the pill and the dialog in screenshots (`FOURCHETTE_UPDATE`).
pub fn sample() -> Release {
    let v = Version::current();
    let notes = "A Git client for macOS: history graph, branches, worktrees, stashes, undo for every local action, a Claude assistant and the GitHub project of the open repository.\n\n## What's new\n\n### New\n\n- Show a file's diff side by side or whole, with a ruler of where the changes are (#136)\n- Show the local changes as a tree of folders, with a List / Tree switch above the lists, and stage or unstage a whole folder from its row (#134)\n- Merge a local branch and its remote branch into one pill, with drawn icons (#132)\n\n### Fixed\n\n- Keep branch names readable next to the PR badge and tracking (#131)\n\n## Install\n\n1. Open `Fourchette-<version>.dmg`.\n2. Drag Fourchette to Applications.\n\n## Requirements\n\n- `git` (Xcode Command Line Tools).";
    Release {
        tag: format!("v{}.{}.0", v.0, v.1 + 1),
        version: Version(v.0, v.1 + 1, 0),
        prerelease: true,
        news: parse_news(notes),
        notes: notes.into(),
        page: format!("https://github.com/{REPO}/releases"),
        dmg_url: String::new(),
        dmg_size: 12_143_632,
        sha256: None,
        commit: "7481beae6a3715b1e55570f72ca7ff9094c58205".into(),
    }
}

/// What an update brought, for `FOURCHETTE_WHATS_NEW` screenshots.
pub fn sample_shown() -> Shown {
    Shown { version: Version::current(), items: sample().news.unwrap_or_default() }
}

/// Release notes as plain text: Markdown headings, bold and code marks taken out, list items
/// as bullets, runs of blank lines folded into one.
pub fn plain_notes(md: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for line in md.replace("\r\n", "\n").lines() {
        let t = line.trim_end();
        let t = t.trim_start_matches('#').trim_start_matches(' ');
        let t = t.replace("**", "").replace('`', "");
        let t = match t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
            Some(rest) => format!("•  {rest}"),
            None => t,
        };
        if t.is_empty() && out.last().is_none_or(|l| l.is_empty()) {
            continue;
        }
        out.push(t);
    }
    while out.last().is_some_and(|l| l.is_empty()) {
        out.pop();
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_and_compares_versions() {
        assert_eq!(Version::parse("v0.2.0"), Some(Version(0, 2, 0)));
        assert_eq!(Version::parse("1.10.3"), Some(Version(1, 10, 3)));
        assert_eq!(Version::parse(" v2.0.11 "), Some(Version(2, 0, 11)));
        for bad in ["", "v", "0.2", "0.2.0.1", "0.2.0-beta", "v0..1", "x1.2.3", "0.2.a", "+1.2.3"] {
            assert_eq!(Version::parse(bad), None, "{bad}");
        }
        assert!(Version(0, 10, 0) > Version(0, 9, 9));
        assert!(Version(1, 0, 0) > Version(0, 99, 99));
        assert!(Version(0, 2, 1) > Version(0, 2, 0));
        assert_eq!(Version(0, 2, 0).to_string(), "0.2.0");
        assert_eq!(Version::current().to_string(), VERSION);
    }

    #[test]
    fn reads_sha256_digests() {
        let hex = "71f3daaf6110503ad2694bd3c7b03361819daffbae48e71ba1b7579c96c31245";
        assert_eq!(parse_digest(&format!("sha256:{hex}")).as_deref(), Some(hex));
        assert_eq!(parse_digest(&format!("sha256:{}", hex.to_uppercase())).as_deref(), Some(hex));
        assert_eq!(parse_digest(hex), None);
        assert_eq!(parse_digest(&format!("sha512:{hex}")), None);
        assert_eq!(parse_digest("sha256:abc"), None);
        assert_eq!(parse_digest(&format!("sha256:{}zz", &hex[..62])), None);
    }

    fn dmg(version: &str, digest: Option<&str>) -> Value {
        let mut a = json!({
            "name": format!("Fourchette-{version}.dmg"),
            "size": 12143632,
            "browser_download_url": format!("https://github.com/C2-POLY/fourchette/releases/download/v{version}/Fourchette-{version}.dmg"),
        });
        if let Some(d) = digest {
            a["digest"] = json!(d);
        }
        a
    }

    fn releases() -> Value {
        let d = "sha256:71f3daaf6110503ad2694bd3c7b03361819daffbae48e71ba1b7579c96c31245";
        json!([
            // A draft with the highest version: never offered.
            {"tag_name": "v0.4.0", "draft": true, "prerelease": false, "body": "", "html_url": "", "assets": [dmg("0.4.0", Some(d))]},
            // No .dmg yet (the upload hasn't finished): left out.
            {"tag_name": "v0.3.0", "draft": false, "prerelease": true, "body": "", "html_url": "", "assets": [{"name": "notes.txt", "size": 3, "browser_download_url": "https://x/notes.txt"}]},
            // A tag that isn't a version: left out.
            {"tag_name": "nightly", "draft": false, "prerelease": true, "body": "", "html_url": "", "assets": [dmg("9.9.9", Some(d))]},
            // A pre-release: kept, and the newest.
            {"tag_name": "v0.2.0", "draft": false, "prerelease": true, "body": "## New\n- Updates", "html_url": "https://github.com/C2-POLY/fourchette/releases/tag/v0.2.0", "assets": [{"name": "Source.zip", "size": 1, "browser_download_url": "https://x/s.zip"}, dmg("0.2.0", Some(d))]},
            {"tag_name": "v0.1.0", "draft": false, "prerelease": false, "body": "", "html_url": "", "assets": [dmg("0.1.0", None)]},
        ])
    }

    #[test]
    fn reads_the_installable_releases() {
        let all = parse_releases(&releases());
        assert_eq!(all.iter().map(|r| r.tag.as_str()).collect::<Vec<_>>(), ["v0.2.0", "v0.1.0"]);
        assert_eq!(all[1].sha256, None);
        assert_eq!(all.iter().map(|r| r.prerelease).collect::<Vec<_>>(), [true, false]);

        let r = latest(&releases(), UpdateChannel::Prerelease).unwrap();
        assert_eq!(r.version, Version(0, 2, 0));
        assert_eq!(r.notes, "## New\n- Updates");
        assert_eq!(r.page, "https://github.com/C2-POLY/fourchette/releases/tag/v0.2.0");
        assert_eq!(r.dmg_url, "https://github.com/C2-POLY/fourchette/releases/download/v0.2.0/Fourchette-0.2.0.dmg");
        assert_eq!(r.dmg_size, 12143632);
        assert_eq!(r.sha256.as_deref(), Some("71f3daaf6110503ad2694bd3c7b03361819daffbae48e71ba1b7579c96c31245"));
        assert!(r.commit.is_empty());

        for channel in UpdateChannel::ALL {
            assert_eq!(latest(&json!([]), channel), None);
            assert_eq!(latest(&json!({"message": "API rate limit exceeded"}), channel), None);
        }
    }

    #[test]
    fn the_channel_picks_the_release() {
        // Pre-release: the highest version of any kind, the draft above it still left out.
        let pre = latest(&releases(), UpdateChannel::Prerelease).unwrap();
        assert_eq!((pre.tag.as_str(), pre.prerelease), ("v0.2.0", true));
        // Stable: v0.2.0 is a pre-release, so the full release v0.1.0 is offered though lower.
        let stable = latest(&releases(), UpdateChannel::Stable).unwrap();
        assert_eq!((stable.tag.as_str(), stable.prerelease), ("v0.1.0", false));
        // Only pre-releases: nothing for Stable. A missing flag reads as a full release.
        let only_pre = json!([{"tag_name": "v0.5.0", "prerelease": true, "assets": [dmg("0.5.0", None)]}]);
        assert_eq!(latest(&only_pre, UpdateChannel::Stable), None);
        assert_eq!(latest(&only_pre, UpdateChannel::Prerelease).map(|r| r.version), Some(Version(0, 5, 0)));
        let no_flag = json!([{"tag_name": "v0.5.0", "assets": [dmg("0.5.0", None)]}]);
        assert_eq!(latest(&no_flag, UpdateChannel::Stable).map(|r| r.prerelease), Some(false));
    }

    #[test]
    fn reads_tag_refs() {
        let lightweight = json!({"ref": "refs/tags/v0.1.0", "object": {"sha": "7481bea", "type": "commit"}});
        assert_eq!(parse_ref_object(&lightweight), Some(("7481bea".into(), "commit".into())));
        let annotated = json!({"tag": "v0.1.0", "object": {"sha": "abc", "type": "tag"}});
        assert_eq!(parse_ref_object(&annotated), Some(("abc".into(), "tag".into())));
        assert_eq!(parse_ref_object(&json!({"message": "Not Found"})), None);
    }

    #[test]
    fn newer_by_version_then_by_commit() {
        let (v1, v2) = (Version(0, 1, 0), Version(0, 2, 0));
        assert!(is_newer(v2, "aaa", v1, "bbb"));
        assert!(is_newer(v2, "aaa", v1, "aaa"));
        assert!(!is_newer(v1, "aaa", v2, "bbb"), "an older release, even from another commit");
        // The same version published again from another commit.
        assert!(is_newer(v1, "aaa", v1, "bbb"));
        assert!(!is_newer(v1, "aaa", v1, "aaa"));
        assert!(!is_newer(v1, "AAA", v1, "aaa"));
        // Without the build's commit (built outside Git), only the version counts.
        assert!(!is_newer(v1, "aaa", v1, ""));
        assert!(is_newer(v2, "aaa", v1, ""));
    }

    #[test]
    fn finds_the_enclosing_app_bundle() {
        let p = |s: &str| bundle_of(Path::new(s));
        assert_eq!(p("/Applications/Fourchette.app/Contents/MacOS/fourchette"), Some(PathBuf::from("/Applications/Fourchette.app")));
        assert_eq!(p("/Users/me/Apps/My Fourchette.app/Contents/MacOS/fourchette"), Some(PathBuf::from("/Users/me/Apps/My Fourchette.app")));
        // cargo run, or a binary copied out of the bundle.
        assert_eq!(p("/Users/me/dev/fourchette/app/target/debug/fourchette"), None);
        assert_eq!(p("/Applications/Fourchette.app/Contents/Resources/fourchette"), None);
        assert_eq!(p("/Applications/Fourchette/Contents/MacOS/fourchette"), None);
        assert_eq!(p("fourchette"), None);
        // The test binary itself isn't in a bundle.
        assert_eq!(running_bundle(), None);
    }

    #[test]
    fn release_notes_as_plain_text() {
        let md = "# Fourchette\r\n\r\n\r\nA **Git** client.\r\n\r\n## Requirements\r\n\r\n- `git` (Xcode).\r\n* `gh`\r\n\r\n";
        assert_eq!(plain_notes(md), "Fourchette\n\nA Git client.\n\nRequirements\n\n•  git (Xcode).\n•  gh");
        assert_eq!(plain_notes(""), "");
    }

    #[test]
    fn splits_the_pull_request_off() {
        assert_eq!(split_pr("Show a diff (#136)"), ("Show a diff".into(), Some(136)));
        assert_eq!(split_pr("  Show a diff   (#7) "), ("Show a diff".into(), Some(7)));
        assert_eq!(split_pr("Show a diff"), ("Show a diff".into(), None));
        assert_eq!(split_pr("Fix (#12) in the middle"), ("Fix (#12) in the middle".into(), None));
        assert_eq!(split_pr("Odd (#x)"), ("Odd (#x)".into(), None));
        assert_eq!(split_pr("Two (#3) (#4)"), ("Two (#3)".into(), Some(4)));
    }

    const NOTES: &str = "A Git client.\n\n## What's new\n\n### New\n\n- Show a **diff** side by side (#136)\n- Tree of `folders` (#134)\n\n### Fixed\n\n- Readable names (#131)\n- Pushed straight to main\n\n### Improved\n\n* Faster graph (#133)\n\n## Install\n\n- Open the dmg (#1)\n";

    fn item(section: Section, text: &str, pr: Option<u32>) -> NewsItem {
        NewsItem { section, text: text.into(), pr }
    }

    #[test]
    fn reads_the_whats_new_section() {
        let news = parse_news(NOTES).unwrap();
        assert_eq!(
            news,
            [
                item(Section::New, "Show a diff side by side", Some(136)),
                item(Section::New, "Tree of folders", Some(134)),
                item(Section::Fixed, "Readable names", Some(131)),
                item(Section::Fixed, "Pushed straight to main", None),
                item(Section::Improved, "Faster graph", Some(133)),
            ],
            "the Install list after it is left out"
        );
        assert_eq!(in_section(&news, Section::Fixed).count(), 2);
        // No section: None, so the dialog shows the plain notes.
        assert_eq!(parse_news("# Fourchette\n\n## Requirements\n\n- git (#2)"), None);
        assert_eq!(parse_news(""), None);
        // A section with nothing in it.
        assert_eq!(parse_news("## What's new\n\nSmall fixes and internal changes.\n\n## Install\n"), Some(vec![]));
        // A curly apostrophe, another case, a top-level heading, items before any ### and
        // under an unknown one count as New.
        let loose = "# What\u{2019}s New\n- First (#5)\n### Other\n- Second (#6)\n#### Fixed\n- Third (#7)\n#Not a heading\n- Fourth";
        let got = parse_news(loose).unwrap();
        assert_eq!(got.iter().map(|i| (i.section, i.pr)).collect::<Vec<_>>(), [(Section::New, Some(5)), (Section::New, Some(6)), (Section::Fixed, Some(7)), (Section::Fixed, None)]);
    }

    fn rel(version: Version, notes: &str) -> Release {
        Release { tag: format!("v{version}"), version, notes: notes.into(), ..sample() }
    }

    fn prs_of(news: &[NewsItem]) -> Vec<Option<u32>> {
        news.iter().map(|n| n.pr).collect()
    }

    #[test]
    fn news_is_what_the_build_lacks() {
        let v2 = Version(0, 2, 0);
        let today = rel(v2, NOTES);
        // A build of the same version with #131 and #134: #136 and #133 are new; the item
        // without a number may already be in it, so it is left out.
        let got = news_for(std::slice::from_ref(&today), v2, &[120, 131, 134]).unwrap();
        assert_eq!(prs_of(&got), [Some(136), Some(133)]);
        // Merged out of order: #133 merged after #134, so a build holding #134 still lacks it,
        // and a build with every one of them lacks nothing.
        assert_eq!(prs_of(&news_for(std::slice::from_ref(&today), v2, &[134, 131, 120]).unwrap()), [Some(136), Some(133)]);
        assert_eq!(news_for(std::slice::from_ref(&today), v2, &[136, 134, 133, 131]), Some(vec![]));
        // Older than every pull request the build lists (its last 500 commits): held.
        assert_eq!(prs_of(&news_for(std::slice::from_ref(&today), v2, &[135, 140]).unwrap()), [Some(136)]);
        // A build made outside Git gets everything.
        assert_eq!(news_for(std::slice::from_ref(&today), v2, &[]).unwrap().len(), 5);
    }

    #[test]
    fn news_gathers_skipped_versions() {
        let (v1, v2, v3) = (Version(0, 1, 0), Version(0, 2, 0), Version(0, 3, 0));
        let releases = [
            rel(v1, "## What's new\n\n### New\n\n- Old (#10)\n"),
            rel(v3, "## What's new\n\n### New\n\n- Third (#150)\n\n### Fixed\n\n- Hotfix on main\n- Also in 0.2.0 (#136)\n"),
            rel(v2, NOTES),
        ];
        // From 0.2.0 with #131 and #134: 0.3.0's items first (the item without a number is in a
        // higher version, so new), then 0.2.0's, #136 once; 0.1.0 is below the build.
        let got = news_for(&releases, v2, &[131, 134]).unwrap();
        assert_eq!(prs_of(&got), [Some(150), None, Some(136), Some(133)]);
        assert_eq!(got[1].text, "Hotfix on main");
        assert_eq!(got[2].section, Section::Fixed, "the newest release's wording wins");
        // A release without the section is skipped; none with it gives None.
        let old_style = [rel(v2, "# Fourchette\n\n## Requirements\n"), rel(v3, "## What's new\n\n- Third (#150)\n")];
        assert_eq!(prs_of(&news_for(&old_style, v2, &[131]).unwrap()), [Some(150)]);
        assert_eq!(news_for(&old_style[..1], v2, &[131]), None);
        assert_eq!(news_for(&[], v2, &[131]), None);
    }

    #[test]
    fn the_build_lists_its_pull_requests() {
        // build.rs reads the commits; under Git the list holds this branch's merged pull requests.
        if commit().is_some() {
            assert!(!prs().is_empty(), "{PRS}");
        }
        assert!(PRS.split(',').all(|n| n.is_empty() || n.parse::<u32>().is_ok()));
    }

    #[test]
    fn whats_new_after_an_update_shows_once() {
        let tmp = fresh_temp_dir().unwrap();
        let file = tmp.0.join("support/whats-new.json");
        let v = Version(0, 3, 0);
        let items = parse_news(NOTES).unwrap();
        save_shown(&file, v, &items).unwrap();
        assert_eq!(take_shown(&file, v), Some(Shown { version: v, items }));
        assert!(!file.exists(), "removed once read");
        assert_eq!(take_shown(&file, v), None);
        // Written for another version: dropped.
        save_shown(&file, v, &parse_news(NOTES).unwrap()).unwrap();
        assert_eq!(take_shown(&file, Version(0, 2, 0)), None);
        assert!(!file.exists());
        // Nothing to show, or a damaged file.
        save_shown(&file, v, &[]).unwrap();
        assert_eq!(take_shown(&file, v), None);
        std::fs::write(&file, "{").unwrap();
        assert_eq!(take_shown(&file, v), None);
        assert!(!file.exists());
    }

    #[test]
    fn the_sample_has_news() {
        let s = sample();
        assert_eq!(s.news.as_ref().map(Vec::len), Some(4));
        assert_eq!(sample_shown().items.len(), 4);
    }

    /// scripts/release-notes.sh on a repository made here: the app reads back what it writes.
    #[test]
    fn the_generated_notes_read_back() {
        let tmp = fresh_temp_dir().unwrap();
        let git = |args: &[&str]| {
            let out = Command::new("git").current_dir(&tmp.0).args(["-c", "user.name=T", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false", "-c", "tag.gpgsign=false"]).args(args).output().unwrap();
            assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        };
        git(&["init", "-q"]);
        git(&["commit", "-q", "--allow-empty", "-m", "feat(old): before the last version (#1)"]);
        git(&["tag", "v0.1.0"]);
        git(&["tag", "v0.3.0"]);
        for subject in [
            "feat(diff): show a diff side by side (#20)",
            "fix(sidebar): keep names readable (#21)",
            "perf(graph)!: lay out the graph faster (#22)",
            "refactor(theme): remove the variants (#23)",
            "feat(site): add a page (#24)",
            "chore: tidy (#25)",
            "docs(readme): explain (#26)",
            "Merge branch 'x'",
            "fix: straight on main",
        ] {
            git(&["commit", "-q", "--allow-empty", "-m", subject]);
        }
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/release-notes.sh");
        let out = Command::new(&script).current_dir(&tmp.0).args(["0.2.0", "HEAD"]).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let md = String::from_utf8(out.stdout).unwrap();
        assert!(md.starts_with("A Git client for macOS"), "{md}");
        assert!(md.contains("## Install") && md.contains("## Requirements"), "{md}");
        assert_eq!(
            parse_news(&md).unwrap(),
            [
                item(Section::New, "Show a diff side by side", Some(20)),
                item(Section::Fixed, "Straight on main", None),
                item(Section::Fixed, "Keep names readable", Some(21)),
                item(Section::Improved, "Lay out the graph faster", Some(22)),
            ],
            "{md}"
        );
        // Nothing worth telling: the section says so, and holds no item.
        git(&["tag", "v0.2.0"]);
        git(&["commit", "-q", "--allow-empty", "-m", "chore: tidy again (#27)"]);
        let out = Command::new(&script).current_dir(&tmp.0).args(["0.2.1"]).output().unwrap();
        let md = String::from_utf8(out.stdout).unwrap();
        assert!(md.contains("Small fixes and internal changes."), "{md}");
        assert_eq!(parse_news(&md), Some(vec![]));
    }

    /// The whole install against a disk image made here and read through a file:// URL: the
    /// app in place is replaced, a bad checksum leaves it untouched. Offline, but hdiutil takes
    /// about ten seconds, so it runs on its own: `cargo test installs_from -- --ignored`.
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "slow: makes and mounts a disk image (about 10 s)"]
    fn installs_from_a_local_disk_image() {
        let tmp = fresh_temp_dir().unwrap();
        let make_app = |at: &Path, marker: &str| {
            std::fs::create_dir_all(at.join("Contents/MacOS")).unwrap();
            std::fs::create_dir_all(at.join("Contents/Resources")).unwrap();
            std::fs::write(at.join("Contents/Info.plist"), "<?xml version=\"1.0\" encoding=\"UTF-8\"?><plist version=\"1.0\"><dict><key>CFBundleExecutable</key><string>fourchette</string><key>CFBundleIdentifier</key><string>com.example.fourchette-test</string></dict></plist>").unwrap();
            std::fs::copy("/usr/bin/true", at.join("Contents/MacOS/fourchette")).unwrap();
            std::fs::write(at.join("Contents/Resources/marker"), marker).unwrap();
            run(Command::new(CODESIGN).args(["--force", "--sign", "-"]).arg(at), "codesign").unwrap();
        };
        // The new app, in a disk image.
        let src = tmp.0.join("src");
        make_app(&src.join("Fourchette.app"), "new");
        let dmg = tmp.0.join("Fourchette-9.0.0.dmg");
        run(Command::new(HDIUTIL).args(["create", "-quiet", "-format", "UDZO", "-volname", "Fourchette test", "-srcfolder"]).arg(&src).arg(&dmg), "hdiutil create").unwrap();
        // The installed one.
        let installed = tmp.0.join("Applications/Fourchette.app");
        make_app(&installed, "old");

        let mut release = Release {
            tag: "v9.0.0".into(),
            version: Version(9, 0, 0),
            prerelease: false,
            notes: String::new(),
            page: String::new(),
            dmg_url: format!("file://{}", dmg.display()),
            dmg_size: std::fs::metadata(&dmg).unwrap().len(),
            sha256: Some("0".repeat(64)),
            commit: String::new(),
            news: None,
        };
        let marker = || std::fs::read_to_string(installed.join("Contents/Resources/marker")).unwrap();
        let siblings = || std::fs::read_dir(installed.parent().unwrap()).unwrap().count();

        let err = install(&release, &installed, &|_| {}).unwrap_err();
        assert!(err.contains("damaged"), "{err}");
        assert_eq!(marker(), "old");

        release.sha256 = None;
        assert!(install(&release, &installed, &|_| {}).unwrap_err().contains("checksum"));

        release.sha256 = Some(sha256(&dmg).unwrap());
        let seen = std::sync::Mutex::new(Vec::new());
        install(&release, &installed, &|p| seen.lock().unwrap().push(p)).unwrap();
        assert_eq!(marker(), "new");
        assert_eq!(siblings(), 1, "no staged copy or backup left next to the app");
        let seen = seen.into_inner().unwrap();
        assert_eq!(seen.first(), Some(&Progress::Downloading(0.0)));
        assert!(seen.contains(&Progress::Installing));
        assert_eq!(seen.last(), Some(&Progress::Done));
    }
}
