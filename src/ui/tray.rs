use crate::app::AppCommand;
use crossbeam_channel::Sender;
use std::sync::Mutex;

const WM_APP: u32 = 0x8000;
const WM_TRAYICON: u32 = WM_APP + 1;
const WM_RBUTTONUP: u32 = 0x0205;
const WM_COMMAND: u32 = 0x0111;
const WM_DESTROY: u32 = 0x0002;
const WM_NULL: u32 = 0x0000;

const CMD_SETTINGS: u32 = 1000;
const CMD_CONSOLE: u32 = 1004;
const CMD_VAD: u32 = 1001;
const CMD_MATH: u32 = 1002;
const CMD_QUIT: u32 = 1003;

unsafe extern "system" {
    fn RegisterClassW(wc: *const WNDCLASSW) -> u16;
    fn CreateWindowExW(
        dwExStyle: u32, lpClassName: *const u16, lpWindowName: *const u16,
        dwStyle: u32, x: i32, y: i32, nWidth: i32, nHeight: i32,
        hWndParent: *mut std::ffi::c_void, hMenu: *mut std::ffi::c_void,
        hInstance: *mut std::ffi::c_void, lpParam: *mut std::ffi::c_void,
    ) -> *mut std::ffi::c_void;
    fn DefWindowProcW(hWnd: *mut std::ffi::c_void, msg: u32, wParam: usize, lParam: isize) -> isize;
    fn GetMessageW(lpMsg: *mut MSG, hWnd: *mut std::ffi::c_void, wMsgFilterMin: u32, wMsgFilterMax: u32) -> i16;
    fn TranslateMessage(lpMsg: *const MSG) -> i32;
    fn DispatchMessageW(lpMsg: *const MSG) -> isize;
    fn PostQuitMessage(nExitCode: i32);
    fn GetCursorPos(lpPoint: *mut POINT) -> i32;
    fn SetForegroundWindow(hWnd: isize) -> i32;
    fn PostMessageW(hWnd: *mut std::ffi::c_void, msg: u32, wParam: usize, lParam: isize) -> i32;
    fn CreatePopupMenu() -> *mut std::ffi::c_void;
    fn AppendMenuW(hMenu: *mut std::ffi::c_void, uFlags: u32, uIdNewItem: usize, lpNewItem: *const u16) -> i32;
    fn TrackPopupMenu(
        hMenu: *mut std::ffi::c_void, uFlags: u32, x: i32, y: i32,
        nReserved: i32, hWnd: *mut std::ffi::c_void, prcRect: *mut std::ffi::c_void,
    ) -> i32;
    fn DestroyMenu(hMenu: *mut std::ffi::c_void) -> i32;
    fn Shell_NotifyIconW(dwMessage: u32, lpData: *const NOTIFYICONDATAW) -> i32;
    fn GetModuleHandleW(lpModuleName: *const u16) -> *mut std::ffi::c_void;
    fn CreateIcon(
        hInstance: *mut std::ffi::c_void, nWidth: i32, nHeight: i32,
        cPlanes: u8, cBitsPerPixel: u8, lpbANDbits: *const u8, lpbXORbits: *const u8,
    ) -> *mut std::ffi::c_void;
}

#[allow(non_snake_case)]
#[repr(C)]
struct WNDCLASSW {
    style: u32,
    lpfnWndProc: Option<unsafe extern "system" fn(*mut std::ffi::c_void, u32, usize, isize) -> isize>,
    cbClsExtra: i32,
    cbWndExtra: i32,
    hInstance: *mut std::ffi::c_void,
    hIcon: *mut std::ffi::c_void,
    hCursor: *mut std::ffi::c_void,
    hbrBackground: *mut std::ffi::c_void,
    lpszMenuName: *const u16,
    lpszClassName: *const u16,
}

#[allow(non_snake_case)]
#[repr(C)]
struct MSG {
    hwnd: *mut std::ffi::c_void,
    message: u32,
    wParam: usize,
    lParam: isize,
    time: u32,
    pt: POINT,
}

#[allow(non_snake_case)]
#[repr(C)]
struct POINT {
    x: i32,
    y: i32,
}

#[allow(non_snake_case)]
#[repr(C)]
struct NOTIFYICONDATAW {
    cbSize: u32,
    hWnd: *mut std::ffi::c_void,
    uID: u32,
    uFlags: u32,
    uCallbackMessage: u32,
    hIcon: *mut std::ffi::c_void,
    szTip: [u16; 128],
    dwState: u32,
    dwStateMask: u32,
    szInfo: [u16; 256],
    #[allow(dead_code)]
    uVersion: u32,
    #[allow(dead_code)]
    szInfoTitle: [u16; 64],
    #[allow(dead_code)]
    dwInfoFlags: u32,
    guidItem: [u8; 16],
    #[allow(dead_code)]
    hBalloonIcon: *mut std::ffi::c_void,
}

