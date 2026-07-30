use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use objc2::rc::Retained;
use objc2::{define_class, extern_methods, msg_send, sel, ClassType, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSImage, NSMenu, NSMenuItem, NSStatusBar,
    NSStatusItem,
};
use objc2_foundation::{NSData, NSObject, NSTimer, NSString};

use crate::app::AppCommand;

pub static TRAY_RECOVERING: AtomicBool = AtomicBool::new(false);
pub fn set_recovering(v: bool) {
    TRAY_RECOVERING.store(v, Ordering::SeqCst);
}

struct TrayState {
    cmd_tx: crossbeam_channel::Sender<AppCommand>,
    recording: Arc<AtomicBool>,
    ready: Arc<AtomicBool>,
}

static TRAY_STATE: Mutex<Option<TrayState>> = Mutex::new(None);
static TRAY_SHOULD_EXIT: AtomicBool = AtomicBool::new(false);

static VAD_ON: AtomicBool = AtomicBool::new(false);
pub fn set_vad_state(on: bool) {
    VAD_ON.store(on, Ordering::SeqCst);
}

static WAKE_ON: AtomicBool = AtomicBool::new(false);
pub fn set_wake_state(on: bool) {
    WAKE_ON.store(on, Ordering::SeqCst);
}

pub struct TrayManager;

impl TrayManager {
    pub fn new(
        cmd_tx: crossbeam_channel::Sender<AppCommand>,
        recording: Arc<AtomicBool>,
        ready: Arc<AtomicBool>,
    ) -> Self {
        *TRAY_STATE.lock().unwrap() = Some(TrayState { cmd_tx, recording, ready });
        TrayManager
    }

    pub fn run(&self) {
        log::info!("TrayManager: macOS tray runs on main thread via run_tray_main()");
    }
}

pub fn tray_cmd_tx() -> MutexGuard<'static, Option<crossbeam_channel::Sender<AppCommand>>> {
    static TX: Mutex<Option<crossbeam_channel::Sender<AppCommand>>> = Mutex::new(None);
    if TX.lock().unwrap().is_none() {
        if let Some(ref state) = *TRAY_STATE.lock().unwrap() {
            *TX.lock().unwrap() = Some(state.cmd_tx.clone());
        }
    }
    TX.lock().unwrap()
}

pub fn request_exit() {
    TRAY_SHOULD_EXIT.store(true, Ordering::SeqCst);
    log::info!("Tray: exit requested");
}

pub(crate) fn icon_from_bytes(_data: &[u8]) -> *mut std::ffi::c_void {
    std::ptr::null_mut()
}

// ── Tick context (main-thread only) ──

struct TickCtx {
    item: Retained<NSStatusItem>,
    loading_img: Option<Retained<NSImage>>,
    idle_img: Option<Retained<NSImage>>,
    rec_img: Option<Retained<NSImage>>,
    mtm: MainThreadMarker,
    blink: bool,
}

thread_local! {
    static TICK: RefCell<Option<TickCtx>> = const { RefCell::new(None) };
}

