//! The GitHub project linked to the repository: loading it, its colours, and writing the moves
//! of its cards.
use super::*;
use crate::github::{self, Board};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

/// A move to write, with what takes it back if GitHub refuses it.
type Write = (github::Ids, github::Move, github::Undo);
/// GitHub's answer to a move.
type Written = (github::Undo, Result<(), String>);

#[derive(Default)]
pub struct ProjectState {
    /// Shared so the views can hold it while they change the rest of the app.
    pub board: Option<Arc<Board>>,
    pub error: Option<String>,
    pub loading: Option<Receiver<Result<Board, String>>>,
    /// The thread writing moves to GitHub one at a time, in order, and its answers.
    pub writes: Option<(Sender<Write>, Receiver<Written>)>,
}

/// The repository's pull requests, read in the background. A failed read (no `gh`, not signed
/// in, offline) keeps what was there and says nothing: the badges are a bonus.
#[derive(Default)]
pub struct PullsState {
    pub pulls: Option<Arc<crate::pulls::Pulls>>,
    pub loading: Option<Receiver<Result<crate::pulls::Pulls, String>>>,
}

/// The colour of a GitHub colour name ("BLUE"), as used by project options and issue types,
/// in the mode on screen.
pub(super) fn named_color(name: &str) -> Option<Color32> {
    Some(match name.to_ascii_uppercase().as_str() {
        "GRAY" => theme::c().dim,
        "BLUE" => theme::lane_color(0),
        "GREEN" => theme::c().added,
        "YELLOW" => theme::c().modified,
        "ORANGE" => theme::lane_color(3),
        "RED" => theme::c().deleted,
        "PINK" => theme::lane_color(4),
        "PURPLE" => theme::c().renamed,
        _ => return None,
    })
}

/// A status column's colour: the one the project gives it, else one guessed from its name.
pub(super) fn status_color(board: &Board, status: &str) -> Color32 {
    if let Some(c) = board.status_color(status).and_then(named_color) {
        return c;
    }
    let s = status.to_lowercase();
    if github::is_done(status) {
        theme::c().added
    } else if s.contains("review") {
        theme::c().renamed
    } else if s.contains("progress") || s.contains("doing") || s.contains("cours") {
        theme::c().accent_text
    } else if s.contains("ready") || s.contains("todo") || s.contains("to do") {
        theme::c().modified
    } else if s.contains("block") {
        theme::c().deleted
    } else {
        theme::c().dim
    }
}

impl App {
    /// Reads the project board of the active tab's repository, in the background.
    pub(super) fn project_load(&mut self) {
        if self.screenshot.is_some() {
            return;
        }
        let t = &mut self.tabs[self.active];
        t.pulls_load();
        let preferred = t.repo.as_ref().and_then(|r| self.library.get(&r.root)).and_then(|e| e.project.clone());
        t.project_load(preferred);
    }

    /// Remembers the chosen project for this repository and reads it.
    pub(super) fn project_pick(&mut self, i: usize) {
        let t = self.tab();
        let (Some(r), Some(b)) = (&t.repo, &t.project.board) else { return };
        let key = b.projects.get(i).map(|p| p.key());
        let root = r.root.clone();
        self.library.set_project(&root, key);
        self.library.save();
        self.project_load();
    }
}

