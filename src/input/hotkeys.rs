use crate::app::AppCommand;
use crate::config::TriggerButton;
use crossbeam_channel::Sender;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

const VK_INSERT: u32 = 0x2D;
const VK_CONTROL: u32 = 0x11;

const WH_KEYBOARD_LL: i32 = 13;
const HC_ACTION: i32 = 0;
const WM_KEYDOWN: u32 = 0x0100;
const WM_KEYUP: u32 = 0x0101;
const WM_SYSKEYDOWN: u32 = 0x0104;
const WM_SYSKEYUP: u32 = 0x0105;
const LLKHF_INJECTED: u32 = 0x00000010;

#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, SetWindowsHookExW, UnhookWindowsHookEx, MSG,
};

#[cfg(target_os = "windows")]
unsafe extern "system" {
    fn GetAsyncKeyState(vKey: i32) -> i16;
}

#[repr(C)]
#[cfg(target_os = "windows")]
#[allow(non_snake_case)]
struct KBDLLHOOKSTRUCT {
    vkCode: u32,
    scanCode: u32,
    flags: u32,
    time: u32,
    dwExtraInfo: usize,
}

#[cfg(target_os = "windows")]
static HOOK_REC: AtomicBool = AtomicBool::new(false);
#[cfg(target_os = "windows")]
static HOOK_TX: Mutex<Option<Sender<AppCommand>>> = Mutex::new(None);

pub struct HotkeyListener {
    _hook: Option<std::thread::JoinHandle<()>>,
}

impl HotkeyListener {
    pub fn new(tx: Sender<AppCommand>, button: TriggerButton) -> Self {
        if matches!(button, TriggerButton::Keyboard) {
            Self::install_kbd_hook(tx)
        } else {
            Self::install_mouse_hook(tx, button)
        }
    }

    #[cfg(target_os = "windows")]
    fn install_kbd_hook(tx: Sender<AppCommand>) -> Self {
        *HOOK_TX.lock().unwrap() = Some(tx);
        HOOK_REC.store(false, Ordering::SeqCst);

        let handle = std::thread::Builder::new()
            .name("hotkey".into())
            .spawn(move || unsafe {
                let hook = SetWindowsHookExW(
                    WH_KEYBOARD_LL,
                    Some(hook_proc),
                    std::ptr::null_mut(),
                    0,
                );

                if hook.is_null() {
                    log::error!("SetWindowsHookExW WH_KEYBOARD_LL failed");
                    return;
                }

                log::info!("Hook WH_KEYBOARD_LL установлен");

                let mut msg = MSG {
                    hwnd: std::ptr::null_mut(),
                    message: 0,
                    wParam: 0,
                    lParam: 0,
                    time: 0,
                    pt: std::mem::zeroed(),
                };

                while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) != 0 {
                    let _ = windows_sys::Win32::UI::WindowsAndMessaging::TranslateMessage(&msg);
                    let _ = windows_sys::Win32::UI::WindowsAndMessaging::DispatchMessageW(&msg);
                }

                UnhookWindowsHookEx(hook);
                log::info!("Hook снят");
            })
            .ok();

        Self { _hook: handle }
    }

    #[cfg(not(target_os = "windows"))]
    fn install_kbd_hook(tx: Sender<AppCommand>) -> Self {
        let _ = tx;
        Self { _hook: None }
    }

    fn install_mouse_hook(tx: Sender<AppCommand>, button: TriggerButton) -> Self {
        let trigger_code = match button {
            TriggerButton::Middle => rdev::Button::Middle,
            TriggerButton::Right => rdev::Button::Right,
            TriggerButton::Extra => rdev::Button::Unknown(0x05),
            _ => return Self { _hook: None },
        };

        let handle = std::thread::Builder::new()
            .name("hotkey".into())
            .spawn(move || {
                let pressed = Arc::new(AtomicBool::new(false));
                let p = pressed.clone();

                let callback = move |event: rdev::Event| {
                    match event.event_type {
                        rdev::EventType::ButtonPress(btn) if btn == trigger_code => {
                            if !p.load(Ordering::SeqCst) {
                                p.store(true, Ordering::SeqCst);
                                let _ = tx.send(AppCommand::StartRecording);
                            }
                        }
                        rdev::EventType::ButtonRelease(btn) if btn == trigger_code => {
                            if p.load(Ordering::SeqCst) {
                                p.store(false, Ordering::SeqCst);
                                let _ = tx.send(AppCommand::StopRecording);
                            }
                        }
                        _ => {}
                    }
                };

                if let Err(e) = rdev::listen(callback) {
                    log::error!("rdev: {e:?}");
                }
            })
            .ok();

        Self { _hook: handle }
    }
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn hook_proc(code: i32, wparam: usize, lparam: isize) -> isize {
    let result = unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) };
    if code != HC_ACTION {
        return result;
    }

    let msg = wparam as u32;
    if msg != WM_KEYDOWN && msg != WM_KEYUP
        && msg != WM_SYSKEYDOWN && msg != WM_SYSKEYUP
    {
        return result;
    }

    let kbd = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };

    // Игнорируем инжектированные события (SendInput и т.п.)
    if kbd.flags & LLKHF_INJECTED != 0 {
        return result;
    }

    let vk = kbd.vkCode;

    if vk != VK_INSERT {
        return result;
    }

    let is_down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;

    if is_down && !HOOK_REC.load(Ordering::SeqCst) {
        // Проверяем физическое состояние Ctrl напрямую у системы
        if unsafe { GetAsyncKeyState(VK_CONTROL as i32) as u32 } & 0x8000 != 0 {
            HOOK_REC.store(true, Ordering::SeqCst);
            if let Some(ref tx) = *HOOK_TX.lock().unwrap() {
                let _ = tx.send(AppCommand::StartRecording);
            }
        }
    } else if !is_down && HOOK_REC.load(Ordering::SeqCst) {
        HOOK_REC.store(false, Ordering::SeqCst);
        if let Some(ref tx) = *HOOK_TX.lock().unwrap() {
            let _ = tx.send(AppCommand::StopRecording);
        }
    }

    result
}
