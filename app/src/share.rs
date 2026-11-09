//! Handing a photo of the board (#222) to macOS: its Share menu (Mail, Messages, AirDrop, Slack
//! when it is installed…), the clipboard, and the camera's shutter sound. Elsewhere only the
//! clipboard works, through egui.

use std::path::Path;

/// Opens the Share menu of macOS for the file at `path`, under `at` (the button, in points from
/// the window's top-left corner).
pub fn share(path: &Path, at: egui::Rect) -> Result<(), String> {
    imp::share(path, at)
}

/// Puts a PNG on the clipboard, as PNG and TIFF so every app can paste it.
pub fn copy_png(png: &[u8]) -> Result<(), String> {
    imp::copy_png(png)
}

/// The system's shutter sound; silent when the Mac is.
pub fn shutter() {
    imp::shutter()
}

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use objc2::rc::Retained;
    use objc2::AnyThread;
    use objc2_app_kit::{NSApplication, NSBitmapImageRep, NSPasteboard, NSPasteboardTypePNG, NSPasteboardTypeTIFF, NSSharingServicePicker, NSSound};
    use objc2_foundation::{MainThreadMarker, NSArray, NSData, NSPoint, NSRect, NSRectEdge, NSSize, NSString, NSURL};
    use std::cell::RefCell;

    const SHUTTER: &str = "/System/Library/Components/CoreAudio.component/Contents/SharedSupport/SystemSounds/system/Shutter.aif";

    thread_local! {
        // Kept alive while macOS shows or plays them.
        static PICKER: RefCell<Option<Retained<NSSharingServicePicker>>> = const { RefCell::new(None) };
        static SOUND: RefCell<Option<Retained<NSSound>>> = const { RefCell::new(None) };
    }

    pub fn share(path: &Path, at: egui::Rect) -> Result<(), String> {
        let mtm = MainThreadMarker::new().ok_or("Sharing needs the main thread")?;
        let window = NSApplication::sharedApplication(mtm).keyWindow().ok_or("No window to share from")?;
        let view = window.contentView().ok_or("No window to share from")?;
        let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
        let items = NSArray::from_retained_slice(&[url]);
        // SAFETY: an array of NSURL is an array of objects, which the picker takes.
        let items: Retained<NSArray> = unsafe { Retained::cast_unchecked(items) };
        let picker = unsafe { NSSharingServicePicker::initWithItems(NSSharingServicePicker::alloc(), &items) };
        // AppKit's views count y from the bottom unless flipped.
        let h = view.bounds().size.height;
        let y = if view.isFlipped() { at.top() as f64 } else { h - at.bottom() as f64 };
        let rect = NSRect::new(NSPoint::new(at.left() as f64, y), NSSize::new(at.width() as f64, at.height() as f64));
        picker.showRelativeToRect_ofView_preferredEdge(rect, &view, NSRectEdge::MinY);
        PICKER.with(|p| *p.borrow_mut() = Some(picker));
        Ok(())
    }

    pub fn copy_png(png: &[u8]) -> Result<(), String> {
        let data = NSData::with_bytes(png);
        let board = NSPasteboard::generalPasteboard();
        board.clearContents();
        let tiff = NSBitmapImageRep::imageRepWithData(&data).and_then(|r| r.TIFFRepresentation());
        let png_ok = unsafe { board.setData_forType(Some(&data), NSPasteboardTypePNG) };
        let tiff_ok = tiff.is_some_and(|t| unsafe { board.setData_forType(Some(&t), NSPasteboardTypeTIFF) });
        if png_ok || tiff_ok {
            Ok(())
        } else {
            Err("Couldn't put the image on the clipboard".into())
        }
    }

    pub fn shutter() {
        if !Path::new(SHUTTER).exists() {
            return;
        }
        let sound = NSSound::initWithContentsOfFile_byReference(NSSound::alloc(), &NSString::from_str(SHUTTER), true);
        if let Some(s) = sound {
            s.play();
            SOUND.with(|k| *k.borrow_mut() = Some(s));
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;

    pub fn share(_path: &Path, _at: egui::Rect) -> Result<(), String> {
        Err("Sharing needs macOS: save the image instead".into())
    }

    pub fn copy_png(_png: &[u8]) -> Result<(), String> {
        Err("Copying an image needs macOS: save it instead".into())
    }

    pub fn shutter() {}
}
