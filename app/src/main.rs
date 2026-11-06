mod ai;
mod app;
mod changelog;
mod cockpit;
mod checkout;
mod delete;
mod demo;
mod diff;
mod filetree;
mod git;
mod github;
mod graph;
mod home;
mod icon;
mod library;
mod menu;
mod merge;
mod modal;
mod operation;
mod pr_draft;
mod preview;
mod push;
mod pull;
mod pulls;
mod repo_state;
mod resolve;
mod settings;
mod sidebar;
mod theme;
mod titlebar;
mod ui;
mod undo;
mod update;
mod worktree;

use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    let mut screenshot: Option<PathBuf> = None;
    let mut repo: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--screenshot" {
            screenshot = args.next().map(PathBuf::from);
        } else if !a.starts_with("--") {
            repo = Some(PathBuf::from(a));
        }
    }
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Fourchette")
        .with_inner_size([1360.0, 840.0])
        .with_min_inner_size([900.0, 560.0])
        // Like the original: no title bar strip; the toolbar is the window's top edge, with the
        // red, yellow and green buttons on it. The title stays for Mission Control and the Window menu.
        .with_fullsize_content_view(true)
        .with_titlebar_shown(false)
        .with_title_shown(false);
    if let Some(icon) = icon::data() {
        viewport = viewport.with_icon(icon);
    }
    // wgpu (Metal): the OpenGL renderer shows stale or stretched frames while the window is resized.
    // Its default present mode (AutoVsync) is kept: it already presents in step with the display.
    let options = eframe::NativeOptions { viewport, renderer: eframe::Renderer::Wgpu, ..Default::default() };
    eframe::run_native("Fourchette", options, Box::new(move |cc| Ok(Box::new(app::App::new(cc, repo, screenshot)))))
}
