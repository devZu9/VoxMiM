use std::cell::RefCell;
use std::io::{Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicBool, Ordering};

use objc2::rc::Retained;
use objc2::{msg_send, sel, ClassType, MainThreadMarker};
use objc2_app_kit::{
    NSColor, NSFont, NSPanel, NSScrollView, NSTextStorage, NSTextView, NSView,
};
use objc2_foundation::{NSObject, NSPoint, NSRect, NSString};

use crate::app::AppCommand;

static IS_OPEN: AtomicBool = AtomicBool::new(false);
static LOG_POS: std::sync::Mutex<u64> = std::sync::Mutex::new(0);

thread_local! {
    static PANEL: RefCell<Option<*mut NSPanel>> = const { RefCell::new(None) };
    static TEXT_VIEW: RefCell<Option<*mut NSTextView>> = const { RefCell::new(None) };
}

fn log_path() -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("logs")
        .join("session.log")
}

pub fn toggle(mtm: MainThreadMarker) {
    if IS_OPEN.fetch_xor(true, Ordering::SeqCst) {
        let panel = PANEL.with_borrow(|p| *p);
        if let Some(ptr) = panel {
            let _: () = unsafe { msg_send![ptr, orderOut: std::ptr::null::<NSObject>()] };
        }
    } else {
        let panel = PANEL.with_borrow(|p| *p);
        if let Some(ptr) = panel {
            let _: () = unsafe { msg_send![ptr, makeKeyAndOrderFront: std::ptr::null::<NSObject>()] };
            return;
        }

        let rect = NSRect {
            origin: NSPoint { x: 200.0, y: 200.0 },
            size: objc2_foundation::NSSize { width: 650.0, height: 500.0 },
        };

        let mask: u64 = (1 << 0) | (1 << 1) | (1 << 2) | (1 << 3) | (1 << 7);

        let panel: *mut NSPanel = unsafe {
            let p: *mut NSPanel = msg_send![NSPanel::class(), alloc];
            msg_send![p, initWithContentRect: rect styleMask: mask backing: 2 defer: 0]
        };
        let _: () = unsafe { msg_send![panel, setTitle: &*ns_string("VoxMiM Log")] };
        let _: () = unsafe { msg_send![panel, setFloatingPanel: true] };
        let _: () = unsafe { msg_send![panel, setReleasedWhenClosed: false] };

        let scroll: *mut NSScrollView = unsafe {
            let s: *mut NSScrollView = msg_send![NSScrollView::class(), alloc];
            msg_send![s, initWithFrame: rect]
        };
        let _: () = unsafe { msg_send![scroll, setHasVerticalScroller: true] };
        let _: () = unsafe { msg_send![scroll, setAutohidesScrollers: true] };
        let _: () = unsafe { msg_send![scroll, setBorderType: 0u64] };

        let tv: *mut NSTextView = unsafe {
            let t: *mut NSTextView = msg_send![NSTextView::class(), alloc];
            msg_send![t, initWithFrame: rect]
        };
        let _: () = unsafe { msg_send![tv, setEditable: false] };
        let _: () = unsafe { msg_send![tv, setSelectable: true] };
        let _: () = unsafe { msg_send![tv, setBackgroundColor: &*NSColor::textBackgroundColor()] };
        let _: () = unsafe { msg_send![tv, setTextColor: &*NSColor::textColor()] };
        if let Some(font) = NSFont::userFixedPitchFontOfSize(11.0) {
            let _: () = unsafe { msg_send![tv, setFont: &*font] };
        }
        let _: () = unsafe { msg_send![scroll, setDocumentView: tv] };
        let _: () = unsafe { msg_send![panel, setContentView: scroll] };

        PANEL.with_borrow_mut(|p| *p = Some(panel));
        TEXT_VIEW.with_borrow_mut(|t| *t = Some(tv));

        *LOG_POS.lock().unwrap() = 0;
        let _ = read_new_lines();

        let _: () = unsafe { msg_send![panel, makeKeyAndOrderFront: std::ptr::null::<NSObject>()] };
        IS_OPEN.store(true, Ordering::SeqCst);
    }
}

pub fn tick(mtm: MainThreadMarker) {
    if !IS_OPEN.load(Ordering::SeqCst) {
        return;
    }
    let _ = read_new_lines();
    PANEL.with_borrow(|p| {
        if let Some(ptr) = *p {
            let visible: bool = unsafe { msg_send![ptr, isVisible] };
            if !visible {
                IS_OPEN.store(false, Ordering::SeqCst);
            }
        }
    });
}

fn read_new_lines() -> Result<(), std::io::Error> {
    let path = log_path();
    if !path.exists() {
        return Ok(());
    }

    let mut file = std::fs::OpenOptions::new().read(true).open(&path)?;
    let pos = *LOG_POS.lock().unwrap();
    let file_len = file.seek(SeekFrom::End(0))?;

    if file_len <= pos {
        return Ok(());
    }

    file.seek(SeekFrom::Start(pos))?;
    let mut buf = Vec::with_capacity((file_len - pos) as usize);
    file.read_to_end(&mut buf)?;
    *LOG_POS.lock().unwrap() = file_len;

    let text = String::from_utf8_lossy(&buf).to_string();
    if text.is_empty() {
        return Ok(());
    }

    TEXT_VIEW.with_borrow(|tv| {
        if let Some(tv) = *tv {
            let storage: *mut NSTextStorage = unsafe { msg_send![tv, textStorage] };
            if !storage.is_null() {
                let attr: *mut objc2_foundation::NSAttributedString = unsafe {
                    let a: *mut objc2_foundation::NSAttributedString = msg_send![
                        objc2_foundation::NSAttributedString::class(), alloc
                    ];
                    msg_send![a, initWithString: &*ns_string(&text)]
                };
                let _: () = unsafe { msg_send![storage, appendAttributedString: attr] };
                let _: () = unsafe { msg_send![tv, scrollToEndOfDocument: std::ptr::null::<NSObject>()] };
            }
        }
    });

    Ok(())
}

fn ns_string(s: &str) -> Retained<NSString> {
    NSString::from_str(s)
}
