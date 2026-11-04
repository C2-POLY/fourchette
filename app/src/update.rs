//! Updates: the releases of C2-POLY/fourchette on GitHub, read and installed in the background.
//!
//! The check reads the public releases API with `curl` (no `gh`, no token: `gh` is optional for
//! people who use the app). A release is newer when its version is higher, or when it is the same
//! version built from another commit, since a day's version is published again after each merge.
//! The install downloads the release's .dmg, checks its SHA-256, copies the app out of it next to
//! the running one, then swaps the two by renaming, so a failure leaves the installed app as it was.
use serde_json::Value;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const REPO: &str = "C2-POLY/fourchette";
/// The version and commit this binary is built from (`build.rs`); the commit is empty outside Git.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const COMMIT: &str = env!("FOURCHETTE_COMMIT");
/// A check at launch, then every 6 hours.
pub const CHECK_EVERY: Duration = Duration::from_secs(6 * 3600);
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
    pub notes: String,
    /// The release's page, for a manual download when the install fails.
    pub page: String,
    pub dmg_url: String,
    pub dmg_size: u64,
    /// Lowercase hex of the .dmg's SHA-256, from the asset's `digest`; None when GitHub gave none.
    pub sha256: Option<String>,
    /// The commit the tag points at, read by `check`.
    pub commit: String,
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
                notes: s(r, "body"),
                page: s(r, "html_url"),
                dmg_url: s(dmg, "browser_download_url"),
                dmg_size: dmg.get("size").and_then(Value::as_u64).unwrap_or(0),
                sha256: dmg.get("digest").and_then(Value::as_str).and_then(parse_digest),
                commit: String::new(),
            })
        })
        .filter(|r| !r.dmg_url.is_empty())
        .collect()
}

/// The release with the highest version.
pub fn latest(v: &Value) -> Option<Release> {
    parse_releases(v).into_iter().max_by_key(|r| r.version)
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

/// The newest release when it is newer than this build, None when this build is up to date.
/// Runs on a background thread; an error (no network, rate limit) is meant to be ignored.
pub fn check() -> Result<Option<Release>, String> {
    let Some(mut r) = latest(&api("releases?per_page=20")?) else { return Ok(None) };
    r.commit = tag_commit(&r.tag)?;
    Ok(is_newer(r.version, &r.commit, Version::current(), COMMIT).then_some(r))
}

// ---------------------------------------------------------------- install

/// How far the install got, sent to the window as it goes.
#[derive(Clone, Debug, PartialEq)]
pub enum Progress {
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
    Release {
        tag: format!("v{}.{}.0", v.0, v.1 + 1),
        version: Version(v.0, v.1 + 1, 0),
        notes: "# Fourchette\n\nA Git client for macOS: history graph, branches, worktrees, stashes, undo for every local action, a Claude assistant and the GitHub project of the open repository.\n\n## What's new\n\n- Update Fourchette from the toolbar in one click.\n- **Settings**: choose the interface and code fonts.\n- Check out a branch by double-clicking its label in the graph.\n\n## Requirements\n\n- `git` (Xcode Command Line Tools).\n- `gh` (optional, for the GitHub project).\n- `claude` (optional, for the assistant).\n- Apple Silicon or Intel, macOS 11 or later.".into(),
        page: format!("https://github.com/{REPO}/releases"),
        dmg_url: String::new(),
        dmg_size: 12_143_632,
        sha256: None,
        commit: "7481beae6a3715b1e55570f72ca7ff9094c58205".into(),
    }
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

        let r = latest(&releases()).unwrap();
        assert_eq!(r.version, Version(0, 2, 0));
        assert_eq!(r.notes, "## New\n- Updates");
        assert_eq!(r.page, "https://github.com/C2-POLY/fourchette/releases/tag/v0.2.0");
        assert_eq!(r.dmg_url, "https://github.com/C2-POLY/fourchette/releases/download/v0.2.0/Fourchette-0.2.0.dmg");
        assert_eq!(r.dmg_size, 12143632);
        assert_eq!(r.sha256.as_deref(), Some("71f3daaf6110503ad2694bd3c7b03361819daffbae48e71ba1b7579c96c31245"));
        assert!(r.commit.is_empty());

        assert_eq!(latest(&json!([])), None);
        assert_eq!(latest(&json!({"message": "API rate limit exceeded"})), None);
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
            notes: String::new(),
            page: String::new(),
            dmg_url: format!("file://{}", dmg.display()),
            dmg_size: std::fs::metadata(&dmg).unwrap().len(),
            sha256: Some("0".repeat(64)),
            commit: String::new(),
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
