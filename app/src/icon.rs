//! The app icon (Dock and Cmd+Tab): a white Git branch on a teal gradient, in the macOS icon shape.
//! Drawn in `assets/icon.svg`; `scripts/app-icon.sh` renders it into `assets/icon.png`, the
//! `assets/icon.icns` of the .app bundle and the site's icons.

fn png() -> &'static [u8] {
    include_bytes!("../assets/icon.png")
}

pub fn data() -> Option<egui::IconData> {
    eframe::icon_data::from_png_bytes(png()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_icon_is_a_square_1024_png() {
        let icon = data().expect("the icon decodes");
        assert_eq!((icon.width, icon.height), (1024, 1024));
        assert_eq!(icon.rgba.len(), 1024 * 1024 * 4);
        // The corners are transparent, outside the rounded square.
        assert_eq!(icon.rgba[3], 0);
        assert_eq!(icon.rgba[1024 * 4 - 1], 0);
    }
}
