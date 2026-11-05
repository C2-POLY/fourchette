//! The pull request window: Claude drafts the title and description of the current branch's pull
//! request in the background (see crate::pr_draft); the person edits them, copies them, or creates
//! the pull request with `gh`, only on that click. The branch must be on GitHub first: the window
//! says so and never pushes.
use super::*;
use crate::modal::{self, pill, text, Hint, Modal, Width};
use crate::pr_draft::{self, Issue};
use egui::RichText;

pub enum PrDraftMsg {
    /// Claude's title and body, with what was read: the commits, the files changed and the issue.
    Drafted { commits: usize, files: String, issue: Option<Result<Issue, String>>, res: Result<(String, String), String> },
    /// The pull request's address on GitHub, or why it wasn't created.
    Created(Result<String, String>),
}

#[derive(Default)]
pub struct PrDraftView {
    pub branch: String,
    pub base: String,
    /// The branch on GitHub (the upstream's name); None while it hasn't been pushed.
    pub head: Option<String>,
    /// Commits not pushed yet.
    pub ahead: usize,
    /// "owner/name" of the upstream's remote, or of the default remote.
    pub slug: Option<String>,
    /// The issue number the branch name carries.
    pub number: Option<u64>,
    /// The issue as GitHub gave it, or why it couldn't.
    pub issue: Option<Result<Issue, String>>,
    pub commits: usize,
    /// The last line of `git diff --stat` ("2 files changed, …").
    pub files: String,
    pub title: String,
    pub body: String,
    pub drafted: bool,
    pub error: Option<String>,
    /// The pull request once created.
    pub url: Option<String>,
    pub job: Option<Receiver<PrDraftMsg>>,
}

impl PrDraftView {
    pub fn loading(&self) -> bool {
        self.job.is_some()
    }

    /// Why Create can't run, if it can't.
    fn blocked(&self) -> Option<String> {
        if self.slug.is_none() {
            return Some("This repository isn't on GitHub, so there is no pull request to create. Copy the text instead.".into());
        }
        if self.head.is_none() {
            return Some(format!("{} isn't on GitHub yet: push it first (Push… ⇧⌘P), then create the pull request.", self.branch));
        }
        None
    }

    fn can_create(&self) -> bool {
        self.drafted && !self.loading() && self.url.is_none() && !self.title.trim().is_empty() && self.blocked().is_none()
    }
}

impl Tab {
    /// Opens the window on the current branch and has Claude draft its pull request.
    pub(super) fn pr_draft_open(&mut self, ctx: &egui::Context) {
        let Some(r) = &self.repo else {
            self.notify("This is the sample project — open one of your repositories to draft a pull request.", true);
            return;
        };
        let Some(branch) = r.current_branch() else {
            self.notify("Check out a branch first: a pull request starts from a branch.", true);
            return;
        };
        let base = crate::ai::base_branch(r).unwrap_or_default();
        if base.is_empty() || base == branch {
            self.notify(format!("{branch} is the base branch: check out the branch you want to merge into it."), true);
            return;
        }
        let up = r.upstream();
        let remote = up.as_ref().map(|(rem, _)| rem.clone()).or_else(|| r.default_remote());
        let slug = remote.and_then(|rem| r.git(&["remote", "get-url", &rem]).ok()).and_then(|u| crate::github::slug(&u));
        let ahead = if up.is_some() { r.ahead_behind().map(|(a, _)| a).unwrap_or(0) } else { 0 };
        self.pr_draft = Some(PrDraftView { number: pr_draft::issue_number(&branch), branch, base, head: up.map(|(_, b)| b), ahead, slug, ..Default::default() });
        self.pr_draft_write(ctx);
    }

