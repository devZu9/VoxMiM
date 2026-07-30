use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use objc2::rc::Retained;
use objc2::{define_class, extern_methods, msg_send, sel, ClassType, MainThreadMarker, MainThreadOnly, runtime::Sel};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSButton, NSControlStateValue,
    NSControlStateValueOff, NSControlStateValueOn, NSImage, NSMenu, NSMenuItem, NSPanel,
    NSStatusBar, NSStatusItem, NSTextField, NSView,
};
use objc2_foundation::{NSData, NSPoint, NSRect, NSObject, NSTimer, NSString};

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

// ── Dialog context ──

#[derive(Copy, Clone)]
struct SendPtr(*mut MenuHandler);
unsafe impl Send for SendPtr {}
unsafe impl std::marker::Sync for SendPtr {}

thread_local! {
    static DLG_PANEL: RefCell<Option<*mut NSPanel>> = const { RefCell::new(None) };
    static DLG_FIELD1: RefCell<Option<*mut NSTextField>> = const { RefCell::new(None) };
    static DLG_FIELD2: RefCell<Option<*mut NSTextField>> = const { RefCell::new(None) };
    static DLG_IS_HALL: RefCell<bool> = const { RefCell::new(false) };
}
static DLG_HANDLER: std::sync::Mutex<Option<SendPtr>> = std::sync::Mutex::new(None);

struct TickCtx {
    item: Retained<NSStatusItem>,
    loading_img: Option<Retained<NSImage>>,
    idle_img: Option<Retained<NSImage>>,
    rec_img: Option<Retained<NSImage>>,
    vad_item: Retained<NSMenuItem>,
    wake_item: Retained<NSMenuItem>,
    mtm: MainThreadMarker,
    blink: bool,
    start_time: std::time::Instant,
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

        #[unsafe(method(handleAddWord:))]
        fn handle_add_word(&self, _sender: &NSObject) {
            show_input_dialog(
                &ns_string(&crate::lang::t("dialog.add_word.title")),
                &ns_string(&crate::lang::t("dialog.add_word.label_wrong")),
                &ns_string(&crate::lang::t("dialog.add_word.label_correct")),
                &ns_string(&crate::lang::t("dialog.add_word.add")),
                &ns_string(&crate::lang::t("dialog.add_word.cancel")),
                false,
            );
        }

        #[unsafe(method(handleAddHall:))]
        fn handle_add_hall(&self, _sender: &NSObject) {
            show_input_dialog(
                &ns_string(&crate::lang::t("dialog.add_hall.title")),
                &ns_string(&crate::lang::t("dialog.add_hall.label")),
                &ns_string(""),
                &ns_string(&crate::lang::t("dialog.add_word.add")),
                &ns_string(&crate::lang::t("dialog.add_word.cancel")),
                true,
            );
        }

        #[unsafe(method(handleDlgAdd:))]
        fn handle_dlg_add(&self, _sender: &NSObject) {
            DLG_PANEL.with_borrow(|p| {
                if let Some(panel) = *p {
                    let label2 = DLG_FIELD2.with_borrow(|f| f.and_then(|ptr| read_text_field(ptr)));
                    let is_hall = DLG_IS_HALL.with_borrow(|h| *h);

                    if is_hall {
                        if let Some(phrase) = label2 {
                            send_cmd(AppCommand::AddHallEntry { phrase });
                        }
                    } else {
                        let label1 = DLG_FIELD1.with_borrow(|f| f.and_then(|ptr| read_text_field(ptr)));
                        if let (Some(wrong), Some(correct)) = (label1, label2) {
                            if !wrong.is_empty() && !correct.is_empty() {
                                send_cmd(AppCommand::AddUserEntry { wrong, correct });
                            }
                        }
                    }
                    // orderOut — скрыть, не удалять. Потом очищаем ссылки
                    let _: () = unsafe { msg_send![panel, orderOut: std::ptr::null::<NSObject>()] };
                }
            });
            DLG_PANEL.with_borrow_mut(|p| *p = None);
            DLG_FIELD1.with_borrow_mut(|f| *f = None);
            DLG_FIELD2.with_borrow_mut(|f| *f = None);
        }

