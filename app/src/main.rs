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

/// The app icon (Dock and Cmd+Tab): a silver fork whose tines are dipped in the graph's colours.
/// Drawn in `assets/icon.svg`; `assets/icon.icns` is the same picture for a future .app bundle.
const ICON_PNG: &[u8] = include_bytes!("../assets/icon.png");

fn app_icon() -> Option<egui::IconData> {
    eframe::icon_data::from_png_bytes(ICON_PNG).ok()
}

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
        .with_min_inner_size([900.0, 560.0]);
    if let Some(icon) = app_icon() {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native("Fourchette", options, Box::new(move |cc| Ok(Box::new(app::App::new(cc, repo, screenshot)))))
}

#[cfg(test)]
mod tests {
    #[test]
    fn app_icon_is_a_square_1024_png() {
        let icon = super::app_icon().expect("assets/icon.png decodes");
        assert_eq!((icon.width, icon.height), (1024, 1024));
        assert_eq!(icon.rgba.len(), 1024 * 1024 * 4);
        // The corners are transparent, outside the rounded square.
        assert_eq!(icon.rgba[3], 0);
    }
}
