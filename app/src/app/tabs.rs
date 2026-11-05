//! One tab per open repository (issue #59): opening, closing and switching tabs, keeping them
//! in the repository list for the next launch, and the strip under the toolbar that shows them
//! once two are open, like the native macOS tabs the original uses.
use super::*;
use crate::ui::fit_text;
use egui::{pos2, vec2, Align2, CornerRadius, Key, Modifiers, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui};
use std::path::Path;

/// Height of the strip under the toolbar.
pub(super) const STRIP_H: f32 = 34.0;
/// A tab's width while they all fit; they narrow down to `TAB_MIN_W` when they don't.
const TAB_W: f32 = 210.0;
const TAB_MIN_W: f32 = 64.0;
/// Space above the tabs, and on the strip's left.
const TAB_TOP: f32 = 5.0;
const STRIP_LEFT: f32 = 8.0;
const PLUS: f32 = 26.0;

/// A tab key typed anywhere in the window.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum TabKey {
    New,
    Close,
    Next,
    Previous,
}

impl TabKey {
    /// ⌘T, ⌘W, ⇧⌘] / ⌃Tab and ⇧⌘[ / ⌃⇧Tab. The menu bar takes ⌘T, ⌘W and ⇧⌘[ ] when its item
    /// is enabled, so these reach the window only when it is not, or when there is no menu bar.
    pub(super) fn of(key: Key, m: Modifiers) -> Option<TabKey> {
        let plain_cmd = m.command && !m.shift && !m.alt && !m.ctrl;
        let shift_cmd = m.command && m.shift && !m.alt && !m.ctrl;
        Some(match key {
            Key::Tab if m.ctrl && !m.command && !m.alt => {
                if m.shift {
                    TabKey::Previous
                } else {
                    TabKey::Next
                }
            }
            Key::T if plain_cmd => TabKey::New,
            Key::W if plain_cmd => TabKey::Close,
            // ⇧] comes as "}" on a US keyboard, or as "]" with Shift elsewhere.
            Key::CloseBracket if shift_cmd => TabKey::Next,
            Key::CloseCurlyBracket if m.command && !m.alt && !m.ctrl => TabKey::Next,
            Key::OpenBracket if shift_cmd => TabKey::Previous,
            Key::OpenCurlyBracket if m.command && !m.alt && !m.ctrl => TabKey::Previous,
            _ => return None,
        })
    }

    pub(super) fn cmd(self, active: usize) -> Cmd {
        match self {
            TabKey::New => Cmd::NewTab,
            TabKey::Close => Cmd::CloseTab(active),
            TabKey::Next => Cmd::ShowTab { next: true },
            TabKey::Previous => Cmd::ShowTab { next: false },
        }
    }
}

impl App {
    /// Shows tab `i`. A tab restored at launch reads its repository now, the first time it is
    /// shown; one whose folder is no longer a repository closes (or shows the sample project
    /// when it is the only tab).
    pub(super) fn activate(&mut self, i: usize) {
        let i = i.min(self.tabs.len() - 1);
        if i != self.active {
            // The palette lists the other tab's branches.
            self.palette = None;
        }
        self.active = i;
        while let Some(path) = self.tabs[self.active].pending.take() {
            match Repo::open(&path) {
                Ok(r) => {
                    // Its timers start now, as for a repository opened at launch.
                    let t = self.tab_mut();
                    t.last_status = Instant::now();
                    t.last_fetch = Instant::now();
                    self.set_repo(r);
                }
                Err(e) => {
                    self.notify(e, true);
                    if self.tabs.len() > 1 {
                        self.tabs.remove(self.active);
                        self.active = self.active.min(self.tabs.len() - 1);
                    } else {
                        self.tab_mut().load_demo();
                    }
                }
            }
        }
        self.save_tabs();
    }

    /// Opens a repository: in its tab when one already shows it, else in a new tab (`new_tab`)
    /// or in place of the active tab's repository. The sample project gives its tab up rather
    /// than staying next to the person's repositories.
    pub(super) fn open_path(&mut self, p: PathBuf, new_tab: bool) {
        let r = match Repo::open(&p) {
            Ok(r) => r,
            Err(e) => return self.notify(e, true),
        };
        if let Some(i) = self.tabs.iter().position(|t| t.root() == Some(r.root.as_path())) {
            return self.activate(i);
        }
        if new_tab && !(self.tab().is_demo() && self.tab().pending.is_none()) {
            self.tabs.push(Tab::new());
            self.palette = None;
            self.active = self.tabs.len() - 1;
        }
        let name = r.name();
        self.set_repo(r);
        self.notify(format!("Opened {name}"), false);
    }

