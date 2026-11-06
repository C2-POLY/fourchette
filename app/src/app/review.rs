//! The review window: Claude reads the staged changes in the background (see crate::review) and
//! its remarks show by file and line with their severity. A click on a remark closes the window
//! and opens that file's staged diff at its line. Nothing is written and the commit stays free.
use super::*;
use crate::modal::{self, text, Modal, Width};
use crate::review::{self, Remark, Severity};
use egui::{vec2, Align2, CornerRadius, RichText, Sense};

/// Claude's summary and remarks, or why it couldn't review.
type Reply = Result<(String, Vec<Remark>), String>;

pub struct ReviewView {
    /// Staged files when the review started.
    pub files: usize,
    pub amend: bool,
    /// The staged paths, to open a remark's file in its staged diff; others open in HEAD's.
    pub staged: Vec<String>,
    pub head: Option<String>,
    pub summary: String,
    pub remarks: Vec<Remark>,
    pub error: Option<String>,
    pub job: Option<Receiver<Reply>>,
}

impl ReviewView {
    pub fn loading(&self) -> bool {
        self.job.is_some()
    }

    fn count(&self, s: Severity) -> usize {
        self.remarks.iter().filter(|r| r.severity == s).count()
    }

    /// Where a remark leads: the staged diff of its file, or the last commit's when amending a
    /// file that isn't staged.
    fn diff_req(&self, r: &Remark) -> DiffReq {
        match &self.head {
            Some(hash) if !self.staged.contains(&r.file) => DiffReq::Commit { hash: hash.clone(), path: r.file.clone() },
            _ => DiffReq::Worktree { path: r.file.clone(), staged: true, untracked: false },
        }
    }
}

/// A severity's colour.
fn colour(s: Severity) -> egui::Color32 {
    let k = theme::c();
    match s {
        Severity::Error => k.deleted,
        Severity::Warning => k.modified,
        Severity::Suggestion => k.accent_text,
    }
}

