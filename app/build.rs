//! On macOS, links `assets/Info.plist` into the binary so the menu bar says "Fourchette" (with
//! its capital) when the app runs straight from `target/`, outside an .app bundle.
//!
//! Also embeds the commit the app is built from as `FOURCHETTE_COMMIT` (empty outside a Git
//! checkout), so the update check can tell two builds of the same version apart.
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let plist = dir.join("assets/Info.plist");
    println!("cargo:rerun-if-changed={}", plist.display());
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-arg-bins=-Wl,-sectcreate,__TEXT,__info_plist,{}", plist.display());
    }

    let commit = git(dir, &["rev-parse", "HEAD"]).unwrap_or_default();
    println!("cargo:rustc-env=FOURCHETTE_COMMIT={commit}");
    // Build again when HEAD moves: a checkout changes HEAD, a commit changes the branch's ref
    // (a loose file, or packed-refs once packed). Only files that exist are watched, since
    // cargo reruns the script every time for a missing one.
    let git_dir = git(dir, &["rev-parse", "--absolute-git-dir"]).map(PathBuf::from);
    let common = git(dir, &["rev-parse", "--git-common-dir"]).map(|c| dir.join(c));
    let mut watched = Vec::new();
    if let Some(g) = &git_dir {
        watched.push(g.join("HEAD"));
    }
    if let Some(c) = &common {
        watched.push(c.join("packed-refs"));
        if let Some(r) = git(dir, &["symbolic-ref", "-q", "HEAD"]) {
            watched.push(c.join(r));
        }
    }
    for p in watched.into_iter().filter(|p| p.exists()) {
        println!("cargo:rerun-if-changed={}", p.display());
    }
}

/// A Git command's trimmed output, or None when Git is missing or fails.
fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git").current_dir(dir).args(args).output().ok()?;
    let s = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (out.status.success() && !s.is_empty()).then_some(s)
}