    /// Closes tab `i`, never the last one. The tab on its right takes its place (on its left
    /// when it was the last). A tab whose Git job is running stays open until it ends.
    pub(super) fn close_tab(&mut self, i: usize) {
        if self.tabs.len() < 2 || i >= self.tabs.len() {
            return;
        }
        if let Some((label, _, false)) = &self.tabs[i].busy {
            let msg = format!("Wait for “{label}” to finish before closing {}.", self.tabs[i].name());
            return self.notify(msg, true);
        }
        self.tabs.remove(i);
        let next = if i < self.active {
            self.active - 1
        } else {
            if i == self.active {
                self.palette = None;
            }
            self.active.min(self.tabs.len() - 1)
        };
        self.active = next;
        self.activate(next);
    }

    /// The next tab, or the previous one, going round.
    pub(super) fn show_next_tab(&mut self, next: bool) {
        let n = self.tabs.len();
        self.activate(if next { (self.active + 1) % n } else { (self.active + n - 1) % n });
    }

    /// Keeps the open tabs in the repository list for the next launch (not while taking a
    /// screenshot); sample-project tabs are not kept. Returns true when something changed.
    pub(super) fn remember_tabs(&mut self) -> bool {
        if self.screenshot.is_some() {
            return false;
        }
        let open: Vec<PathBuf> = self.tabs.iter().filter_map(|t| t.root().map(Path::to_path_buf)).collect();
        let active = self.tab().root().and_then(|r| open.iter().position(|p| p == r)).unwrap_or(0);
        if self.library.tabs == open && self.library.active_tab == active {
            return false;
        }
        self.library.tabs = open;
        self.library.active_tab = active;
        true
    }

    pub(super) fn save_tabs(&mut self) {
        if self.remember_tabs() {
            self.library.save();
        }
    }

    /// At launch: the tabs open at the last quit, each read when first shown, then `explicit`
    /// (the repository given on the command line, or the current folder) in its tab or a new
    /// one. Returns false when no saved tab is left, to start as before tabs.
    pub(super) fn restore_tabs(&mut self, explicit: Option<PathBuf>) -> bool {
        let (saved, active) = self.library.restored_tabs();
        if saved.is_empty() {
            return false;
        }
        self.tabs = saved.into_iter().map(Tab::unread).collect();
        self.active = active;
        if let Some(p) = explicit {
            self.open_path(p, true);
        }
        self.activate(self.active);
        true
    }

    /// For `FOURCHETTE_TABS=<n>` screenshots: the sample project in `n` tabs, each with its own
    /// name and branch, the first one on screen.
    pub(super) fn sample_tabs(&mut self, n: usize) {
        const NAMES: [&str; 9] = ["sample-project", "fourchette", "pinball-fw", "api-gateway", "design-system", "docs-site", "infra", "mobile-app", "scripts"];
        const BRANCHES: [&str; 5] = ["develop", "main", "feature/search", "hotfix/login-timeout", "feature/graph-colours"];
        while self.tabs.len() < n.clamp(1, 20) {
            let mut t = Tab::new();
            t.load_demo();
            self.tabs.push(t);
        }
        for (k, t) in self.tabs.iter_mut().enumerate() {
            if t.repo.is_some() {
                continue;
            }
            t.sample_name = Some(NAMES[k % NAMES.len()].to_string());
            if k > 0 {
                let b = BRANCHES[k % BRANCHES.len()];
                t.head = t.refs.iter().find(|r| r.name == b).map(|r| r.target.clone()).unwrap_or_default();
                t.branch = Some(b.to_string());
            }
        }
    }

    // ---------------------------------------------------------------- the strip