impl Tab {
    /// Reads this repository's project board in the background, `preferred` if it is there.
    fn project_load(&mut self, preferred: Option<String>) {
        let Some(r) = &self.repo else { return };
        let url = r.default_remote().and_then(|rem| r.git(&["remote", "get-url", &rem]).ok()).unwrap_or_default();
        let Some(slug) = github::slug(&url) else {
            self.project = ProjectState::default();
            return;
        };
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(github::load(&slug, preferred.as_deref()));
        });
        self.project.loading = Some(rx);
    }

    /// Reads this repository's pull requests in the background, unless a read is under way.
    /// Outside GitHub there are none.
    pub(super) fn pulls_load(&mut self) {
        let Some(r) = &self.repo else { return };
        if self.pulls.loading.is_some() {
            return;
        }
        let (Some(slug), Some(remote)) = (self.github_slug(), r.default_remote()) else {
            self.pulls = PullsState::default();
            return;
        };
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(crate::pulls::load(&slug, &remote));
        });
        self.pulls.loading = Some(rx);
    }

    /// "owner/name" of the repository on GitHub, from its default remote's address.
    pub(super) fn github_slug(&self) -> Option<String> {
        let r = self.repo.as_ref()?;
        let remote = r.default_remote()?;
        github::slug(&r.git(&["remote", "get-url", &remote]).ok()?)
    }

    pub(super) fn pulls_poll(&mut self) {
        let Some(rx) = &self.pulls.loading else { return };
        match rx.try_recv() {
            Ok(Ok(p)) => {
                self.pulls.pulls = Some(Arc::new(p));
                self.pulls.loading = None;
            }
            Ok(Err(_)) | Err(std::sync::mpsc::TryRecvError::Disconnected) => self.pulls.loading = None,
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
    }

    /// The pull request a branch label shows: a local branch's, or a remote branch's on the
    /// remote the pull requests were read from. Tags have none.
    pub(super) fn pull_of(&self, r: &GitRef) -> Option<&crate::pulls::Pull> {
        let p = self.pulls.pulls.as_deref()?;
        match r.kind {
            RefKind::Local => p.for_branch(&r.name),
            RefKind::Remote => p.for_remote(&r.name),
            _ => None,
        }
    }

    /// The pull request of a history pill: its local branch's, else its remote branch's.
    pub(super) fn pill_pull(&self, pill: &crate::ui::RefPill) -> Option<&crate::pulls::Pull> {
        self.pull_of(pill.main).or_else(|| pill.remote.and_then(|r| self.pull_of(r)))
    }

    pub(super) fn project_poll(&mut self) {
        self.project_written();
        let Some(rx) = &self.project.loading else { return };
        let Ok(res) = rx.try_recv() else { return };
        self.project.loading = None;
        match res {
            Ok(b) => {
                self.project.board = Some(Arc::new(b));
                self.project.error = None;
            }
            Err(e) => self.project.error = Some(e),
        }
    }

    /// Moves a card at once, then writes the move to GitHub in the background. The sample
    /// project has no ids, so its moves stay on screen.
    pub(super) fn project_move(&mut self, m: github::Move) {
        let Some(b) = self.project.board.as_mut() else { return };
        let board = Arc::make_mut(b);
        let Some(undo) = board.apply(&m) else { return };
        if board.ids.project.is_empty() {
            return;
        }
        let ids = board.ids.clone();
        let (tx, _) = self.project.writes.get_or_insert_with(|| {
            let (tx, jobs) = channel::<Write>();
            let (done, rx) = channel();
            std::thread::spawn(move || {
                for (ids, m, undo) in jobs {
                    let _ = done.send((undo, github::write_move(&ids, &m)));
                }
            });
            (tx, rx)
        });
        let _ = tx.send((ids, m, undo));
    }

    /// A move GitHub refused goes back where it was, with the reason.
    fn project_written(&mut self) {
        let Some((_, rx)) = &self.project.writes else { return };
        let failed: Vec<(github::Undo, String)> = rx.try_iter().filter_map(|(u, r)| r.err().map(|e| (u, e))).collect();
        for (u, e) in failed {
            let Some(b) = self.project.board.as_mut() else { continue };
            let board = Arc::make_mut(b);
            let name = board.items.iter().find(|i| i.id == u.item).map(|i| i.number.map(|n| format!("#{n}")).unwrap_or_else(|| format!("“{}”", i.title))).unwrap_or_else(|| "the card".into());
            board.revert(&u);
            if self.issue.offer.as_ref().is_some_and(|o| o.item == u.item) {
                self.issue.offer = None;
            }
            self.notify(format!("Couldn't move {name}, so it went back: {e}"), true);
        }
    }

    /// Open tickets of this repository in the project: those not in a finished status.
    pub(super) fn project_open_count(&self) -> usize {
        let Some(b) = &self.project.board else { return 0 };
        b.items.iter().filter(|i| b.in_repo(i) && !github::is_done(&i.status)).count()
    }
}