const NIF_MESSAGE: u32 = 0x00000001;
const NIF_ICON: u32 = 0x00000002;
const NIF_TIP: u32 = 0x00000004;
const NIF_GUID: u32 = 0x00000020;
const NIM_ADD: u32 = 0x00000000;
const NIM_DELETE: u32 = 0x00000002;
const MF_STRING: u32 = 0x00000000;
const MF_SEPARATOR: u32 = 0x00000800;
const MF_GRAYED: u32 = 0x00000001;
const TPM_RIGHTBUTTON: u32 = 0x00000002;
const TPM_BOTTOMALIGN: u32 = 0x00000020;

static TRAY_TX: Mutex<Option<Sender<AppCommand>>> = Mutex::new(None);

pub struct TrayManager;

impl TrayManager {
    pub fn new(cmd_tx: Sender<AppCommand>, _recording: std::sync::Arc<std::sync::atomic::AtomicBool>) -> Self {
        *TRAY_TX.lock().unwrap() = Some(cmd_tx);
        Self
    }

    pub fn run(&self) {
        unsafe {
            let instance = GetModuleHandleW(std::ptr::null());
            let class_name: Vec<u16> = "VoxMiMTray\0".encode_utf16().collect();

            let wc = WNDCLASSW {
                style: 0,
                lpfnWndProc: Some(wnd_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: instance,
                hIcon: std::ptr::null_mut(),
                hCursor: std::ptr::null_mut(),
                hbrBackground: std::ptr::null_mut(),
                lpszMenuName: std::ptr::null(),
                lpszClassName: class_name.as_ptr(),
            };

            RegisterClassW(&wc);

            let hwnd = CreateWindowExW(
                0, class_name.as_ptr(), class_name.as_ptr(),
                0, 0, 0, 0, 0,
                std::ptr::null_mut(), std::ptr::null_mut(),
                instance, std::ptr::null_mut(),
            );

            let hicon = load_hicon("blue-voice.png");

            let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = hwnd;
            nid.uID = 1;
            nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_GUID;
            nid.uCallbackMessage = WM_TRAYICON;
            nid.hIcon = hicon;
            nid.guidItem = [0x21, 0x43, 0x65, 0x87, 0x12, 0x34, 0x56, 0x78, 0xAB, 0xCD, 0xEF, 0x01, 0x23, 0x45, 0x67, 0x89];

            let tip: Vec<u16> = "VoxMiM\0".encode_utf16().collect();
            for (i, &c) in tip.iter().enumerate() {
                if i < 127 { nid.szTip[i] = c; }
            }

            Shell_NotifyIconW(NIM_ADD, &nid);
            log::info!("Трей-иконка запущена");

            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) != 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            Shell_NotifyIconW(NIM_DELETE, &nid);
        }
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: *mut std::ffi::c_void, msg: u32, wparam: usize, lparam: isize,
) -> isize {
    match msg {
        WM_TRAYICON => {
            let event = (lparam & 0xFFFF) as u32;
            if event == WM_RBUTTONUP {
                unsafe { show_menu(hwnd); }
            }
            return 0;
        }
        WM_COMMAND => {
            let id = (wparam & 0xFFFF) as u32;
            if let Some(ref tx) = *TRAY_TX.lock().unwrap() {
                match id {
                    CMD_SETTINGS => { let _ = tx.send(AppCommand::OpenSettings); }
                    CMD_CONSOLE => {
                        let hwnd = crate::CONSOLE_HWND.load(std::sync::atomic::Ordering::SeqCst);
                        if hwnd != 0 {
                            unsafe {
                                unsafe extern "system" {
                                    fn IsWindowVisible(hWnd: isize) -> i32;
                                    fn ShowWindow(hWnd: isize, nCmdShow: i32) -> i32;
                                }
                                if IsWindowVisible(hwnd) != 0 {
                                    ShowWindow(hwnd, 0); // SW_HIDE
                                } else {
                                    ShowWindow(hwnd, 5); // SW_SHOW
                                }
                            }
                        }
                    }
                    CMD_VAD => { let _ = tx.send(AppCommand::ToggleVad(true)); }
                    CMD_MATH => { let _ = tx.send(AppCommand::ToggleMathMode(true)); }
                    CMD_QUIT => { let _ = tx.send(AppCommand::Quit); }
                    _ => {}
                }
            }
            return 0;
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0); }
            return 0;
        }
        _ => {}
    }
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

