//! The GitHub project linked to the repository: loading it and its colours (read only).
use super::*;
use crate::github::{self, Board};
use std::sync::mpsc::Receiver;
use std::sync::Arc;

#[derive(Default)]
pub struct ProjectState {
    /// Shared so the views can hold it while they change the rest of the app.
    pub board: Option<Arc<Board>>,
    pub error: Option<String>,
    pub loading: Option<Receiver<Result<Board, String>>>,
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

    pub(super) fn project_poll(&mut self) {
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

    /// Open tickets of this repository in the project: those not in a finished status.
    pub(super) fn project_open_count(&self) -> usize {
        let Some(b) = &self.project.board else { return 0 };
        b.items.iter().filter(|i| b.in_repo(i) && !github::is_done(&i.status)).count()
    }
}