    /// Reads the branch and its issue, then asks Claude, in the background.
    fn pr_draft_write(&mut self, ctx: &egui::Context) {
        let model = self.claude_model;
        let Some(root) = self.repo.as_ref().map(|r| r.root.clone()) else { return };
        let Some(view) = self.pr_draft.as_mut() else { return };
        if view.loading() {
            return;
        }
        view.error = None;
        let (branch, base, slug, number, known) = (view.branch.clone(), view.base.clone(), view.slug.clone(), view.number, view.issue.clone());
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let b = pr_draft::read_branch(&Repo { root }, &branch, &base);
            let files = b.stat.lines().last().unwrap_or("").trim().to_string();
            let issue = known.or_else(|| {
                let n = number?;
                Some(slug.as_deref().ok_or_else(|| "This repository isn't on GitHub.".to_string()).and_then(|s| pr_draft::read_issue(s, n)))
            });
            let res = if b.commits.is_empty() && b.diff.trim().is_empty() {
                Err(format!("{branch} has nothing that {base} doesn't have: there is nothing to describe."))
            } else {
                let i = issue.as_ref().and_then(|i| i.as_ref().ok());
                crate::ai::ask(&pr_draft::prompt(&b, i), crate::ai::SYSTEM, Some(&pr_draft::schema()), model).and_then(|v| pr_draft::read_draft(&v, i.map(|i| i.number)))
            };
            let _ = tx.send(PrDraftMsg::Drafted { commits: b.commits.len() + b.more, files, issue, res });
            ctx.request_repaint();
        });
        view.job = Some(rx);
    }

    pub(super) fn pr_draft_poll(&mut self) {
        let Some(view) = self.pr_draft.as_mut() else { return };
        let Some(msg) = view.job.as_ref().and_then(|rx| rx.try_recv().ok()) else { return };
        view.job = None;
        match msg {
            PrDraftMsg::Drafted { commits, files, issue, res } => {
                (view.commits, view.files, view.issue) = (commits, files, issue);
                match res {
                    Ok((title, body)) => {
                        (view.title, view.body, view.drafted) = (title, body, true);
                    }
                    Err(e) => view.error = Some(e),
                }
            }
            PrDraftMsg::Created(Ok(url)) => {
                view.url = Some(url);
                self.notify("Created the pull request on GitHub", false);
            }
            PrDraftMsg::Created(Err(e)) => view.error = Some(e),
        }
    }

    /// Runs `gh pr create` with the text as it is in the window, in the background.
    fn pr_draft_create(&mut self, ctx: &egui::Context) {
        let Some(view) = self.pr_draft.as_mut() else { return };
        let (Some(slug), Some(head)) = (view.slug.clone(), view.head.clone()) else { return };
        if !view.can_create() {
            return;
        }
        view.error = None;
        let (base, title, body) = (view.base.clone(), view.title.trim().to_string(), view.body.trim().to_string());
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(PrDraftMsg::Created(pr_draft::create(&slug, &base, &head, &title, &body)));
            ctx.request_repaint();
        });
        view.job = Some(rx);
    }

    /// The pull request window.
    pub(super) fn pr_draft_window(&mut self, ctx: &egui::Context) {
        let Some(view) = self.pr_draft.as_mut() else { return };
        let k = theme::c();
        let loading = view.loading();
        let mut frame = Modal::new("pr_draft", Width::Large, "ai", vec![text("Pull request")])
            .status(vec![pill(&view.branch), text("into"), pill(&view.base)])
            .cancel(Some("Close"))
            .extra("Draft again")
            .extra("Copy");
        frame = match &view.url {
            Some(_) => frame.primary("Open on GitHub", false, true),
            None => frame.hint(vec![Hint::Text("Nothing is sent to GitHub until you click Create".into())]).primary("Create pull request", false, view.can_create()),
        };
        // Enter in the title or the description is typing, not Create.
        let typing = ctx.memory(|m| m.focused().is_some()) && ctx.input(|i| i.key_pressed(egui::Key::Enter));
        let out = frame.show(ctx, |ui| {
            if !view.drafted && view.error.is_none() {
                ui.add_space(30.0);
                ui.vertical_centered(|ui| {
                    ui.spinner();
                    ui.add_space(8.0);
                    ui.label(RichText::new("Claude is reading the branch and drafting its pull request…").color(k.dim));
                    ui.label(RichText::new("It reads the diff, the commits and the issue — nothing is sent to GitHub until you choose.").size(theme::FS_SECONDARY).color(k.faint));
                });
                ui.add_space(30.0);
            }
            if view.drafted || view.commits > 0 {
                let commits = format!("{} commit{}", view.commits, if view.commits == 1 { "" } else { "s" });
                let mut source = if view.files.is_empty() { commits } else { format!("{commits}, {}", view.files) };
                match (&view.issue, view.number) {
                    (Some(Ok(i)), _) => source.push_str(&format!(" — issue #{}: {}", i.number, i.title)),
                    (Some(Err(e)), Some(n)) => source.push_str(&format!(" — without issue #{n}: {e}")),
                    _ => source.push_str(" — no issue in the branch name"),
                }
                modal::caption(ui, &source);
                ui.add_space(8.0);
            }
            if view.drafted {
                modal::caption(ui, "Title");
                ui.add_space(2.0);
                ui.add_enabled_ui(view.url.is_none(), |ui| modal::text_field(ui, &mut view.title, "type(scope): subject"));
                ui.add_space(10.0);
                modal::caption(ui, "Description");
                ui.add_space(2.0);
                egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                    ui.add_enabled(view.url.is_none(), egui::TextEdit::multiline(&mut view.body).font(theme::mono()).desired_rows(12).desired_width(f32::INFINITY).margin(egui::Margin::symmetric(10, 8)));
                });
            }
            if let Some(e) = &view.error {
                ui.add_space(8.0);
                modal::note(ui, k.deleted, |ui| {
                    ui.label(RichText::new(e).color(k.text));
                });
            }
            if let Some(url) = &view.url {
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Created:").color(k.added));
                    ui.hyperlink_to(RichText::new(url).font(theme::mono()), url);
                });
            } else if view.drafted {
                let warning = view.blocked().or_else(|| (view.ahead > 0).then(|| format!("{} commit{} not pushed yet: the pull request will only show what is on GitHub.", view.ahead, if view.ahead == 1 { " is" } else { "s are" })));
                if let Some(w) = warning {
                    ui.add_space(8.0);
                    modal::note(ui, k.modified, |ui| {
                        ui.label(RichText::new(w).color(k.text));
                    });
                }
                if loading {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.spinner();
                        modal::caption(ui, "Working…");
                    });
                }
            }
        });
        if out.close {
            self.pr_draft = None;
            return;
        }
        if out.primary && !typing {
            match view.url.clone() {
                Some(url) => ctx.open_url(egui::OpenUrl::new_tab(url)),
                None => self.pr_draft_create(ctx),
            }
            return;
        }
        match out.extra {
            Some(0) if !loading && view.url.is_none() => {
                view.drafted = false;
                self.pr_draft_write(ctx);
            }
            Some(1) if view.drafted => {
                ctx.copy_text(pr_draft::as_text(&view.title, &view.body));
                self.notify("Copied the pull request's title and description", false);
            }
            _ => {}
        }
    }

    /// The screenshot mode's window, with the sample project's draft.
    pub(super) fn pr_draft_sample(&mut self) {
        let (b, issue, title, body) = pr_draft::sample();
        let files = b.stat.lines().last().unwrap_or("").trim().to_string();
        self.pr_draft = Some(PrDraftView { number: Some(issue.number), branch: b.name, base: b.base, head: Some("feature/44-search-by-author".into()), slug: Some("example/sample-project".into()), commits: b.commits.len(), files, issue: Some(Ok(issue)), title, body, drafted: true, ..Default::default() });
    }
}
