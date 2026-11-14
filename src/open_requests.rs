//! Files the system asks the app to open: a double-click in the Finder, "Open With",
//! a file dropped on the Dock icon.
//!
//! macOS doesn't put these on the command line. It sends them to the application delegate
//! (`application:openURLs:`), when the app starts and while it runs. The delegate belongs to
//! winit, which ignores them, so `install` adds the missing method. The files wait in a queue
//! that `EditorApp` empties every frame.

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use eframe::egui;

static QUEUE: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
/// Wakes the app up when a file arrives while it sits idle.
static CONTEXT: OnceLock<egui::Context> = OnceLock::new();

/// Starts listening. Call before the event loop starts: the files the app is launched
/// with arrive while it starts.
pub fn install() {
    #[cfg(target_os = "macos")]
    mac::install();
}

/// The context to repaint when a file arrives.
pub fn wake(ctx: &egui::Context) {
    let _ = CONTEXT.set(ctx.clone());
}

/// The files asked for since the last call.
pub fn take() -> Vec<PathBuf> {
    std::mem::take(&mut *QUEUE.lock().unwrap_or_else(|e| e.into_inner()))
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn push(paths: Vec<PathBuf>) {
    if paths.is_empty() {
        return;
    }
    QUEUE.lock().unwrap_or_else(|e| e.into_inner()).extend(paths);
    if let Some(ctx) = CONTEXT.get() {
        ctx.request_repaint();
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use std::ffi::c_void;

    use objc2::runtime::{AnyObject, Imp, Sel};
    use objc2::{class, ffi, sel};
    use objc2_foundation::{NSArray, NSURL};

    /// `-[NSApplicationDelegate application:openURLs:]`.
    extern "C-unwind" fn open_urls(_this: &AnyObject, _cmd: Sel, _app: *mut c_void, urls: &NSArray<NSURL>) {
        super::push(urls.iter().filter_map(|url| url.to_file_path()).collect());
    }

    pub fn install() {
        // winit registers its delegate class only when the event loop is created, inside
        // eframe, and by the time eframe hands control back the launch files have already
        // been delivered. So the method goes on NSObject, which the delegate inherits from:
        // AppKit only ever calls it on the application delegate.
        let function: extern "C-unwind" fn(&AnyObject, Sel, *mut c_void, &NSArray<NSURL>) = open_urls;
        // SAFETY: the function has the signature of `application:openURLs:`, which the type
        // encoding describes (void; self, _cmd, two objects).
        unsafe {
            let imp: Imp = std::mem::transmute(function);
            let class = class!(NSObject) as *const _ as *mut _;
            ffi::class_addMethod(class, sel!(application:openURLs:), imp, c"v@:@@".as_ptr());
        }
    }
}
