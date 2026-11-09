//! The clone dialog (#204), opened from the home page's "Clone a repository…", File › Clone
//! Repository… and the palette: the person's GitHub repositories and their organisations' (see
//! crate::clone), a search that also takes a pasted URL or `owner/name`, a filter by account,
//! where the clone goes, then `git clone` in the background with its progress. A repository
//! already in the list is opened instead. Once cloned, it joins the list and opens.
use super::*;
use crate::clone::{self as model, Listing, Login, LoginMsg, Progress, Remote, Running};
use crate::modal::{self, text, Hint, Modal, Width};
use egui::{pos2, vec2, Align2, CornerRadius, FontId, Key, Margin, RichText, Sense, Stroke, StrokeKind, Ui};
use std::path::Path;

const ROW_H: f32 = 46.0;
const LIST_H: f32 = 300.0;

/// What the reading thread found: the listing, and the GitHub repositories of the list's folders.
type Found = (Result<Listing, String>, Vec<(PathBuf, Vec<String>)>);

pub struct CloneView {
    pub query: String,
    /// The account pill chosen; None is All.
    account: Option<String>,
    /// None while `gh` is read.
    listing: Option<Result<Listing, String>>,
    /// The repositories of the list and the GitHub repositories their remotes point at.
    known: Vec<(PathBuf, Vec<String>)>,
    rx: Option<Receiver<Found>>,
    /// The URL of the row chosen; the first row when it is gone.
    selected: Option<String>,
    /// The folder the clone goes into.
    parent: PathBuf,
    running: Option<Running>,
    progress: Option<Progress>,
    error: Option<String>,
    /// The destination checked last and what came of it, so the disk is read once per change.
    dest_check: Option<(PathBuf, Result<(), String>)>,
    /// `gh auth login --web` under way, from "Sign in to GitHub".
    login: Option<Login>,
    /// Its one-time code and the page to type it on, once `gh` gave them.
    device: Option<(String, String)>,
    /// Why the sign-in failed or stopped.
    login_error: Option<String>,
    /// Screenshots: invented repositories, no disk and no network.
    sample: bool,
    focused: bool,
}

/// A button of the note that stands in for the list.
enum Act {
    SignIn,
    StopSignIn,
    Copy(String),
    OpenUrl(String),
}

/// A repository the dialog offers.
#[derive(Clone)]
struct Row {
    url: String,
    /// The folder `git clone` makes.
    name: String,
    remote: Option<Remote>,
    /// Typed or pasted in the search field rather than listed.
    typed: bool,
    /// Already in the list, there.
    cloned: Option<PathBuf>,
}

impl CloneView {
    fn new(parent: PathBuf) -> Self {
        CloneView { query: String::new(), account: None, listing: None, known: Vec::new(), rx: None, selected: None, parent, running: None, progress: None, error: None, dest_check: None, login: None, device: None, login_error: None, sample: false, focused: false }
    }

    fn ssh(&self) -> bool {
        matches!(&self.listing, Some(Ok(l)) if l.ssh)
    }

    fn signing_in(&self) -> bool {
        self.login.is_some() || (self.sample && self.device.is_some())
    }

    fn busy(&self) -> bool {
        self.running.is_some() || (self.sample && self.progress.is_some())
    }

    /// The rows for the search and the account: what the field names first, unless it is one of
    /// the listed repositories, then the listed ones that match.
    fn rows(&self) -> Vec<Row> {
        let listed: &[Remote] = match &self.listing {
            Some(Ok(l)) => &l.repos,
            _ => &[],
        };
        let ssh = self.ssh();
        let cloned = |slug: Option<&str>| slug.and_then(|s| model::already_cloned(s, &self.known));
        let row = |r: &Remote| Row { url: r.url(ssh), name: r.name.clone(), remote: Some(r.clone()), typed: false, cloned: cloned(Some(&r.slug)) };
        if let Some(src) = model::parse_source(&self.query, ssh) {
            if let Some(r) = src.slug.as_deref().and_then(|s| listed.iter().find(|r| r.slug.eq_ignore_ascii_case(s))) {
                return vec![row(r)];
            }
            let cloned = cloned(src.slug.as_deref());
            let mut rows = vec![Row { url: src.url, name: src.name, remote: None, typed: true, cloned }];
            rows.extend(model::filter(listed, &self.query, self.account.as_deref()).into_iter().map(row));
            return rows;
        }
        model::filter(listed, &self.query, self.account.as_deref()).into_iter().map(row).collect()
    }

