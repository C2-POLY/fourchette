//! The window has no title bar strip, like the original: the content fills the window
//! (`main.rs`) and the toolbar is the top edge. On macOS the red, yellow and green buttons are
//! moved onto the toolbar's centre line, and the toolbar leaves `INSET` free on its left for them.

/// Height of the toolbar, which is also the band the window buttons are centred in.
pub const TOOLBAR_H: f32 = 60.0;
/// Space between the window's left edge and the close button.
const BUTTONS_X: f64 = 20.0;
/// Width the toolbar leaves free on its left for the three buttons (20 + 3 × 14 + 2 × 6 pt,
/// plus a gap), when they are there.
pub const INSET: f32 = 86.0;

/// The toolbar's left margin: room for the window buttons, except in full screen where
/// macOS hides them.
pub fn inset(fullscreen: bool) -> f32 {
    if cfg!(target_os = "macos") && !fullscreen {
        INSET
    } else {
        0.0
    }
}

/// The toolbar's empty background, the window's top edge: dragging it moves the window and a
/// double-click zooms it. Call it first, so the buttons drawn over it keep their clicks.
pub fn drag_area(ui: &mut egui::Ui) {
    let bg = ui.interact(ui.max_rect(), ui.id().with("window_drag"), egui::Sense::click_and_drag());
    if bg.drag_started_by(egui::PointerButton::Primary) {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    if bg.double_clicked() {
        let zoomed = ui.input(|i| i.viewport().maximized.unwrap_or(false));
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!zoomed));
    }
}

/// Height of the band the buttons are centred in: the toolbar's, or the full board's thinner
/// bar while it shows (#153). Kept as f32 bits.
static BAND_H: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(TOOLBAR_H.to_bits());

/// The height of the bar at the window's top edge, set each frame before `place_buttons`.
pub fn set_band(h: f32) {
    BAND_H.store(h.to_bits(), std::sync::atomic::Ordering::Relaxed);
}

fn band_h() -> f64 {
    f32::from_bits(BAND_H.load(std::sync::atomic::Ordering::Relaxed)) as f64
}

/// Where the buttons' container goes, in the window frame's coordinates (origin at the bottom
/// left): the top `bar_h` of a window `window_h` high.
fn band(window_h: f64, bar_h: f64) -> (f64, f64) {
    (window_h - bar_h, bar_h)
}

/// Vertical offset that centres a button `button_h` high in a band `bar_h` high.
fn centred_y(button_h: f64, bar_h: f64) -> f64 {
    ((bar_h - button_h) / 2.0).round()
}

#[cfg(target_os = "macos")]
pub use mac::place_buttons;

#[cfg(not(target_os = "macos"))]
pub fn place_buttons() {}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod mac {
    use std::ptr::NonNull;
    use std::sync::atomic::{AtomicBool, Ordering};

    use block2::RcBlock;
    use objc2_app_kit::{
        NSApplication, NSView, NSWindowButton, NSWindowDidEndLiveResizeNotification, NSWindowDidEnterFullScreenNotification,
        NSWindowDidExitFullScreenNotification, NSWindowDidResizeNotification, NSWindowStyleMask,
    };
    use objc2_foundation::{MainThreadMarker, NSNotification, NSNotificationCenter, NSPoint, NSRect};

    static OBSERVING: AtomicBool = AtomicBool::new(false);

    /// AppKit lays the titlebar out again at every step of a resize and puts the buttons back at
    /// their default spot, and egui only redraws after that: waiting for the next frame shows them
    /// jumping. So they are also placed as the window announces a resize or a full screen change,
    /// on the main thread, before the step is drawn. Registered once.
    fn observe_resizes() {
        if OBSERVING.swap(true, Ordering::Relaxed) {
            return;
        }
        let center = unsafe { NSNotificationCenter::defaultCenter() };
        let block = RcBlock::new(|_: NonNull<NSNotification>| place_buttons());
        for name in unsafe {
            [
                NSWindowDidResizeNotification,
                NSWindowDidEndLiveResizeNotification,
                NSWindowDidEnterFullScreenNotification,
                NSWindowDidExitFullScreenNotification,
            ]
        } {
            // The centre keeps the observer for the life of the app.
            std::mem::forget(unsafe { center.addObserverForName_object_queue_usingBlock(Some(name), None, None, &block) });
        }
    }

    fn differs(a: f64, b: f64) -> bool {
        (a - b).abs() > 0.5
    }

    /// Moves the close, minimise and zoom buttons onto the toolbar's centre line. AppKit puts
    /// them back after a resize or a full screen change, so this runs every frame, and on the window's
    /// resize notifications, and only touches a frame that is off. Their container is grown to the toolbar's height, the way
    /// Electron places them (`trafficLightPosition`).
    pub fn place_buttons() {
        let Some(mtm) = MainThreadMarker::new() else { return };
        observe_resizes();
        let app = NSApplication::sharedApplication(mtm);
        for window in app.windows().iter() {
            let mask = window.styleMask();
            if !mask.contains(NSWindowStyleMask::FullSizeContentView) || mask.contains(NSWindowStyleMask::FullScreen) {
                continue;
            }
            let buttons: Vec<_> = [NSWindowButton::NSWindowCloseButton, NSWindowButton::NSWindowMiniaturizeButton, NSWindowButton::NSWindowZoomButton]
                .into_iter()
                .filter_map(|b| window.standardWindowButton(b))
                .collect();
            let Some(first) = buttons.first() else { continue };
            // button → NSTitlebarView → NSTitlebarContainerView
            let Some(container) = (unsafe { first.superview() }).and_then(|v| unsafe { v.superview() }) else { continue };
            let bar_h = super::band_h();
            let (y, h) = super::band(window.frame().size.height, bar_h);
            let c = container.frame();
            if differs(c.origin.y, y) || differs(c.size.height, h) {
                unsafe { container.setFrame(NSRect::new(NSPoint::new(c.origin.x, y), objc2_foundation::NSSize::new(c.size.width, h))) };
            }
            let step = buttons.get(1).map(|b| b.frame().origin.x - first.frame().origin.x).unwrap_or(20.0);
            for (n, b) in buttons.iter().enumerate() {
                let f = b.frame();
                let want = NSPoint::new(super::BUTTONS_X + n as f64 * step, super::centred_y(f.size.height, bar_h));
                if differs(f.origin.x, want.x) || differs(f.origin.y, want.y) {
                    let view: &NSView = b;
                    unsafe { view.setFrameOrigin(want) };
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buttons_sit_on_the_toolbars_centre_line() {
        assert_eq!(band(840.0, 60.0), (780.0, 60.0));
        assert_eq!(centred_y(14.0, 60.0), 23.0);
        assert_eq!(centred_y(16.0, 60.0), 22.0);
        // The full board's thinner bar (#153).
        assert_eq!(band(840.0, 44.0), (796.0, 44.0));
        assert_eq!(centred_y(14.0, 44.0), 15.0);
    }

    #[test]
    fn the_toolbar_leaves_room_for_the_buttons_unless_full_screen() {
        assert_eq!(inset(true), 0.0);
        if cfg!(target_os = "macos") {
            // Close at 20 pt, three 14 pt buttons 20 pt apart: they end at 74 pt.
            assert!(inset(false) >= 20.0 + 2.0 * 20.0 + 14.0 + 8.0);
        }
    }
}
