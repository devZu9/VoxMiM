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

static HOOK_REC: AtomicBool = AtomicBool::new(false);
static HOOK_TX: Mutex<Option<Sender<AppCommand>>> = Mutex::new(None);
static VAD_ENABLED: AtomicBool = AtomicBool::new(false);
static VAD_KEY_LOCK: AtomicBool = AtomicBool::new(false);

pub fn reset_recording_state() {
    HOOK_REC.store(false, Ordering::SeqCst);
    VAD_KEY_LOCK.store(false, Ordering::SeqCst);
}

pub fn set_vad_enabled(enabled: bool) {
    VAD_ENABLED.store(enabled, Ordering::SeqCst);
}

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

    #[cfg(target_os = "macos")]
    fn install_kbd_hook(tx: Sender<AppCommand>) -> Self {
        use core_foundation::runloop::{CFRunLoop, kCFRunLoopDefaultMode};
        use core_graphics::event::{
            CGEventTap, CGEventTapLocation, CGEventTapOptions,
            CGEventTapPlacement, CGEventTapProxy, CGEventType, EventField,
        };

        *HOOK_TX.lock().unwrap() = Some(tx);
        HOOK_REC.store(false, Ordering::SeqCst);

        let handle = std::thread::Builder::new()
            .name("hotkey".into())
            .spawn(move || {
                let cb_tx = {
                    let guard = HOOK_TX.lock().unwrap();
                    guard.clone().unwrap()
                };

                let tap = match CGEventTap::new(
                    CGEventTapLocation::HID,
                    CGEventTapPlacement::HeadInsertEventTap,
                    CGEventTapOptions::Default,
                    vec![CGEventType::KeyDown, CGEventType::KeyUp],
                    move |_proxy: CGEventTapProxy, _etype: CGEventType, event: &core_graphics::event::CGEvent| {
                        let flags = event.get_flags();
                        if !flags.contains(core_graphics::event::CGEventFlags::CGEventFlagCommand) {
                            return Some(event.clone());
                        }

                        let keycode = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;
                        if keycode != core_graphics::event::KeyCode::ESCAPE {
                            return Some(event.clone());
                        }

                        let is_down = matches!(event.get_type(), CGEventType::KeyDown);
                        let vad = VAD_ENABLED.load(Ordering::SeqCst);

                        if is_down {
                            if vad {
                                if VAD_KEY_LOCK.load(Ordering::SeqCst) {
                                    return Some(event.clone());
                                }
                                VAD_KEY_LOCK.store(true, Ordering::SeqCst);
                                let _ = cb_tx.send(AppCommand::StartRecording);
                            } else if !HOOK_REC.load(Ordering::SeqCst) {
                                HOOK_REC.store(true, Ordering::SeqCst);
                                let _ = cb_tx.send(AppCommand::StartRecording);
                            }
                        } else {
                            if vad {
                                VAD_KEY_LOCK.store(false, Ordering::SeqCst);
                            } else if HOOK_REC.load(Ordering::SeqCst) {
                                HOOK_REC.store(false, Ordering::SeqCst);
                                let _ = cb_tx.send(AppCommand::StopRecording);
                            }
                        }

                        Some(event.clone())
                    },
                ) {
                    Ok(t) => t,
                    Err(()) => {
                        log::error!("CGEventTap не создан. Нужны разрешения: 1) Системные настройки → Конфиденциальность → Универсальный доступ → Terminal, 2) macOS 15+: → Мониторинг ввода → Terminal. После добавления перезапустите VoxMiM.");
                        return;
                    }
                };

                let current = CFRunLoop::get_current();
                let loop_source = match tap.mach_port.create_runloop_source(0) {
                    Ok(s) => s,
                    Err(()) => {
                        log::error!("RunLoopSource: ошибка создания");
                        return;
                    }
                };
                current.add_source(&loop_source, unsafe { kCFRunLoopDefaultMode });
                tap.enable();
                log::info!("CGEventTap установлен (Cmd+Esc)");

                CFRunLoop::run_current();
                log::info!("CGEventTap снят");
            })
            .ok();

        Self { _hook: handle }
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
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
    let vad = VAD_ENABLED.load(Ordering::SeqCst);

    if is_down {
        let ctrl_held = unsafe { GetAsyncKeyState(VK_CONTROL as i32) as u32 } & 0x8000 != 0;
        if !ctrl_held {
            return result;
        }

        if vad {
            if VAD_KEY_LOCK.load(Ordering::SeqCst) {
                return result; // автоповтор Insert — игнорируем
            }
            VAD_KEY_LOCK.store(true, Ordering::SeqCst);
            // VAD tap: Insert всегда шлёт StartRecording.
            // on_start() сам решает: начать запись или force_stop (если уже запись).
            if let Some(ref tx) = *HOOK_TX.lock().unwrap() {
                let _ = tx.send(AppCommand::StartRecording);
            }
        } else if !HOOK_REC.load(Ordering::SeqCst) {
            // Hold-режим: Insert зажат → запись
            HOOK_REC.store(true, Ordering::SeqCst);
            if let Some(ref tx) = *HOOK_TX.lock().unwrap() {
                let _ = tx.send(AppCommand::StartRecording);
            }
        }
    } else if !is_down && vad {
        VAD_KEY_LOCK.store(false, Ordering::SeqCst);
    } else if !is_down && HOOK_REC.load(Ordering::SeqCst) && !vad {
        // Hold-режим: отпустили Insert → стоп
        HOOK_REC.store(false, Ordering::SeqCst);
        if let Some(ref tx) = *HOOK_TX.lock().unwrap() {
            let _ = tx.send(AppCommand::StopRecording);
        }
    }

    result
}