    fn chosen<'a>(&self, rows: &'a [Row]) -> Option<&'a Row> {
        self.selected.as_ref().and_then(|u| rows.iter().find(|r| &r.url == u)).or_else(|| rows.first())
    }

    /// Whether `dest` can take the clone, read again only when it changes.
    fn dest_ok(&mut self, dest: &Path) -> Result<(), String> {
        if self.sample {
            return Ok(());
        }
        if self.dest_check.as_ref().is_none_or(|(p, _)| p != dest) {
            self.dest_check = Some((dest.to_path_buf(), model::check_dest(dest)));
        }
        self.dest_check.as_ref().map(|(_, r)| r.clone()).unwrap_or(Ok(()))
    }
}

impl App {
    /// Where clones go: the folder used last, else `~/dev`, else the home folder.
    fn clone_parent(&self) -> PathBuf {
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"));
        match &self.library.clone_parent {
            Some(p) if p.is_dir() => p.clone(),
            _ => model::default_parent(&home, Path::is_dir),
        }
    }

    /// Opens the dialog and reads the person's repositories in the background.
    pub(super) fn clone_open(&mut self, ctx: &egui::Context) {
        if self.clone.is_some() {
            return;
        }
        self.palette = None;
        self.switcher = None;
        self.clone = Some(CloneView::new(self.clone_parent()));
        self.clone_list(ctx);
    }

