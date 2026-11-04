//! The app icon (Dock and Cmd+Tab), one per variant: a fork whose tines are dipped in the graph's
//! colours, in silver (Classic), gold, glowing pink and cyan (Neon) or copper (Forest).
//! Drawn in `assets/icon*.svg`; `assets/icon.icns` is the Classic picture for a future .app bundle.
use crate::theme::Variant;

pub fn png(v: Variant) -> &'static [u8] {
    match v {
        Variant::Classic => include_bytes!("../assets/icon.png"),
        Variant::Gold => include_bytes!("../assets/icon-gold.png"),
        Variant::Neon => include_bytes!("../assets/icon-neon.png"),
        Variant::Forest => include_bytes!("../assets/icon-forest.png"),
    }
}

pub fn data(v: Variant) -> Option<egui::IconData> {
    eframe::icon_data::from_png_bytes(png(v)).ok()
}

/// Shows a variant's icon in the Dock and Cmd+Tab right away. eframe sets the icon given to the
/// viewport once, at start-up, and winit ignores `ViewportCommand::Icon` on macOS, so there the
/// icon is handed to AppKit directly. Call it from the main thread (egui's update runs there).
pub fn show(ctx: &egui::Context, v: Variant) {
    let Some(icon) = data(v) else { return };
    #[cfg(target_os = "macos")]
    {
        let _ = ctx;
        mac::set_app_icon(&icon);
    }
    #[cfg(not(target_os = "macos"))]
    ctx.send_viewport_cmd(egui::ViewportCommand::Icon(Some(std::sync::Arc::new(icon))));
}

#[cfg(target_os = "macos")]
mod mac {
    use objc2::ClassType as _;
    use objc2_app_kit::{NSApplication, NSBitmapFormat, NSBitmapImageRep, NSDeviceRGBColorSpace, NSImage};
    use objc2_foundation::{MainThreadMarker, NSSize};

    /// Builds the image from raw pixels rather than from the PNG: on some macOS versions an NSImage
    /// made from a PNG loads whatever libpng it finds and can crash (eframe avoids it for the same
    /// reason, egui issue 7155).
    #[allow(unsafe_code)]
    pub fn set_app_icon(icon: &egui::IconData) {
        let Some(mtm) = MainThreadMarker::new() else { return };
        let (w, h) = (icon.width as isize, icon.height as isize);
        if icon.rgba.len() != (w * h * 4) as usize {
            return;
        }
        // SAFETY: on the main thread; with null planes the bitmap owns a buffer of exactly
        // w * h * 4 bytes (4 samples of 8 bits, no padding), which is filled before use.
        unsafe {
            let Some(rep) = NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bitmapFormat_bytesPerRow_bitsPerPixel(
                NSBitmapImageRep::alloc(),
                std::ptr::null_mut(),
                w,
                h,
                8,
                4,
                true,
                false,
                NSDeviceRGBColorSpace,
                NSBitmapFormat::AlphaNonpremultiplied,
                w * 4,
                32,
            ) else {
                return;
            };
            let dst = rep.bitmapData();
            if dst.is_null() || rep.bytesPerRow() != w * 4 {
                return;
            }
            std::ptr::copy_nonoverlapping(icon.rgba.as_ptr(), dst, icon.rgba.len());
            let image = NSImage::initWithSize(NSImage::alloc(), NSSize::new(w as f64, h as f64));
            image.addRepresentation(&rep);
            NSApplication::sharedApplication(mtm).setApplicationIconImage(Some(&image));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_variant_icon_is_a_square_1024_png() {
        for v in Variant::ALL {
            let icon = data(v).unwrap_or_else(|| panic!("the {} icon decodes", v.label()));
            assert_eq!((icon.width, icon.height), (1024, 1024), "{}", v.label());
            assert_eq!(icon.rgba.len(), 1024 * 1024 * 4);
            // The corners are transparent, outside the rounded square.
            assert_eq!(icon.rgba[3], 0, "{}", v.label());
            assert_eq!(icon.rgba[1024 * 4 - 1], 0, "{}", v.label());
        }
    }
}
