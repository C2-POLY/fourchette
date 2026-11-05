//! The app icon (Dock and Cmd+Tab): a silver fork whose tines are dipped in the graph's colours.
//! Drawn in `assets/icon.svg`; `assets/icon.icns` is the same picture for the .app bundle.

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