    /// Reads the person's repositories and the list's remotes in the background (again after a
    /// sign-in).
    fn clone_list(&mut self, ctx: &egui::Context) {
        let paths: Vec<PathBuf> = self.library.entries.iter().map(|e| e.path.clone()).collect();
        let Some(view) = self.clone.as_mut() else { return };
        view.listing = None;
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let known = model::known_slugs(&paths);
            let _ = tx.send((model::list(), known));
            ctx.request_repaint();
        });
        view.rx = Some(rx);
    }

    /// "Sign in to GitHub": `gh auth login --web` in the background.
    fn clone_sign_in(&mut self, ctx: &egui::Context) {
        let Some(view) = self.clone.as_mut() else { return };
        if view.sample || view.signing_in() {
            return;
        }
        view.login_error = None;
        view.device = None;
        let c = ctx.clone();
        match model::login(move || c.request_repaint()) {
            Ok(l) => view.login = Some(l),
            Err(e) => view.login_error = Some(e),
        }
    }

    /// For `FOURCHETTE_CLONE=1|cloning|error|signed-out|signing-in|no-gh` screenshots: the sample
    /// repositories, one already in the list, and the clone of the first under way or failed; or
    /// `gh` signed out (its sign-in waiting for the browser), or not installed.
    pub(super) fn clone_sample(&mut self, state: &str) {
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/Users/me"));
        let mut view = CloneView::new(home.join("dev"));
        view.sample = true;
        let signed_out = state == "signed-out" || state == "signing-in";
        view.listing = Some(match state {
            "no-gh" => Err(model::NO_GH.into()),
            _ if signed_out => Err(model::NOT_SIGNED_IN.into()),
            _ => Ok(model::sample()),
        });
        view.known = vec![(home.join("dev/design-tokens"), vec!["acme/design-tokens".into()])];
        match state {
            "cloning" => view.progress = Some(Progress { text: "Receiving objects: 62% (1843/2972), 14.20 MiB | 6.10 MiB/s".into(), fraction: Some(0.08 + 0.77 * 0.62) }),
            "error" => view.error = Some("remote: Repository not found.\nfatal: repository 'https://github.com/acme/storefront.git/' not found".into()),
            "signed-out" => view.query = "nightowl-labs/lantern".into(),
            "signing-in" => view.device = Some(("82F4-C47F".into(), model::DEVICE_URL.into())),
            _ => {}
        }
        self.clone = Some(view);
    }

    /// Change…: another folder for clones, kept for next time.
    pub(super) fn clone_pick_folder(&mut self) {
        let Some(view) = self.clone.as_mut() else { return };
        if let Some(p) = rfd::FileDialog::new().set_title("Clone into").set_directory(&view.parent).pick_folder() {
            view.parent = p.clone();
            view.dest_check = None;
            if !view.sample {
                self.library.clone_parent = Some(p);
                self.library.save();
            }
        }
    }

    /// Once cloned, or for a repository already in the list: it opens in the home page's tab,
    /// else in a new one, which adds it to the list (`App::set_repo`).
    pub(super) fn clone_done(&mut self, path: PathBuf) {
        self.clone = None;
        let new_tab = !self.tab().home;
        self.open_path(path, new_tab);
    }

    fn clone_poll(&mut self, ctx: &egui::Context, cmds: &mut Vec<Cmd>) {
        let Some(view) = self.clone.as_mut() else { return };
        let (mut signed_in, mut finished) = (false, false);
        if let Some(l) = &view.login {
            while let Ok(m) = l.rx.try_recv() {
                match m {
                    // The page opens at once; the code is shown to copy, and gh put it on the clipboard.
                    LoginMsg::Code { code, url } => {
                        cmds.push(Cmd::OpenUrl(url.clone()));
                        view.device = Some((code, url));
                    }
                    LoginMsg::Done(r) => {
                        view.device = None;
                        finished = true;
                        match r {
                            Ok(()) => signed_in = true,
                            Err(e) => view.login_error = Some(e),
                        }
                    }
                }
            }
            if finished {
                view.login = None;
            }
        }
        if signed_in {
            self.notify("Signed in to GitHub", false);
            self.clone_list(ctx);
        }
        let Some(view) = self.clone.as_mut() else { return };
        if let Some((listing, known)) = view.rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
            view.listing = Some(listing);
            view.known = known;
            view.rx = None;
        }
        let mut done = None;
        if let Some(run) = &view.running {
            while let Ok(m) = run.rx.try_recv() {
                match m {
                    model::Msg::Progress(p) => view.progress = Some(p),
                    model::Msg::Done(r) => done = Some(r),
                }
            }
        }
        match done {
            Some(Ok(path)) => {
                view.running = None;
                cmds.push(Cmd::CloneOpen(path));
            }
            Some(Err(e)) => {
                view.running = None;
                view.progress = None;
                view.error = Some(e);
                view.dest_check = None;
            }
            None => {}
        }
    }

    /// Starts `git clone` of the chosen row into the folder shown.
    fn clone_start(&mut self, row: &Row, ctx: &egui::Context) {
        let Some(view) = self.clone.as_mut() else { return };
        if view.sample || view.busy() {
            return;
        }
        let dest = view.parent.join(&row.name);
        view.error = None;
        let c = ctx.clone();
        match model::start(&row.url, &dest, move || c.request_repaint()) {
            Ok(run) => {
                view.running = Some(run);
                view.progress = Some(Progress { text: "Connecting…".into(), fraction: Some(0.0) });
                self.library.clone_parent = Some(view.parent.clone());
                self.library.save();
            }
            Err(e) => {
                view.error = Some(e);
                view.dest_check = None;
            }
        }
    }

    /// The dialog.
    pub(super) fn clone_window(&mut self, ctx: &egui::Context, cmds: &mut Vec<Cmd>) {
        self.clone_poll(ctx, cmds);
        let Some(view) = self.clone.as_mut() else { return };
        let k = theme::c();
        let rows = view.rows();
        let busy = view.busy();
        // Up and Down move the choice, even with the search field focused.
        if !busy {
            let (up, down) = ctx.input(|i| (i.key_pressed(Key::ArrowUp), i.key_pressed(Key::ArrowDown)));
            if (up || down) && !rows.is_empty() {
                let at = view.chosen(&rows).and_then(|c| rows.iter().position(|r| r.url == c.url)).unwrap_or(0);
                let next = if down { (at + 1).min(rows.len() - 1) } else { at.saturating_sub(1) };
                view.selected = Some(rows[next].url.clone());
            }
        }
        let chosen = view.chosen(&rows).cloned();
        let dest = chosen.as_ref().map(|r| view.parent.join(&r.name));
        let dest_ok = match (&chosen, &dest) {
            (Some(r), Some(d)) if r.cloned.is_none() => view.dest_ok(d),
            _ => Ok(()),
        };
        let status = match &view.listing {
            None => "Reading your GitHub repositories…".to_string(),
            Some(Ok(l)) => format!("From your GitHub account ({}) and its organisations", l.login),
            Some(Err(_)) => "Paste a repository's address, or owner/name on GitHub".to_string(),
        };
        let (label, enabled) = match (&chosen, busy) {
            (_, true) => ("Cloning…", false),
            (Some(r), _) if r.cloned.is_some() => ("Open", true),
            (Some(_), _) => ("Clone and open", dest_ok.is_ok()),
            (None, _) => ("Clone and open", false),
        };
        let hint = if busy {
            vec![Hint::Text("Cancel stops the clone and removes what it wrote".into())]
        } else {
            vec![Hint::Key(modal::Cap::Up), Hint::Key(modal::Cap::Down), Hint::Text("to choose".into()), Hint::Key(modal::Cap::Text("Enter")), Hint::Text("to clone".into())]
        };
        let frame = Modal::new("clone", Width::Large, "clone", vec![text("Clone a repository")]).status(vec![text(status)]).hint(hint).primary(label, false, enabled);
        let mut pick = None;
        let mut change = false;
        let mut act = None;
        let out = frame.show(ctx, |ui| {
            ui.add_enabled_ui(!busy, |ui| {
                let field = modal::text_field(ui, &mut view.query, "Search your repositories, or paste a URL or owner/name…");
                if !view.focused {
                    field.request_focus();
                    view.focused = true;
                }
                ui.add_space(8.0);
                if let Some(Ok(l)) = &view.listing {
                    let accounts: Vec<String> = std::iter::once(l.login.clone()).chain(l.orgs.iter().cloned()).collect();
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                        if chip(ui, "All", view.account.is_none()).clicked() {
                            view.account = None;
                        }
                        for a in accounts {
                            let on = view.account.as_deref() == Some(a.as_str());
                            if chip(ui, &a, on).clicked() {
                                view.account = if on { None } else { Some(a) };
                            }
                        }
                    });
                    ui.add_space(6.0);
                }
                list(ui, view, &rows, chosen.as_ref(), &mut pick, &mut act);
            });
            ui.add_space(12.0);
            if let (Some(r), Some(d)) = (&chosen, &dest) {
                match &r.cloned {
                    Some(p) => {
                        ui.horizontal(|ui| {
                            field_label(ui, "Already in");
                            path_box(ui, &model::tilde(p), ui.available_width());
                        });
                        modal::caption(ui, "This repository is already in your list: Open shows it rather than cloning it again.");
                    }
                    None => {
                        ui.horizontal(|ui| {
                            field_label(ui, "Clone into");
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if modal::button(ui, "Change…", modal::Kind::Plain, !busy).on_hover_text("Choose the folder clones go into; it is kept for next time").clicked() {
                                    change = true;
                                }
                                path_box(ui, &model::tilde(d), ui.available_width());
                            });
                        });
                        if let Err(e) = &dest_ok {
                            ui.add_space(6.0);
                            modal::note(ui, k.modified, |ui| {
                                ui.label(RichText::new(e).color(k.text));
                            });
                        }
                    }
                }
            }
            if busy {
                if let Some(p) = &view.progress {
                    progress(ui, p);
                }
            }
            if let Some(e) = &view.error {
                ui.add_space(8.0);
                modal::note(ui, k.deleted, |ui| {
                    ui.label(RichText::new("The clone failed").strong().color(k.strong));
                    ui.label(RichText::new(e).font(theme::mono()).color(k.text));
                });
            }
        });
        if let Some(url) = pick {
            view.selected = Some(url);
        }
        if out.close {
            if let Some(run) = &view.running {
                run.stop();
            }
            if let Some(l) = &view.login {
                l.stop();
            }
            self.clone = None;
            return;
        }
        if change {
            cmds.push(Cmd::ClonePickFolder);
        }
        match act {
            Some(Act::SignIn) => self.clone_sign_in(ctx),
            Some(Act::StopSignIn) => {
                if let Some(l) = &view.login {
                    l.stop();
                }
            }
            Some(Act::Copy(t)) => cmds.push(Cmd::Copy(t)),
            Some(Act::OpenUrl(u)) => cmds.push(Cmd::OpenUrl(u)),
            None => {}
        }
        if out.primary && enabled {
            match chosen {
                Some(Row { cloned: Some(p), .. }) => cmds.push(Cmd::CloneOpen(p)),
                Some(r) => self.clone_start(&r, ctx),
                None => {}
            }
        }
    }
}