// ── MenuHandler ──

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "VoxMiMMenuHandler"]
    struct MenuHandler;

    impl MenuHandler {
        #[unsafe(method(handleSettings:))]
        fn handle_settings(&self, _sender: &NSObject) {
            send_cmd(AppCommand::OpenSettings);
        }

        #[unsafe(method(handleVadToggle:))]
        fn handle_vad_toggle(&self, _sender: &NSObject) {
            VAD_ON.fetch_xor(true, Ordering::SeqCst);
            send_cmd(AppCommand::ToggleVad);
        }

        #[unsafe(method(handleWakeToggle:))]
        fn handle_wake_toggle(&self, _sender: &NSObject) {
            WAKE_ON.fetch_xor(true, Ordering::SeqCst);
            send_cmd(AppCommand::ToggleWake);
        }

        #[unsafe(method(handleMathMode:))]
        fn handle_math_mode(&self, _sender: &NSObject) {
            send_cmd(AppCommand::ToggleMathMode);
        }

        #[unsafe(method(handleEditDict:))]
        fn handle_edit_dict(&self, _sender: &NSObject) {
            send_cmd(AppCommand::EditUserDict);
        }

        #[unsafe(method(handleEditHall:))]
        fn handle_edit_hall(&self, _sender: &NSObject) {
            send_cmd(AppCommand::EditHallDict);
        }

        #[unsafe(method(handleQuit:))]
        fn handle_quit(&self, _sender: &NSObject) {
            send_cmd(AppCommand::Quit);
            TRAY_SHOULD_EXIT.store(true, Ordering::SeqCst);
            if let Some(mtm) = MainThreadMarker::new() {
                let app = NSApplication::sharedApplication(mtm);
                app.terminate(None);
            }
        }

        #[unsafe(method(tick:))]
        fn tick(&self, _timer: &NSTimer) {
            TICK.with_borrow_mut(|ctx| {
                if let Some(ref mut c) = *ctx {
                    if TRAY_SHOULD_EXIT.load(Ordering::SeqCst) {
                        return;
                    }

                    let state = TRAY_STATE.lock().unwrap();
                    let ready = state.as_ref().map(|s| s.ready.load(Ordering::SeqCst)).unwrap_or(false);
                    let recording = state.as_ref().map(|s| s.recording.load(Ordering::SeqCst)).unwrap_or(false);
                    let recovering = TRAY_RECOVERING.load(Ordering::SeqCst);
                    drop(state);

                    c.blink = !c.blink;

                    if recovering || !ready {
                        // Мигание: loading_img при blink, idle_img в паузе
                        let img = if c.blink { c.loading_img.as_ref() } else { c.idle_img.as_ref() };
                        if let Some(img) = img {
                            update_icon(&c.item, img, c.mtm);
                        }
                    } else {
                        let img = if recording { c.rec_img.as_ref() } else { c.idle_img.as_ref() };
                        if let Some(img) = img {
                            update_icon(&c.item, img, c.mtm);
                        }
                    }
                }
            });
        }
    }
);

impl MenuHandler {
    extern_methods!(
        #[unsafe(method(new))]
        fn new(mtm: MainThreadMarker) -> Retained<Self>;
    );
}

fn send_cmd(cmd: AppCommand) {
    if let Some(ref state) = *TRAY_STATE.lock().unwrap() {
        let _ = state.cmd_tx.send(cmd);
    }
}

fn ns_string(s: &str) -> Retained<NSString> {
    NSString::from_str(s)
}