        #[unsafe(method(handleDlgCancel:))]
        fn handle_dlg_cancel(&self, _sender: &NSObject) {
            DLG_PANEL.with_borrow(|p| {
                if let Some(panel) = *p {
                    let _: () = unsafe { msg_send![panel, orderOut: std::ptr::null::<NSObject>()] };
                }
            });
            DLG_PANEL.with_borrow_mut(|p| *p = None);
            DLG_FIELD1.with_borrow_mut(|f| *f = None);
            DLG_FIELD2.with_borrow_mut(|f| *f = None);
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

        #[unsafe(method(handleShowLog:))]
        fn handle_show_log(&self, _sender: &NSObject) {
            if let Some(mtm) = MainThreadMarker::new() {
                crate::ui::log_macos::toggle(mtm);
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

                    // Обновление галочек меню
                    let on = NSControlStateValueOn;
                    let off = NSControlStateValueOff;
                    c.vad_item.setState(if VAD_ON.load(Ordering::SeqCst) { on } else { off });
                    c.wake_item.setState(if WAKE_ON.load(Ordering::SeqCst) { on } else { off });

                    if recovering || !ready || c.start_time.elapsed().as_secs() < 3 {
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
                    crate::ui::log_macos::tick(c.mtm);
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

fn read_text_field(field: *mut NSTextField) -> Option<String> {
    let ptr: *mut objc2_foundation::NSString = unsafe { msg_send![field, stringValue] };
    if ptr.is_null() { return None; }
    let ns = unsafe { &*ptr as &objc2_foundation::NSString };
    Some(ns.to_string())
}

fn make_label(panel: *mut NSPanel, x: f64, y: f64, w: f64, text: &NSString) {
    let content: *mut NSView = unsafe { msg_send![panel, contentView] };
    let lbl: *mut NSTextField = unsafe {
        let l: *mut NSTextField = msg_send![NSTextField::class(), alloc];
        msg_send![l, initWithFrame: NSRect {
            origin: NSPoint { x, y },
            size: objc2_foundation::NSSize { width: w, height: 20.0 }
        }]
    };
    let _: () = unsafe { msg_send![lbl, setStringValue: text] };
    let _: () = unsafe { msg_send![lbl, setBezeled: false] };
    let _: () = unsafe { msg_send![lbl, setDrawsBackground: false] };
    let _: () = unsafe { msg_send![lbl, setEditable: false] };
    let _: () = unsafe { msg_send![content, addSubview: lbl] };
}

fn make_field(panel: *mut NSPanel, x: f64, y: f64, w: f64, h: f64) -> *mut NSTextField {
    let content: *mut NSView = unsafe { msg_send![panel, contentView] };
    let f: *mut NSTextField = unsafe {
        let f_: *mut NSTextField = msg_send![NSTextField::class(), alloc];
        msg_send![f_, initWithFrame: NSRect {
            origin: NSPoint { x, y },
            size: objc2_foundation::NSSize { width: w, height: h }
        }]
    };
    let _: () = unsafe { msg_send![f, setBezeled: true] };
    let _: () = unsafe { msg_send![f, setDrawsBackground: true] };
    let _: () = unsafe { msg_send![f, setEditable: true] };
    let _: () = unsafe { msg_send![content, addSubview: f] };
    f
}

fn make_button(panel: *mut NSPanel, x: f64, y: f64, w: f64, h: f64, title: &NSString, target: &MenuHandler, action: Sel) -> *mut NSButton {
    let content: *mut NSView = unsafe { msg_send![panel, contentView] };
    let btn: *mut NSButton = unsafe { msg_send![NSButton::class(), alloc] };
    let _: () = unsafe { msg_send![btn, initWithFrame: NSRect {
        origin: NSPoint { x, y },
        size: objc2_foundation::NSSize { width: w, height: h }
    }] };
    let _: () = unsafe { msg_send![btn, setTitle: title] };
    let _: () = unsafe { msg_send![btn, setTarget: target] };
    let _: () = unsafe { msg_send![btn, setAction: action] };
    let _: () = unsafe { msg_send![content, addSubview: btn] };
    btn
}

fn show_input_dialog(
    title: &NSString,
    label1: &NSString,
    label2: &NSString,
    add_title: &NSString,
    cancel_title: &NSString,
    is_hall: bool,
) {
    let mtm = match MainThreadMarker::new() { Some(m) => m, None => return };
    log::info!("Dialog: создаю \"{}\"", title.to_string());

    let w: f64 = 400.0;
    let h: f64 = if label2.len() > 0 { 200.0 } else { 160.0 };
    let rect = NSRect {
        origin: NSPoint { x: 300.0, y: 300.0 },
        size: objc2_foundation::NSSize { width: w, height: h },
    };
    let mask: u64 = (1 << 0) | (1 << 1); // titled | closable (без nonactivating)

    let panel: *mut NSPanel = unsafe {
        let p: *mut NSPanel = msg_send![NSPanel::class(), alloc];
        msg_send![p, initWithContentRect: rect styleMask: mask backing: 2 defer: 0]
    };
    if panel.is_null() { log::warn!("Dialog: panel nil"); return; }
    let _: () = unsafe { msg_send![panel, setTitle: title] };
    let _: () = unsafe { msg_send![panel, setFloatingPanel: true] };
    let _: () = unsafe { msg_send![panel, setReleasedWhenClosed: false] };
    let _: () = unsafe { msg_send![panel, setLevel: 3i64] }; // NSFloatingWindowLevel

    make_label(panel, 16.0, h - 48.0, 140.0, label1);
    let field1 = make_field(panel, 16.0, h - 72.0, 360.0, 22.0);
    let _: () = unsafe { msg_send![panel, makeFirstResponder: field1] };

    let field2: *mut NSTextField;
    if label2.len() > 0 {
        make_label(panel, 16.0, h - 100.0, 140.0, label2);
        field2 = make_field(panel, 16.0, h - 124.0, 360.0, 22.0);
    } else {
        field2 = field1;
    }

    // Tab-переход: field1 → field2 → field1
    let _: () = unsafe { msg_send![field1, setNextKeyView: field2] };
    let _: () = unsafe { msg_send![field2, setNextKeyView: field1] };

    let handler = DLG_HANDLER.lock().unwrap().as_ref().copied()
        .map(|s| unsafe { &*(s.0 as *const MenuHandler) });
    if let Some(h) = handler {
        let add_btn = make_button(panel, 16.0, 10.0, 100.0, 28.0, add_title, h, sel!(handleDlgAdd:));
        let _: () = unsafe { msg_send![add_btn, setKeyEquivalent: &*ns_string("\r")] };
        let _ = make_button(panel, 126.0, 10.0, 100.0, 28.0, cancel_title, h, sel!(handleDlgCancel:));
    }

    DLG_PANEL.with_borrow_mut(|p| *p = Some(panel));
    DLG_FIELD1.with_borrow_mut(|f| *f = Some(field1));
    DLG_FIELD2.with_borrow_mut(|f| *f = if is_hall { Some(field1) } else { Some(field2) });
    DLG_IS_HALL.with_borrow_mut(|h| *h = is_hall);

    let _: () = unsafe { msg_send![panel, makeKeyAndOrderFront: std::ptr::null::<NSObject>()] };
    log::info!("Dialog: показана");
}

fn ns_string(s: &str) -> Retained<NSString> {
    NSString::from_str(s)
}

fn build_menu(handler: &MenuHandler, mtm: MainThreadMarker) -> (Retained<NSMenu>, Retained<NSMenuItem>, Retained<NSMenuItem>) {
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

    // Настройки
    let settings_item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm), &ns_string(&crate::lang::t("tray.menu.settings")),
            Some(sel!(handleSettings:)), &ns_string(""),
        )
    };
    let _: () = unsafe { msg_send![&settings_item, setTarget: handler] };
    menu.addItem(&settings_item);

    // Показать лог
    let log_item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm), &ns_string(&crate::lang::t("tray.menu.show_log")),
            Some(sel!(handleShowLog:)), &ns_string(""),
        )
    };
    let _: () = unsafe { msg_send![&log_item, setTarget: handler] };
    menu.addItem(&log_item);
    menu.addItem(&NSMenuItem::separatorItem(mtm));

    // Добавить слово
    let add_word = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm), &ns_string(&crate::lang::t("tray.menu.add_word")),
            Some(sel!(handleAddWord:)), &ns_string(""),
        )
    };
    let _: () = unsafe { msg_send![&add_word, setTarget: handler] };
    menu.addItem(&add_word);

    // Редактировать словарь
    let edit_dict = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm), &ns_string(&crate::lang::t("tray.menu.edit_dict")),
            Some(sel!(handleEditDict:)), &ns_string(""),
        )
    };
    let _: () = unsafe { msg_send![&edit_dict, setTarget: handler] };
    menu.addItem(&edit_dict);
    menu.addItem(&NSMenuItem::separatorItem(mtm));

    // Добавить галлюцинацию
    let add_hall = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm), &ns_string(&crate::lang::t("tray.menu.add_hall")),
            Some(sel!(handleAddHall:)), &ns_string(""),
        )
    };
    let _: () = unsafe { msg_send![&add_hall, setTarget: handler] };
    menu.addItem(&add_hall);

    // Редактировать галлюцинации
    let edit_hall = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm), &ns_string(&crate::lang::t("tray.menu.edit_hall")),
            Some(sel!(handleEditHall:)), &ns_string(""),
        )
    };
    let _: () = unsafe { msg_send![&edit_hall, setTarget: handler] };
    menu.addItem(&edit_hall);
    menu.addItem(&NSMenuItem::separatorItem(mtm));

    // VAD
    let vad_item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm), &ns_string(&crate::lang::t("tray.menu.auto_stop")),
            Some(sel!(handleVadToggle:)), &ns_string(""),
        )
    };
    let _: () = unsafe { msg_send![&vad_item, setTarget: handler] };
    menu.addItem(&vad_item);

    // Wake Word
    let wake_item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm), &ns_string(&crate::lang::t("tray.menu.voice_activation")),
            Some(sel!(handleWakeToggle:)), &ns_string(""),
        )
    };
    let _: () = unsafe { msg_send![&wake_item, setTarget: handler] };
    menu.addItem(&wake_item);

    // Math Mode
    let math_item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm), &ns_string(&crate::lang::t("tray.menu.math_mode")),
            Some(sel!(handleMathMode:)), &ns_string(""),
        )
    };
    let _: () = unsafe { msg_send![&math_item, setTarget: handler] };
    menu.addItem(&math_item);
    menu.addItem(&NSMenuItem::separatorItem(mtm));

    // Выход
    let quit_item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm), &ns_string(&crate::lang::t("tray.menu.quit")),
            Some(sel!(handleQuit:)), &ns_string(""),
        )
    };
    let _: () = unsafe { msg_send![&quit_item, setTarget: handler] };
    menu.addItem(&quit_item);

    (menu, vad_item, wake_item)
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
    // Резервный путь — напрямую на NSStatusItem
    // Работает до того, как button стал доступен
    let _: () = unsafe { msg_send![item, setImage: img] };
}

pub fn run_tray_main() {
    let mtm = MainThreadMarker::new().expect("run_tray_main must be called from main thread");

    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    let handler = MenuHandler::new(mtm);
    *DLG_HANDLER.lock().unwrap() = Some(SendPtr(&*handler as *const _ as *mut _));
    let (menu, vad_item, wake_item) = build_menu(&handler, mtm);

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
            vad_item,
            wake_item,
            mtm,
            blink: false,
            start_time: std::time::Instant::now(),
        });
    });

    // NSTimer fires tick: every 0.3s on the main run loop
    let _: Retained<NSTimer> = unsafe {
        msg_send![NSTimer::class(), scheduledTimerWithTimeInterval: 0.3, target: &*handler, selector: sel!(tick:), userInfo: None::<&NSObject>, repeats: true]
    };

    app.finishLaunching();
    app.run();
}