/// The rows, or what stands in for them: a spinner while `gh` is read, the note without it.
fn list(ui: &mut Ui, view: &CloneView, rows: &[Row], chosen: Option<&Row>, pick: &mut Option<String>, act: &mut Option<Act>) {
    let k = theme::c();
    let frame = egui::Frame::new().fill(k.input).stroke(Stroke::new(1.0_f32, k.border)).corner_radius(CornerRadius::same(10)).inner_margin(Margin::same(4));
    frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.set_min_height(LIST_H);
        egui::ScrollArea::vertical().max_height(LIST_H).auto_shrink([false, true]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            for r in rows.iter().filter(|r| r.typed) {
                if row(ui, r, chosen.is_some_and(|c| c.url == r.url)).clicked() {
                    *pick = Some(r.url.clone());
                }
            }
            match &view.listing {
                None => {
                    ui.add_space(LIST_H / 2.0 - 30.0);
                    ui.vertical_centered(|ui| {
                        ui.spinner();
                        ui.add_space(6.0);
                        ui.label(RichText::new("Reading your repositories with the GitHub command line…").color(k.dim));
                    });
                }
                Some(Err(e)) => {
                    ui.add_space(8.0);
                    egui::Frame::new().inner_margin(Margin::symmetric(8, 4)).show(ui, |ui| signed_out(ui, view, e, act));
                }
                Some(Ok(_)) => {
                    let listed: Vec<&Row> = rows.iter().filter(|r| !r.typed).collect();
                    if listed.is_empty() && rows.is_empty() {
                        ui.add_space(LIST_H / 2.0 - 12.0);
                        ui.vertical_centered(|ui| ui.label(RichText::new("Nothing matches — paste a URL or type owner/name to clone another repository").color(k.faint)));
                    }
                    for r in listed {
                        let on = chosen.is_some_and(|c| c.url == r.url);
                        let resp = row(ui, r, on);
                        if resp.clicked() {
                            *pick = Some(r.url.clone());
                        }
                        if on && ui.input(|i| i.key_pressed(Key::ArrowUp) || i.key_pressed(Key::ArrowDown)) {
                            resp.scroll_to_me(None);
                        }
                    }
                }
            }
        });
    });
}