    /// The tabs, left-aligned: a dot in the branch's colour, the repository, its branch, and a
    /// close button on the active one, then "+". The active tab takes the content's colour and
    /// joins it: the strip's bottom line stops under it.
    pub(super) fn tab_strip(&self, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
        let c = theme::c();
        let strip = ui.max_rect();
        let p = ui.painter().with_clip_rect(strip);
        let n = self.tabs.len();
        let room = strip.width() - STRIP_LEFT - PLUS - 24.0;
        let w = (room / n as f32).clamp(TAB_MIN_W, TAB_W).floor();
        let top = strip.top() + TAB_TOP;
        let bottom = strip.bottom();
        let rect_of = |k: usize| Rect::from_min_max(pos2(strip.left() + STRIP_LEFT + k as f32 * w, top), pos2(strip.left() + STRIP_LEFT + (k + 1) as f32 * w, bottom));
        let line = Stroke::new(1.0_f32, c.border);
        let radius = CornerRadius { nw: 7, ne: 7, sw: 0, se: 0 };

        // The bottom line, except under the active tab.
        let active = rect_of(self.active);
        let y = bottom - 0.5;
        p.hline(strip.left()..=active.left(), y, line);
        p.hline(active.right()..=strip.right(), y, line);

        for (k, t) in self.tabs.iter().enumerate() {
            let r = rect_of(k);
            let is_active = k == self.active;
            let resp = ui.interact(r, ui.id().with(("tab", t.key)), Sense::click());
            // Drawn a little past the bottom, so the outline has no bottom edge.
            let body = Rect::from_min_max(r.min, pos2(r.max.x, bottom + 8.0));
            if is_active {
                p.rect_filled(body, radius, c.bg);
                p.rect_stroke(body, radius, line, StrokeKind::Inside);
            } else {
                if resp.hovered() {
                    p.rect_filled(Rect::from_min_max(r.min + vec2(2.0, 2.0), r.max - vec2(2.0, 4.0)), CornerRadius::same(6), c.hover);
                }
                // A thin line between two background tabs.
                if k + 1 < n && k + 1 != self.active && !resp.hovered() {
                    p.vline(r.right(), (r.top() + 8.0)..=(r.bottom() - 8.0), line);
                }
            }

            let cy = r.center().y;
            let dot = if t.pending.is_some() { c.faint } else { t.ref_color(&t.head) };
            p.circle_filled(pos2(r.left() + 14.0, cy), 4.0, dot);
            let close_w = if is_active { 26.0 } else { 10.0 };
            let x = r.left() + 25.0;
            let max = (r.right() - close_w - x).max(0.0);
            let name = fit_text(&p, pos2(x, cy), Align2::LEFT_CENTER, &t.name(), theme::body(), if is_active { c.strong } else { c.text }, max);
            let branch = match (&t.repo, &t.pending, &t.branch) {
                (_, Some(_), _) => None,
                (_, None, Some(b)) => Some(b.clone()),
                (Some(_), None, None) => Some(format!("detached at {}", short(&t.head))),
                (None, None, None) => None,
            };
            let room = r.right() - close_w - (name.right() + 6.0);
            if let Some(b) = branch.filter(|_| room > 24.0) {
                fit_text(&p, pos2(name.right() + 6.0, cy + 0.5), Align2::LEFT_CENTER, &b, theme::small(), c.faint, room);
            }

            let mut on_close = false;
            if is_active && n > 1 {
                let x_rect = Rect::from_center_size(pos2(r.right() - 15.0, cy), vec2(18.0, 18.0));
                let x_resp = ui.interact(x_rect, ui.id().with(("tab_close", t.key)), Sense::click()).on_hover_text("Close tab (⌘W)");
                if x_resp.hovered() {
                    p.rect_filled(x_rect, CornerRadius::same(4), c.hover);
                }
                cross(&p, x_rect.center(), 3.5, if x_resp.hovered() { c.strong } else { c.dim });
                on_close = x_resp.hovered();
                if x_resp.clicked() {
                    cmds.push(Cmd::CloseTab(k));
                }
            }
            if !on_close {
                let tip = t.root().map(|p| p.display().to_string()).unwrap_or_else(|| "Sample project".into());
                let resp = resp.on_hover_text(tip);
                if resp.clicked() && !is_active {
                    cmds.push(Cmd::SelectTab(k));
                }
                if resp.middle_clicked() {
                    cmds.push(Cmd::CloseTab(k));
                }
            }
        }

        let after = rect_of(n - 1).right();
        let plus = Rect::from_center_size(pos2(after + 8.0 + PLUS / 2.0, top + (bottom - top) / 2.0), vec2(PLUS, PLUS));
        let resp = ui.interact(plus, ui.id().with("tab_new"), Sense::click()).on_hover_text("New tab (⌘T)");
        if resp.hovered() {
            p.rect_filled(plus, CornerRadius::same(6), c.hover);
        }
        let col = if resp.hovered() { c.strong } else { c.dim };
        let s = Stroke::new(1.5_f32, col);
        let m = plus.center();
        p.line_segment([m - vec2(6.0, 0.0), m + vec2(6.0, 0.0)], s);
        p.line_segment([m - vec2(0.0, 6.0), m + vec2(0.0, 6.0)], s);
        if resp.clicked() {
            cmds.push(Cmd::NewTab);
        }
    }
}

