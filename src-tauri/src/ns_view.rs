use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{msg_send, ClassType};
use objc2_app_kit::{NSColor, NSView};
use objc2_foundation::{CGFloat, NSPoint, NSRect, NSSize};

pub struct NsView {
    view: Retained<NSView>,
    parent: Retained<NSView>,
}

// Safety: NSView is a reference-counted Objective-C object that can be safely
// sent between threads. We only access it from the main thread in practice
// (via Tauri commands), but the type system needs this for Arc<Mutex<Controller>>.
unsafe impl Send for NsView {}
unsafe impl Sync for NsView {}

impl NsView {
    /// Create a child NSView inside the given parent NSView.
    /// `parent_ns_view` is the raw pointer from raw-window-handle's AppKitWindowHandle.
    ///
    /// # Safety
    /// The parent pointer must be a valid NSView pointer from the current application.
    pub fn create_child(parent_ns_view: *mut NSView) -> Result<Self, String> {
        if parent_ns_view.is_null() {
            return Err("Parent NSView pointer is null".to_string());
        }

        // Safety: We trust the caller to provide a valid parent pointer
        let parent = unsafe { Retained::retain(parent_ns_view) }
            .ok_or_else(|| "Failed to retain parent NSView".to_string())?;

        // Create child view with zero frame (will be configured later)
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1.0, 1.0));
        let view = unsafe {
            // Allocate and initialize using raw msg_send
            let alloc: *mut NSView = msg_send![NSView::class(), alloc];
            let init: *mut NSView = msg_send![alloc, initWithFrame: frame];
            Retained::retain(init).ok_or_else(|| "Failed to retain child NSView".to_string())?
        };

        // Set black background
        unsafe {
            let black = NSColor::blackColor();
            let view_ptr = Retained::as_ptr(&view);
            let _: () = msg_send![view_ptr, setWantsLayer: true];
            let layer: *mut AnyObject = msg_send![view_ptr, layer];
            if !layer.is_null() {
                let cg_black: *mut AnyObject = msg_send![&black, CGColor];
                let _: () = msg_send![layer, setBackgroundColor: cg_black];
            }
        }

        // Add as subview
        unsafe {
            let parent_ptr = Retained::as_ptr(&parent);
            let view_ptr = Retained::as_ptr(&view);
            let _: () = msg_send![parent_ptr, addSubview: view_ptr];
        }

        Ok(Self { view, parent })
    }

    /// Get the NSView pointer as u64 for mpv's --wid
    pub fn view_id(&self) -> u64 {
        Retained::as_ptr(&self.view) as u64
    }

    /// Reposition and resize the child view.
    /// x, y are in CSS pixels (top-left origin) — we flip to AppKit coords internally.
    /// width, height are in CSS pixels.
    pub fn configure(&self, x: i32, y: i32, width: u32, height: u32) -> Result<(), String> {
        // Get parent height for coordinate flipping
        let parent_frame: NSRect = unsafe {
            let parent_ptr = Retained::as_ptr(&self.parent);
            msg_send![parent_ptr, frame]
        };
        let parent_height = parent_frame.size.height;

        // Convert from CSS (top-left origin) to AppKit (bottom-left origin)
        let appkit_x = x as CGFloat;
        let appkit_y = parent_height - (y as CGFloat) - (height as CGFloat);
        let appkit_width = width as CGFloat;
        let appkit_height = height as CGFloat;

        let frame = NSRect::new(
            NSPoint::new(appkit_x, appkit_y),
            NSSize::new(appkit_width, appkit_height),
        );

        unsafe {
            let view_ptr = Retained::as_ptr(&self.view);
            let _: () = msg_send![view_ptr, setFrame: frame];
        }

        Ok(())
    }

    /// Show the view (setHidden:NO)
    pub fn show(&mut self) -> Result<(), String> {
        unsafe {
            let view_ptr = Retained::as_ptr(&self.view);
            let _: () = msg_send![view_ptr, setHidden: false];
        }
        Ok(())
    }

    /// Hide the view (setHidden:YES)
    pub fn hide(&mut self) -> Result<(), String> {
        unsafe {
            let view_ptr = Retained::as_ptr(&self.view);
            let _: () = msg_send![view_ptr, setHidden: true];
        }
        Ok(())
    }
}

impl Drop for NsView {
    fn drop(&mut self) {
        // Remove from parent to clean up
        unsafe {
            let view_ptr = Retained::as_ptr(&self.view);
            let _: () = msg_send![view_ptr, removeFromSuperview];
        }
    }
}