/// Without `gh`, or signed out: why the list is empty, and what to do about it — copy the
/// install command, or sign in through the browser, its code shown while GitHub waits.
fn signed_out(ui: &mut Ui, view: &CloneView, e: &str, act: &mut Option<Act>) {
    let k = theme::c();
    if view.signing_in() {
        // A tinted box like a note's, without its warning sign: nothing is wrong.
        let frame = egui::Frame::new()
            .fill(theme::tint(k.accent, k.elevated, 0.08))
            .stroke(Stroke::new(1.0_f32, theme::tint(k.accent, k.elevated, 0.3)))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::symmetric(14, 12));
        frame.show(ui, |ui| {
            ui.set_width(ui.available_width());
            match &view.device {
                Some((code, url)) => {
                    ui.label(RichText::new("Type this code on the GitHub page that opened in your browser:").color(k.text));
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(code).font(FontId::monospace(26.0)).strong().color(k.strong));
                        ui.add_space(10.0);
                        if modal::button(ui, "Copy", modal::Kind::Plain, true).on_hover_text("gh has already put it on the clipboard").clicked() {
                            *act = Some(Act::Copy(code.clone()));
                        }
                        if modal::button(ui, "Open the page again", modal::Kind::Plain, true).on_hover_text(url.as_str()).clicked() {
                            *act = Some(Act::OpenUrl(url.clone()));
                        }
                    });
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(RichText::new("Waiting for you to approve in the browser…").color(k.dim));
                    });
                }
                None => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(RichText::new("Asking GitHub for a sign-in code…").color(k.dim));
                    });
                }
            }
            ui.add_space(6.0);
            if modal::button(ui, "Cancel sign-in", modal::Kind::Plain, true).clicked() {
                *act = Some(Act::StopSignIn);
            }
        });
    } else {
        modal::note(ui, k.modified, |ui| {
            ui.label(RichText::new(e).color(k.text));
            ui.add_space(8.0);
            if e == model::NO_GH {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(model::INSTALL_GH).font(theme::mono()).color(k.strong));
                    ui.add_space(8.0);
                    if modal::button(ui, "Copy", modal::Kind::Plain, true).on_hover_text("Paste it in Terminal to install the GitHub command line with Homebrew").clicked() {
                        *act = Some(Act::Copy(model::INSTALL_GH.into()));
                    }
                });
            } else if e == model::NOT_SIGNED_IN && modal::button(ui, "Sign in to GitHub", modal::Kind::Primary, true).on_hover_text("Runs gh auth login in the background: you approve in your browser with a one-time code").clicked() {
                *act = Some(Act::SignIn);
            }
        });
    }
    if let Some(err) = &view.login_error {
        ui.add_space(8.0);
        modal::note(ui, k.deleted, |ui| {
            ui.label(RichText::new("The sign-in didn't go through").strong().color(k.strong));
            ui.label(RichText::new(err).font(theme::mono()).color(k.text));
        });
    }
}