/// A small ×, drawn rather than typed.
fn cross(p: &egui::Painter, c: Pos2, half: f32, col: Color32) {
    let s = Stroke::new(1.4_f32, col);
    p.add(Shape::line_segment([c + vec2(-half, -half), c + vec2(half, half)], s));
    p.add(Shape::line_segment([c + vec2(-half, half), c + vec2(half, -half)], s));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("fourchette-tabs-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// A repository with one commit, at `dir/name`.
    fn repo(dir: &Path, name: &str) -> PathBuf {
        let p = dir.join(name);
        std::fs::create_dir_all(&p).unwrap();
        let g = |args: &[&str]| assert!(std::process::Command::new("git").args(args).current_dir(&p).output().unwrap().status.success(), "git {args:?}");
        g(&["init", "-q", "-b", "main"]);
        std::fs::write(p.join("a.txt"), "a").unwrap();
        g(&["add", "."]);
        g(&["-c", "user.name=T", "-c", "user.email=t@t", "commit", "-q", "-m", "first"]);
        // Repo::open gives the root as Git writes it (/private/var… rather than /var… on macOS).
        crate::git::find_root(&p).unwrap()
    }

    /// An app on the repository list in `dir`, with one blank tab, as at launch.
    fn app(dir: &Path) -> App {
        App::with_library(crate::library::Library::read(&dir.join("repos.json")), None)
    }

    fn roots(a: &App) -> Vec<PathBuf> {
        a.tabs.iter().map(|t| t.root().unwrap().to_path_buf()).collect()
    }

    fn toast(a: &App) -> Option<&str> {
        a.toast.as_ref().map(|t| t.msg.as_str())
    }

    #[test]
    fn tabs_open_close_and_switch() {
        let d = tmp("open");
        let (r1, r2, r3) = (repo(&d, "one"), repo(&d, "two"), repo(&d, "three"));
        let mut a = app(&d);
        a.open_path(r1.clone(), false);
        assert_eq!((a.tabs.len(), a.active), (1, 0));
        // ⌘T then a pick: a new tab at the end, on screen.
        a.open_path(r2.clone(), true);
        a.open_path(r3.clone(), true);
        assert_eq!(roots(&a), [r1.clone(), r2.clone(), r3.clone()]);
        assert_eq!(a.active, 2);
        assert_eq!(toast(&a), Some("Opened three"));
        // ⇧⌘] and ⇧⌘[ go round.
        a.show_next_tab(true);
        assert_eq!(a.active, 0);
        a.show_next_tab(false);
        assert_eq!(a.active, 2);
        a.activate(1);
        assert_eq!(a.tab().name(), "two");
        // ⌘O in a tab changes that tab's repository only.
        let r4 = repo(&d, "four");
        a.open_path(r4.clone(), false);
        assert_eq!(roots(&a), [r1.clone(), r4.clone(), r3.clone()]);
        // Closing the active tab shows the one on its right, or on its left at the end.
        a.close_tab(1);
        assert_eq!((roots(&a), a.active), (vec![r1.clone(), r3.clone()], 1));
        a.close_tab(1);
        assert_eq!((roots(&a), a.active), (vec![r1.clone()], 0));
        // A background tab closes without moving the view.
        a.open_path(r2.clone(), true);
        a.open_path(r3.clone(), true);
        a.activate(2);
        a.close_tab(0);
        assert_eq!((roots(&a), a.active), (vec![r2.clone(), r3.clone()], 1));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn opening_an_open_repository_goes_to_its_tab() {
        let d = tmp("already");
        let (r1, r2) = (repo(&d, "one"), repo(&d, "two"));
        let mut a = app(&d);
        a.open_path(r1.clone(), false);
        a.open_path(r2.clone(), true);
        a.tabs[1].summary = "Half-written".into();
        a.activate(0);
        // From ⌘T, ⌘O, a favourite or Open Recent, and from a folder inside it.
        a.open_path(r2.join("."), true);
        assert_eq!((a.tabs.len(), a.active), (2, 1));
        assert_eq!(a.tab().summary, "Half-written", "the tab is the same, not reread");
        a.activate(0);
        a.open_path(r2.clone(), false);
        assert_eq!((roots(&a), a.active), (vec![r1, r2], 1));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_last_tab_never_closes() {
        let d = tmp("last");
        let r1 = repo(&d, "one");
        let mut a = app(&d);
        a.open_path(r1.clone(), false);
        a.close_tab(0);
        a.run(vec![Cmd::CloseTab(0), Cmd::ShowTab { next: true }], &egui::Context::default());
        assert_eq!((roots(&a), a.active), (vec![r1], 0));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_tab_with_a_git_job_running_stays_open() {
        let d = tmp("busy");
        let (r1, r2) = (repo(&d, "one"), repo(&d, "two"));
        let mut a = app(&d);
        a.open_path(r1, false);
        a.open_path(r2, true);
        let (_tx, rx) = channel();
        a.tabs[0].busy = Some(("Push to 'origin'".into(), rx, false));
        a.close_tab(0);
        assert_eq!(a.tabs.len(), 2);
        assert_eq!(toast(&a), Some("Wait for “Push to 'origin'” to finish before closing one."));
        // A quiet auto-fetch does not hold it.
        let (_tx, rx) = channel();
        a.tabs[0].busy = Some(("Fetch 'origin'".into(), rx, true));
        a.close_tab(0);
        assert_eq!(a.tabs.len(), 1);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_sample_project_gives_its_tab_up() {
        let d = tmp("sample");
        let r1 = repo(&d, "one");
        let mut a = app(&d);
        a.tab_mut().load_demo();
        a.open_path(r1.clone(), true);
        assert_eq!((roots(&a), a.active), (vec![r1], 0));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn restored_tabs_are_read_only_when_first_shown() {
        let d = tmp("unread");
        let (r1, r2, r3) = (repo(&d, "one"), repo(&d, "two"), repo(&d, "three"));
        let mut a = app(&d);
        a.library.tabs = vec![r1.clone(), r2.clone(), r3.clone()];
        a.library.active_tab = 1;
        assert!(a.restore_tabs(None));
        assert_eq!((roots(&a), a.active), (vec![r1.clone(), r2.clone(), r3.clone()], 1));
        assert!(a.tabs[1].repo.is_some() && a.tabs[1].pending.is_none() && !a.tabs[1].commits.is_empty());
        for k in [0, 2] {
            let t = &a.tabs[k];
            assert!(t.repo.is_none() && t.pending.is_some() && t.commits.is_empty(), "tab {k} is not read");
        }
        assert_eq!(a.tabs[0].name(), "one");

        // Its timers and the refresh on focus leave it alone.
        let long_ago = Instant::now().checked_sub(std::time::Duration::from_secs(3600)).unwrap();
        let ctx = egui::Context::default();
        let settings = crate::settings::Settings { auto_fetch: true, fetch_minutes: 1, ..Default::default() };
        for t in &mut a.tabs {
            t.last_status = long_ago;
            t.last_fetch = long_ago;
        }
        tab::poll_tabs(&mut a.tabs, &ctx, settings, true);
        a.tabs[0].refresh_status();
        assert!(a.tabs[0].busy.is_none() && a.tabs[0].last_status == long_ago && a.tabs[0].last_fetch == long_ago);
        assert!(a.tabs[0].status.is_empty() && a.tabs[0].repo.is_none());
        // The one on screen refreshed its status (it has no remote to fetch from).
        assert!(a.tabs[1].last_status > long_ago);

        // Shown: read, with its timers starting now.
        a.run(vec![Cmd::SelectTab(0)], &ctx);
        assert_eq!(a.active, 0);
        let t = &a.tabs[0];
        assert!(t.repo.is_some() && t.pending.is_none() && !t.commits.is_empty());
        assert!(t.last_fetch > long_ago && t.last_status > long_ago);
        assert!(a.tabs[2].pending.is_some(), "the third is still not read");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_restored_tab_that_is_no_longer_a_repository_closes() {
        let d = tmp("gone");
        let (r1, r2) = (repo(&d, "one"), repo(&d, "two"));
        let mut a = app(&d);
        a.library.tabs = vec![r1.clone(), r2.clone()];
        a.library.active_tab = 0;
        assert!(a.restore_tabs(None));
        std::fs::remove_dir_all(r2.join(".git")).unwrap();
        a.show_next_tab(true);
        assert_eq!((roots(&a), a.active), (vec![r1], 0));
        assert!(toast(&a).is_some_and(|m| m.contains("not a Git repository")));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn tabs_survive_a_relaunch_with_the_active_one() {
        let d = tmp("relaunch");
        let (r1, r2, r3) = (repo(&d, "one"), repo(&d, "two"), repo(&d, "three"));
        let mut a = app(&d);
        a.open_path(r1.clone(), false);
        a.open_path(r2.clone(), true);
        a.open_path(r3.clone(), true);
        a.activate(1);
        let back = crate::library::Library::read(&d.join("repos.json"));
        assert_eq!((back.tabs, back.active_tab), (vec![r1.clone(), r2.clone(), r3.clone()], 1));

        // The next launch, with the current folder a repository already in a tab: its tab.
        let mut b = app(&d);
        assert!(b.restore_tabs(Some(r3.clone())));
        assert_eq!((roots(&b), b.active), (vec![r1.clone(), r2.clone(), r3.clone()], 2));
        assert!(b.tabs[1].pending.is_some(), "the saved active tab was not needed, so not read");
        // One not in a tab yet: a new tab.
        let r4 = repo(&d, "four");
        let mut c = app(&d);
        assert!(c.restore_tabs(Some(r4.clone())));
        assert_eq!((roots(&c), c.active), (vec![r1, r2, r3, r4], 3));

        // Nothing saved: launch as before tabs.
        let mut e = app(&tmp("relaunch-empty"));
        assert!(!e.restore_tabs(None));
        assert_eq!(e.tabs.len(), 1);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_background_tabs_message_names_its_repository() {
        let d = tmp("notice");
        let (r1, r2) = (repo(&d, "one"), repo(&d, "two"));
        let mut a = app(&d);
        a.open_path(r1, false);
        a.open_path(r2, true);
        a.tabs[0].notify("Pushed main to origin", false);
        a.take_notices();
        assert_eq!(toast(&a), Some("one: Pushed main to origin"));
        a.tabs[1].notify("Fetched", false);
        a.take_notices();
        assert_eq!(toast(&a), Some("Fetched"), "the tab on screen needs no name");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn each_tab_has_its_own_egui_key() {
        let a = Tab::new();
        let b = Tab::new();
        assert_ne!(a.key, b.key);
    }

    #[test]
    fn tab_keys() {
        let m = |command: bool, shift: bool, ctrl: bool| Modifiers { command, mac_cmd: command, shift, ctrl, alt: false };
        assert_eq!(TabKey::of(Key::Tab, m(false, false, true)), Some(TabKey::Next));
        assert_eq!(TabKey::of(Key::Tab, m(false, true, true)), Some(TabKey::Previous));
        assert_eq!(TabKey::of(Key::Tab, m(false, false, false)), None, "Tab alone moves the focus");
        assert_eq!(TabKey::of(Key::T, m(true, false, false)), Some(TabKey::New));
        assert_eq!(TabKey::of(Key::T, m(true, true, false)), None, "⇧⌘T is New Tag");
        assert_eq!(TabKey::of(Key::T, m(true, false, true)), None, "⌃⌘T is Open in Terminal");
        assert_eq!(TabKey::of(Key::W, m(true, false, false)), Some(TabKey::Close));
        assert_eq!(TabKey::of(Key::CloseBracket, m(true, true, false)), Some(TabKey::Next));
        assert_eq!(TabKey::of(Key::CloseCurlyBracket, m(true, true, false)), Some(TabKey::Next));
        assert_eq!(TabKey::of(Key::OpenBracket, m(true, true, false)), Some(TabKey::Previous));
        assert_eq!(TabKey::of(Key::OpenCurlyBracket, m(true, true, false)), Some(TabKey::Previous));
        assert_eq!(TabKey::of(Key::CloseBracket, m(true, false, false)), None);
        assert!(matches!(TabKey::Close.cmd(3), Cmd::CloseTab(3)));
    }

    #[test]
    fn the_palette_command_sets_the_saved_setting_the_tabs_follow() {
        let d = tmp("commit-limit");
        let r1 = repo(&d, "one");
        let mut a = app(&d);
        a.open_path(r1, false);
        let ctx = egui::Context::default();
        a.run(vec![Cmd::SetCommitLimit(5_000)], &ctx);
        assert_eq!(a.library.settings.max_commits, 5_000);
        assert_eq!(toast(&a), Some("maxCommitCount set to 5000"));
        tab::poll_tabs(&mut a.tabs, &ctx, a.library.settings, false);
        assert_eq!(a.tabs[0].commit_limit, 5_000);
        // Kept for the next launch.
        assert_eq!(crate::library::Library::read(&d.join("repos.json")).settings.max_commits, 5_000);
        let _ = std::fs::remove_dir_all(&d);
    }
}
