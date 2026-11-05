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

/// Where the buttons' container goes, in the window frame's coordinates (origin at the bottom
/// left): the top `TOOLBAR_H` of a window `window_h` high.
fn band(window_h: f64) -> (f64, f64) {
    (window_h - TOOLBAR_H as f64, TOOLBAR_H as f64)
}

/// Vertical offset that centres a button `button_h` high in the band.
fn centred_y(button_h: f64) -> f64 {
    ((TOOLBAR_H as f64 - button_h) / 2.0).round()
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
            let (y, h) = super::band(window.frame().size.height);
            let c = container.frame();
            if differs(c.origin.y, y) || differs(c.size.height, h) {
                unsafe { container.setFrame(NSRect::new(NSPoint::new(c.origin.x, y), objc2_foundation::NSSize::new(c.size.width, h))) };
            }
            let step = buttons.get(1).map(|b| b.frame().origin.x - first.frame().origin.x).unwrap_or(20.0);
            for (n, b) in buttons.iter().enumerate() {
                let f = b.frame();
                let want = NSPoint::new(super::BUTTONS_X + n as f64 * step, super::centred_y(f.size.height));
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
        assert_eq!(band(840.0), (780.0, 60.0));
        assert_eq!(centred_y(14.0), 23.0);
        assert_eq!(centred_y(16.0), 22.0);
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