/// One repository: owner and name, its marks, then its description or when it changed; on the
/// right, "Already cloned" or when it changed.
fn row(ui: &mut Ui, r: &Row, on: bool) -> egui::Response {
    let k = theme::c();
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
    let p = ui.painter();
    let fill = if on { k.select } else if resp.hovered() { k.hover } else { egui::Color32::TRANSPARENT };
    p.rect_filled(rect, CornerRadius::same(7), fill);
    let x = rect.left() + 12.0;
    let top = rect.top() + 14.0;
    let bottom = rect.top() + 32.0;
    let body = FontId::proportional(theme::FS_BODY);
    let small = FontId::proportional(theme::FS_SECONDARY);
    // The right side first, so the name and description stop before it.
    let mut right = rect.right() - 12.0;
    let updated = r.remote.as_ref().map(|m| format!("updated {}", crate::git::relative_time(m.updated)));
    if r.cloned.is_some() {
        right = tag(p, right, rect.center().y, "Already cloned", k.accent_text, k.accent_soft, true) - 10.0;
    } else if let (Some(u), Some(m)) = (&updated, &r.remote) {
        if !m.description.is_empty() {
            let rr = p.text(pos2(right, rect.center().y), Align2::RIGHT_CENTER, u, small.clone(), k.faint);
            right = rr.left() - 14.0;
        }
    }
    let (owner, name, line) = match &r.remote {
        Some(m) => (format!("{}/", m.owner), m.name.clone(), if m.description.is_empty() { updated.clone().unwrap_or_default() } else { m.description.clone() }),
        None => (String::new(), r.name.clone(), r.url.clone()),
    };
    let mut at = x;
    if !owner.is_empty() {
        at = p.text(pos2(at, top), Align2::LEFT_CENTER, &owner, body.clone(), k.dim).right();
    }
    let title = if r.typed { format!("Clone {name}") } else { name };
    at = crate::ui::fit_text(p, pos2(at, top), Align2::LEFT_CENTER, &title, body, k.strong, (right - at).max(20.0)).right() + 8.0;
    if let Some(m) = &r.remote {
        for (on, label) in [(m.private, "Private"), (m.fork, "Fork")] {
            if on && at + 60.0 < right {
                at = tag(p, at, top, label, k.dim, k.input, false) + 6.0;
            }
        }
    }
    let line_font = if r.typed { theme::mono() } else { small };
    crate::ui::fit_text(p, pos2(x, bottom), Align2::LEFT_CENTER, &line, line_font, k.dim, (right - x).max(20.0));
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A small outlined mark. `from_right`: `x` is its right edge, else its left; returns the other edge.
fn tag(p: &egui::Painter, x: f32, y: f32, text: &str, fg: egui::Color32, bg: egui::Color32, from_right: bool) -> f32 {
    let k = theme::c();
    let g = p.layout_no_wrap(text.to_string(), FontId::proportional(theme::FS_SMALL), fg);
    let w = g.size().x + 14.0;
    let left = if from_right { x - w } else { x };
    let rect = egui::Rect::from_min_size(pos2(left, y - 9.0), vec2(w, 18.0));
    p.rect(rect, CornerRadius::same(9), bg, Stroke::new(1.0_f32, k.border), StrokeKind::Inside);
    p.galley(rect.center() - g.size() / 2.0, g, fg);
    if from_right { left } else { left + w }
}

/// An account pill of the filter.
fn chip(ui: &mut Ui, text: &str, on: bool) -> egui::Response {
    let k = theme::c();
    let font = FontId::proportional(theme::FS_SECONDARY);
    let fg = if on { k.accent_text } else { k.dim };
    let g = ui.painter().layout_no_wrap(text.to_string(), font, fg);
    let (rect, resp) = ui.allocate_exact_size(vec2(g.size().x + 22.0, 26.0), Sense::click());
    let p = ui.painter();
    let fill = if on { k.accent_soft } else if resp.hovered() { k.hover } else { k.input };
    let stroke = if on { theme::tint(k.accent, k.elevated, 0.5) } else { k.border };
    p.rect(rect, CornerRadius::same(13), fill, Stroke::new(1.0_f32, stroke), StrokeKind::Inside);
    p.galley(rect.center() - g.size() / 2.0, g, if on { k.accent_text } else if resp.hovered() { k.text } else { k.dim });
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The caption left of the destination.
fn field_label(ui: &mut Ui, text: &str) {
    let (rect, _) = ui.allocate_exact_size(vec2(76.0, 30.0), Sense::hover());
    ui.painter().text(pos2(rect.left(), rect.center().y), Align2::LEFT_CENTER, text, FontId::proportional(theme::FS_SECONDARY), theme::c().faint);
}

/// The destination, in a field-like box `w` wide.
fn path_box(ui: &mut Ui, path: &str, w: f32) {
    let k = theme::c();
    let (rect, _) = ui.allocate_exact_size(vec2(w.max(60.0), 30.0), Sense::hover());
    let p = ui.painter();
    p.rect(rect, CornerRadius::same(8), k.input, Stroke::new(1.0_f32, k.border), StrokeKind::Inside);
    crate::ui::fit_text(p, pos2(rect.left() + 10.0, rect.center().y), Align2::LEFT_CENTER, path, theme::mono(), k.text, rect.width() - 20.0);
}

/// Git's progress line and the bar of the whole clone.
fn progress(ui: &mut Ui, p: &Progress) {
    let k = theme::c();
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        ui.spinner();
        ui.label(RichText::new(&p.text).font(theme::mono()).color(k.text));
    });
    ui.add_space(6.0);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 6.0), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(3), k.border_strong);
    let mut done = rect;
    done.set_width(rect.width() * p.fraction.unwrap_or(0.0).clamp(0.0, 1.0));
    painter.rect_filled(done, CornerRadius::same(3), k.accent);
}
