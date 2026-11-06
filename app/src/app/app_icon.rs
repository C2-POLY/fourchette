//! The app icon in the colour chosen in Settings › Appearance (#159): drawn in a background thread
//! (`icon::render`), then given to the Dock through AppKit (`icon::set_dock_icon`, on the main
//! thread) and, from an .app bundle, to the
//! Finder (`icon::set_finder_icon`). One drawing runs at a time; a colour asked meanwhile (the
//! picker being dragged) is drawn next, and only the last one counts.
use super::*;
use crate::icon::{self, IconColour};
use std::sync::mpsc::{channel, Receiver};

#[derive(Default)]
pub(super) struct IconState {
    /// The colour the Dock shows, or is about to.
    wanted: IconColour,
    /// The Finder icon is to be written for `wanted` (a saved choice, not a preview).
    finder: bool,
    /// The colour being drawn, and where its result comes back.
    running: Option<IconColour>,
    done: Option<Receiver<(IconColour, Option<Vec<u8>>)>>,
    /// The colour the Dock shows now.
    shown: IconColour,
    /// The .app this process runs from; None under `cargo run` or in a screenshot.
    bundle: Option<PathBuf>,
    /// FOURCHETTE_ICON=<preset|#rrggbb>: a colour shown without being saved, until one is chosen.
    pub preview: Option<IconColour>,
}

impl IconState {
    /// The colour the Settings row shows as chosen.
    pub fn chosen(&self, saved: IconColour) -> IconColour {
        self.preview.unwrap_or(saved)
    }

    /// Draws the icon in `colour` for the Dock, and for the Finder when `finder` (the choice is
    /// saved). Nothing to draw for the Dock when it already shows that colour.
    pub fn request(&mut self, colour: IconColour, finder: bool) {
        self.wanted = colour;
        self.finder |= finder && self.bundle.is_some();
        if self.running.is_none() && (colour != self.shown || self.finder) {
            self.spawn();
        }
    }

    fn spawn(&mut self) {
        let colour = self.wanted;
        let finder = std::mem::take(&mut self.finder).then(|| self.bundle.clone()).flatten();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            // The PNG is made here, off the UI thread; Teal is the shipped one.
            let png = match colour {
                IconColour::Teal => Some(icon::png().to_vec()),
                _ => icon::icon(colour).as_ref().and_then(icon::png_bytes),
            };
            if let Some(bundle) = finder {
                // Teal takes the custom icon away: the bundle's own icon.icns is the teal one.
                match colour {
                    IconColour::Teal => {
                        icon::set_finder_icon(&bundle, None);
                    }
                    _ => {
                        if let Some(png) = &png {
                            icon::set_finder_icon(&bundle, Some(png));
                        }
                    }
                }
            }
            let _ = tx.send((colour, png));
        });
        self.running = Some(colour);
        self.done = Some(rx);
    }

    /// Takes a finished drawing to the Dock, then starts the next one if another colour was asked.
    pub fn poll(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.done else { return };
        match rx.try_recv() {
            Ok((colour, png)) => {
                if let Some(png) = png {
                    icon::set_dock_icon(&png);
                }
                self.shown = colour;
                self.done = None;
                self.running = None;
                if self.wanted != self.shown || self.finder {
                    self.spawn();
                }
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => ctx.request_repaint_after(std::time::Duration::from_millis(50)),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.done = None;
                self.running = None;
            }
        }
    }
}

impl App {
    /// At launch: the saved colour (or FOURCHETTE_ICON's) in the Dock. From an .app bundle the
    /// Finder icon is written again when the colour isn't Teal, since an update replaces the
    /// bundle and the custom icon with it.
    pub(super) fn icon_start(&mut self) {
        let shot = self.screenshot.is_some();
        self.icon.bundle = if shot { None } else { crate::update::running_bundle() };
        // The window opens with the shipped teal icon (main.rs).
        self.icon.shown = IconColour::Teal;
        self.icon.preview = std::env::var("FOURCHETTE_ICON").ok().and_then(|v| IconColour::parse(&v));
        let saved = self.library.settings.app_icon;
        match self.icon.preview {
            Some(c) => self.icon.request(c, false),
            None => self.icon.request(saved, saved != IconColour::Teal),
        }
    }

    /// The colour chosen in Settings is saved: the Dock and the Finder take it.
    pub(super) fn icon_chosen(&mut self, colour: IconColour) {
        self.icon.preview = None;
        self.icon.request(colour, true);
    }

    /// The picker is being dragged: only the Dock follows, nothing is saved yet.
    pub(super) fn icon_preview(&mut self, colour: IconColour) {
        self.icon.request(colour, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dock_is_left_alone_when_it_shows_the_colour_already() {
        let mut s = IconState::default();
        s.request(IconColour::Teal, false);
        assert!(s.running.is_none(), "teal is the icon the window opens with");
        // Without a bundle the Finder is never written, so Teal asks for nothing either.
        s.request(IconColour::Teal, true);
        assert!(s.running.is_none() && !s.finder);
    }

    #[test]
    fn a_colour_asked_during_a_drawing_is_drawn_next() {
        let mut s = IconState::default();
        s.request(IconColour::Red, false);
        assert_eq!(s.running, Some(IconColour::Red));
        s.request(IconColour::Blue, false);
        assert_eq!(s.running, Some(IconColour::Red), "one at a time");
        assert_eq!(s.wanted, IconColour::Blue);
        let ctx = egui::Context::default();
        while s.shown != IconColour::Blue {
            s.poll(&ctx);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(s.running.is_none());
    }

    #[test]
    fn the_preview_shows_until_a_colour_is_chosen() {
        let s = IconState { preview: Some(IconColour::Green), ..IconState::default() };
        assert_eq!(s.chosen(IconColour::Red), IconColour::Green);
        assert_eq!(IconState::default().chosen(IconColour::Red), IconColour::Red);
    }
}
