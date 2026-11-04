mod ai;
mod app;
mod demo;
mod git;
mod github;
mod graph;
mod library;
mod theme;
mod ui;
mod undo;
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
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Fourchette")
            .with_inner_size([1360.0, 840.0])
            .with_min_inner_size([900.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native("Fourchette", options, Box::new(move |cc| Ok(Box::new(app::App::new(cc, repo, screenshot)))))
}