fn build_menu(handler: &MenuHandler, mtm: MainThreadMarker) -> Retained<NSMenu> {
    let menu: Retained<NSMenu> = unsafe { msg_send![NSMenu::alloc(mtm), init] };

    let ver = format!("VoxMiM v{}", env!("CARGO_PKG_VERSION"));
    let ver_item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm), &ns_string(&ver), None, &ns_string(""),
        )
    };
    ver_item.setEnabled(false);
    menu.addItem(&ver_item);
    menu.addItem(&NSMenuItem::separatorItem(mtm));

    let settings_item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm), &ns_string(&crate::lang::t("tray.menu.settings")),
            Some(sel!(handleSettings:)), &ns_string(""),
        )
    };
    let _: () = unsafe { msg_send![&settings_item, setTarget: handler] };
    menu.addItem(&settings_item);
    menu.addItem(&NSMenuItem::separatorItem(mtm));

    let edit_dict = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm), &ns_string(&crate::lang::t("tray.menu.edit_dict")),
            Some(sel!(handleEditDict:)), &ns_string(""),
        )
    };
    let _: () = unsafe { msg_send![&edit_dict, setTarget: handler] };
    menu.addItem(&edit_dict);

    let edit_hall = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm), &ns_string(&crate::lang::t("tray.menu.edit_hall")),
            Some(sel!(handleEditHall:)), &ns_string(""),
        )
    };
    let _: () = unsafe { msg_send![&edit_hall, setTarget: handler] };
    menu.addItem(&edit_hall);
    menu.addItem(&NSMenuItem::separatorItem(mtm));

    let vad_item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm), &ns_string(&crate::lang::t("tray.menu.auto_stop")),
            Some(sel!(handleVadToggle:)), &ns_string(""),
        )
    };
    let _: () = unsafe { msg_send![&vad_item, setTarget: handler] };
    menu.addItem(&vad_item);

    let wake_item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm), &ns_string(&crate::lang::t("tray.menu.voice_activation")),
            Some(sel!(handleWakeToggle:)), &ns_string(""),
        )
    };
    let _: () = unsafe { msg_send![&wake_item, setTarget: handler] };
    menu.addItem(&wake_item);

    let math_item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm), &ns_string(&crate::lang::t("tray.menu.math_mode")),
            Some(sel!(handleMathMode:)), &ns_string(""),
        )
    };
    let _: () = unsafe { msg_send![&math_item, setTarget: handler] };
    menu.addItem(&math_item);
    menu.addItem(&NSMenuItem::separatorItem(mtm));

    let quit_item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm), &ns_string(&crate::lang::t("tray.menu.quit")),
            Some(sel!(handleQuit:)), &ns_string(""),
        )
    };
    let _: () = unsafe { msg_send![&quit_item, setTarget: handler] };
    menu.addItem(&quit_item);

    menu
}

fn icon_data(is_loading: bool, is_recording: bool) -> &'static [u8] {
    if is_loading {
        include_bytes!("../../assets/hourglass-fill.png")
    } else if is_recording {
        include_bytes!("../../assets/microphone-stage-light.png")
    } else {
        include_bytes!("../../assets/blue-voice.png")
    }
}

fn make_image(data: &[u8]) -> Option<Retained<NSImage>> {
    let ns_data = unsafe { NSData::dataWithBytes_length(data.as_ptr() as *const std::ffi::c_void, data.len()) };
    unsafe {
        let obj: *mut NSImage = msg_send![NSImage::class(), alloc];
        let obj: *mut NSImage = msg_send![obj, initWithData: &*ns_data];
        let img = Retained::from_raw(obj)?;
        img.setTemplate(true);
        img.setSize(objc2_foundation::NSSize { width: 18.0, height: 18.0 });
        Some(img)
    }
}

fn update_icon(item: &NSStatusItem, img: &NSImage, mtm: MainThreadMarker) {
    if let Some(btn) = item.button(mtm) {
        btn.setImage(Some(img));
    }
}

pub fn run_tray_main() {
    let mtm = MainThreadMarker::new().expect("run_tray_main must be called from main thread");

    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    let handler = MenuHandler::new(mtm);
    let menu = build_menu(&handler, mtm);

    let status_item = NSStatusBar::systemStatusBar().statusItemWithLength(-1.0);
    status_item.setAutosaveName(Some(&ns_string("VoxMiM")));
    status_item.setMenu(Some(&menu));

    let loading_img = make_image(icon_data(true, false));
    if let Some(ref img) = loading_img {
        update_icon(&status_item, img, mtm);
    }
    let idle_img = make_image(icon_data(false, false));
    let rec_img = make_image(icon_data(false, true));

    log::info!("Tray: macOS status item created");

    TICK.with_borrow_mut(|c| {
        *c = Some(TickCtx {
            item: status_item,
            loading_img,
            idle_img,
            rec_img,
            mtm,
            blink: false,
        });
    });

    // NSTimer fires tick: every 0.3s on the main run loop
    let _: Retained<NSTimer> = unsafe {
        msg_send![NSTimer::class(), scheduledTimerWithTimeInterval: 0.3, target: &*handler, selector: sel!(tick:), userInfo: None::<&NSObject>, repeats: true]
    };

    app.finishLaunching();
    app.run();
}