unsafe fn show_menu(hwnd: *mut std::ffi::c_void) {
    let menu = unsafe { CreatePopupMenu() };
    if menu.is_null() { return; }

    let ver = format!("VoxMiM v{}\0", env!("CARGO_PKG_VERSION"));
    let ver_w: Vec<u16> = ver.encode_utf16().collect();
    unsafe { AppendMenuW(menu, MF_STRING | MF_GRAYED, 0, ver_w.as_ptr()); }

    unsafe { AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null()); }

    let set_w: Vec<u16> = "Настройки\0".encode_utf16().collect();
    unsafe { AppendMenuW(menu, MF_STRING, CMD_SETTINGS as usize, set_w.as_ptr()); }

    unsafe { AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null()); }

    let con_visible = unsafe {
        unsafe extern "system" {
            fn IsWindowVisible(hWnd: isize) -> i32;
        }
        let hwnd = crate::CONSOLE_HWND.load(std::sync::atomic::Ordering::SeqCst);
        hwnd != 0 && IsWindowVisible(hwnd) != 0
    };
    let con_label = if con_visible { "Скрыть окно" } else { "Показать окно" };
    let con_w: Vec<u16> = format!("{con_label}\0").encode_utf16().collect();
    unsafe { AppendMenuW(menu, MF_STRING, CMD_CONSOLE as usize, con_w.as_ptr()); }

    unsafe { AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null()); }

    let vad_w: Vec<u16> = "VAD\0".encode_utf16().collect();
    unsafe { AppendMenuW(menu, MF_STRING, CMD_VAD as usize, vad_w.as_ptr()); }

    let math_w: Vec<u16> = "Math Mode\0".encode_utf16().collect();
    unsafe { AppendMenuW(menu, MF_STRING, CMD_MATH as usize, math_w.as_ptr()); }

    unsafe { AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null()); }

    let quit_w: Vec<u16> = "Выход\0".encode_utf16().collect();
    unsafe { AppendMenuW(menu, MF_STRING, CMD_QUIT as usize, quit_w.as_ptr()); }

    let mut pt: POINT = unsafe { std::mem::zeroed() };
    unsafe { GetCursorPos(&mut pt); }
    unsafe { SetForegroundWindow(hwnd as isize); }

    unsafe {
        TrackPopupMenu(menu, TPM_RIGHTBUTTON | TPM_BOTTOMALIGN, pt.x, pt.y, 0, hwnd, std::ptr::null_mut());
        PostMessageW(hwnd, WM_NULL, 0, 0);
        DestroyMenu(menu);
    }
}

fn load_hicon(name: &str) -> *mut std::ffi::c_void {
    let paths = [
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("assets").join(name))),
        Some(std::path::PathBuf::from("assets").join(name)),
    ];

    for path in paths.into_iter().flatten() {
        if path.exists() {
            if let Ok(data) = std::fs::read(&path) {
                if let Ok(img) = image::load_from_memory(&data) {
                    let rgba = img.to_rgba8();
                    let (w, h) = rgba.dimensions();
                    let pixels = rgba.into_raw();
                    let hicon = create_hicon(&pixels, w, h);
                    if !hicon.is_null() {
                        return hicon;
                    }
                }
            }
        }
    }

    let size = 32;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let cx = size as i32 / 2;
            let cy = size as i32 / 2;
            let dx = (x as i32 - cx).abs();
            let dy = (y as i32 - cy).abs();
            if dx * dx + dy * dy < (size as i32 / 3).pow(2) || y as i32 > cy + size as i32 / 4 && dx < size as i32 / 6 {
                rgba.extend_from_slice(&[64, 160, 255, 255]);
            } else {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    create_hicon(&rgba, size, size)
}

fn create_hicon(rgba: &[u8], width: u32, height: u32) -> *mut std::ffi::c_void {
    unsafe {
        let stride = ((width * 32 + 31) / 32) * 4;
        let and_h = ((height + 31) / 32) as usize;
        let and_size = and_h * 4 * (width as usize / 32 + 1);
        let xor_size = (stride * height) as usize;

        let mut xor_bits = vec![0u8; xor_size];
        let mut and_bits = vec![0u8; and_size];

        for y in 0..height {
            for x in 0..width {
                let si = ((y * width + x) * 4) as usize;
                let di = (y * stride + x * 4) as usize;
                let a = rgba[si + 3];
                xor_bits[di] = rgba[si + 2];
                xor_bits[di + 1] = rgba[si + 1];
                xor_bits[di + 2] = rgba[si];
                xor_bits[di + 3] = a;
                if a < 128 {
                    let bi = (y * ((width + 31) / 32) * 4 + x / 32) as usize;
                    if bi < and_bits.len() {
                        and_bits[bi] |= 0x80u8.wrapping_shr(x % 32);
                    }
                }
            }
        }

        let icon = CreateIcon(
            std::ptr::null_mut(),
            width as i32,
            height as i32,
            1, 32,
            and_bits.as_ptr(),
            xor_bits.as_ptr(),
        );
        if icon.is_null() {
            log::warn!("CreateIcon failed");
        }
        icon
    }
}