impl Tab {
    /// Opens the window and has Claude review the staged changes (with the last commit when amending).
    pub(super) fn review_open(&mut self, ctx: &egui::Context) {
        let Some(r) = &self.repo else {
            self.notify("This is the sample project — open one of your repositories first.", true);
            return;
        };
        if self.review.as_ref().is_some_and(ReviewView::loading) {
            return;
        }
        // Like Suggest: when amending, the commit replaces the last one, so both are read.
        let diff = if self.amend { r.git(&["diff", "--cached", "-M", "HEAD^"]) } else { r.git(&["diff", "--cached", "-M"]) }.unwrap_or_default();
        if diff.trim().is_empty() {
            self.notify("Stage some changes first, then Claude can review them.", true);
            return;
        }
        let head = if self.amend { r.git(&["rev-parse", "HEAD"]).ok().map(|h| h.trim().to_string()) } else { None };
        let prompt = review::prompt(&diff, self.amend);
        let model = self.claude_model;
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(crate::ai::ask(&prompt, crate::ai::SYSTEM, Some(&review::schema()), model).map(|v| review::read(&v)));
            ctx.request_repaint();
        });
        let staged: Vec<String> = self.status.staged.iter().map(|f| f.path.clone()).collect();
        self.review = Some(ReviewView { files: staged.len(), amend: self.amend, staged, head, summary: String::new(), remarks: vec![], error: None, job: Some(rx) });
    }

    pub(super) fn review_poll(&mut self) {
        let Some(view) = self.review.as_mut() else { return };
        let Some(res) = view.job.as_ref().and_then(|rx| rx.try_recv().ok()) else { return };
        view.job = None;
        match res {
            Ok((summary, remarks)) => (view.summary, view.remarks) = (summary, remarks),
            Err(e) => view.error = Some(e),
        }
    }

    /// The review window.
    pub(super) fn review_window(&mut self, ctx: &egui::Context, cmds: &mut Vec<Cmd>) {
        let Some(view) = self.review.as_ref() else { return };
        let k = theme::c();
        let loading = view.loading();
        let files = format!("{} staged file{}", view.files, if view.files == 1 { "" } else { "s" });
        let status = if view.amend { vec![text(files), text("and the last commit")] } else { vec![text(files)] };
        let mut go: Option<DiffReq> = None;
        let mut goto: Option<u32> = None;
        let out = Modal::new("review", Width::Large, "ai", vec![text("Review with Claude")])
            .status(status)
            .hint(vec![modal::Hint::Text(if view.remarks.is_empty() { "Nothing is changed in the repository" } else { "Click a remark to see its line · nothing is changed" }.into())])
            .cancel(Some("Close"))
            .primary("Review again", false, !loading)
            .show(ctx, |ui| {
                if loading {
                    ui.add_space(30.0);
                    ui.vertical_centered(|ui| {
                        ui.spinner();
                        ui.add_space(8.0);
                        ui.label(RichText::new("Claude is reading the staged changes…").color(k.dim));
                        ui.label(RichText::new("It only reads them: nothing is changed, and you can commit whenever you like.").size(theme::FS_SECONDARY).color(k.faint));
                    });
                    ui.add_space(30.0);
                    return;
                }
                if let Some(e) = &view.error {
                    modal::note(ui, k.deleted, |ui| {
                        ui.label(RichText::new(e).color(k.text));
                    });
                    return;
                }
                if !view.summary.is_empty() {
                    ui.label(RichText::new(&view.summary).color(k.text));
                    ui.add_space(6.0);
                }
                let counts: Vec<String> = Severity::ALL.into_iter().filter_map(|s| {
                    let n = view.count(s);
                    (n > 0).then(|| format!("{n} {}{}", s.label().to_lowercase(), if n == 1 { "" } else { "s" }))
                }).collect();
                if view.remarks.is_empty() {
                    modal::note(ui, k.added, |ui| {
                        ui.label(RichText::new("Nothing to report: Claude found no problem in these changes.").color(k.text));
                    });
                    return;
                }
                modal::caption(ui, &counts.join(" · "));
                ui.add_space(6.0);
                egui::ScrollArea::vertical().max_height(380.0).auto_shrink([false, true]).show(ui, |ui| {
                    let mut file = "";
                    for r in &view.remarks {
                        if r.file != file {
                            file = &r.file;
                            ui.add_space(6.0);
                            ui.label(RichText::new(file).font(theme::mono()).color(k.strong));
                            ui.add_space(2.0);
                        }
                        if remark_row(ui, r).clicked() {
                            go = Some(view.diff_req(r));
                            goto = r.line;
                        }
                    }
                });
            });
        if let Some(req) = go {
            self.review = None;
            cmds.push(Cmd::DiffAt(req, goto));
            return;
        }
        if out.close {
            self.review = None;
        } else if out.primary && !loading {
            self.review_open(ctx);
        }
    }

    /// The screenshot mode's window, with the sample project's review (`loading`: Claude still
    /// reading; `diff`: the diff a click on the first remark opens, in place of the window).
    pub(super) fn review_sample(&mut self, loading: bool, diff: bool) {
        let (summary, remarks) = review::sample();
        let job = loading.then(|| channel().1);
        let view = ReviewView { files: 1, amend: false, staged: vec!["src/graph.rs".into()], head: None, summary, remarks, error: None, job };
        if diff {
            let r = &view.remarks[0];
            self.open_diff(view.diff_req(r));
            if let Some(d) = &mut self.diff {
                d.goto = r.line;
            }
            return;
        }
        // Behind it, the local changes, where its Review button is.
        self.sel = Sel::Wip;
        self.review = Some(view);
    }
}

/// One remark: its severity as a coloured tag, its line and its message, highlighted on hover.
fn remark_row(ui: &mut egui::Ui, r: &Remark) -> egui::Response {
    let k = theme::c();
    let col = colour(r.severity);
    let w = ui.available_width();
    let left = 176.0;
    let galley = ui.painter().layout(r.message.clone(), egui::FontId::proportional(theme::FS_BODY), k.text, w - left - 12.0);
    let h = galley.size().y.max(18.0) + 12.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(6), k.hover);
    }
    let y = rect.top() + 6.0 + 9.0;
    // The severity as a small rounded tag in its colour.
    let tag = p.layout_no_wrap(r.severity.label().to_string(), egui::FontId::proportional(theme::FS_SMALL), col);
    let tag_rect = egui::Rect::from_min_size(egui::pos2(rect.left() + 8.0, y - 9.0), vec2(tag.size().x + 12.0, 18.0));
    p.rect_filled(tag_rect, CornerRadius::same(9), theme::tint(col, k.elevated, 0.16));
    p.galley(egui::pos2(tag_rect.left() + 6.0, y - tag.size().y / 2.0), tag, col);
    let line = r.line.map(|n| format!("line {n}")).unwrap_or_else(|| "file".into());
    p.text(egui::pos2(rect.left() + left - 12.0, y), Align2::RIGHT_CENTER, line, theme::mono(), k.faint);
    p.galley(egui::pos2(rect.left() + left, rect.top() + 6.0), galley, k.text);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}
