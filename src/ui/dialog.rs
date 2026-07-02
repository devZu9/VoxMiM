use crate::app::AppCommand;
use std::sync::atomic::{AtomicBool, Ordering};

static DLG_ACTIVE: AtomicBool = AtomicBool::new(false);

const IDC_EDIT_WRONG: usize = 201;
const IDC_EDIT_CORRECT: usize = 202;
const IDC_BTN_ADD: usize = 203;
const IDC_BTN_CANCEL: usize = 204;

unsafe extern "system" {
    fn RegisterClassW(wc: *const WNDCLASSW) -> u16;
    fn CreateWindowExW(
        dwExStyle: u32, lpClassName: *const u16, lpWindowName: *const u16,
        dwStyle: u32, x: i32, y: i32, nWidth: i32, nHeight: i32,
        hWndParent: *mut std::ffi::c_void, hMenu: *mut std::ffi::c_void,
        hInstance: *mut std::ffi::c_void, lpParam: *mut std::ffi::c_void,
    ) -> *mut std::ffi::c_void;
    fn DefWindowProcW(hWnd: *mut std::ffi::c_void, msg: u32, wParam: usize, lParam: isize) -> isize;
    fn DestroyWindow(hWnd: *mut std::ffi::c_void) -> i32;
    fn GetWindowTextLengthW(hWnd: *mut std::ffi::c_void) -> i32;
    fn GetWindowTextW(hWnd: *mut std::ffi::c_void, lpString: *mut u16, nMaxCount: i32) -> i32;
    fn GetDlgItem(hDlg: *mut std::ffi::c_void, nIDDlgItem: i32) -> *mut std::ffi::c_void;
    fn GetSystemMetrics(nIndex: i32) -> i32;
    fn ShowWindow(hWnd: *mut std::ffi::c_void, nCmdShow: i32) -> i32;
    fn SetFocus(hWnd: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    fn GetModuleHandleW(lpModuleName: *const u16) -> *mut std::ffi::c_void;
    fn PostMessageW(hWnd: *mut std::ffi::c_void, msg: u32, wParam: usize, lParam: isize) -> i32;
    fn EnableWindow(hWnd: *mut std::ffi::c_void, bEnable: i32) -> i32;
    fn GetParent(hWnd: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
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

unsafe fn get_edit_text(hwnd: *mut std::ffi::c_void) -> String {
    let len = unsafe { GetWindowTextLengthW(hwnd) };
    if len <= 0 {
        return String::new();
    }
    let mut buf = vec![0u16; (len + 1) as usize];
    unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), len + 1); }
    let actual = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..actual])
}

fn send_cmd_tx(cmd: AppCommand) {
    let guard = crate::ui::tray::tray_cmd_tx();
    if let Some(tx) = guard.as_ref() {
        let _ = tx.send(cmd);
    }
}

static WC_STATIC: [u16; 7] = [83, 84, 65, 84, 73, 67, 0];
static WC_EDIT: [u16; 5] = [69, 68, 73, 84, 0];
static WC_BUTTON: [u16; 7] = [66, 85, 84, 84, 79, 78, 0];

unsafe extern "system" fn dlg_wnd_proc(
    hwnd: *mut std::ffi::c_void, msg: u32, wparam: usize, lparam: isize,
) -> isize {
    const WM_CREATE: u32 = 0x0001;
    const WM_CLOSE: u32 = 0x0010;
    const WM_DESTROY: u32 = 0x0002;
    const WM_COMMAND: u32 = 0x0111;
    const WM_SETICON: u32 = 0x0080;

    match msg {
        WM_CREATE => unsafe {
            let instance = GetModuleHandleW(std::ptr::null());

            let edit_w = 320i32;
            let edit_h = 26i32;
            let left = 40i32;
            let label_w = 300i32;

            let lbl1: Vec<u16> = "Как распознано:\0".encode_utf16().collect();
            CreateWindowExW(0, WC_STATIC.as_ptr(), lbl1.as_ptr(),
                0x50000000, left, 20, label_w, 20, hwnd, std::ptr::null_mut(), instance, std::ptr::null_mut());

            let edit_wrong = CreateWindowExW(0x00000200, WC_EDIT.as_ptr(), std::ptr::null(),
                0x50010080, left, 44, edit_w, edit_h, hwnd,
                IDC_EDIT_WRONG as *mut std::ffi::c_void, instance, std::ptr::null_mut());
            SetFocus(edit_wrong);

            let lbl2: Vec<u16> = "Правильный вариант:\0".encode_utf16().collect();
            CreateWindowExW(0, WC_STATIC.as_ptr(), lbl2.as_ptr(),
                0x50000000, left, 92, label_w, 20, hwnd, std::ptr::null_mut(), instance, std::ptr::null_mut());

            CreateWindowExW(0x00000200, WC_EDIT.as_ptr(), std::ptr::null(),
                0x50010080, left, 116, edit_w, edit_h, hwnd,
                IDC_EDIT_CORRECT as *mut std::ffi::c_void, instance, std::ptr::null_mut());

            let add: Vec<u16> = "Добавить\0".encode_utf16().collect();
            CreateWindowExW(0, WC_BUTTON.as_ptr(), add.as_ptr(),
                0x50010000, left, 170, 100, 30, hwnd,
                IDC_BTN_ADD as *mut std::ffi::c_void, instance, std::ptr::null_mut());

            let cancel: Vec<u16> = "Отмена\0".encode_utf16().collect();
            CreateWindowExW(0, WC_BUTTON.as_ptr(), cancel.as_ptr(),
                0x50010000, left + 110, 170, 100, 30, hwnd,
                IDC_BTN_CANCEL as *mut std::ffi::c_void, instance, std::ptr::null_mut());

            let icon = crate::ui::tray::icon_from_bytes(include_bytes!("../../assets/blue-voice.png"));
            if !icon.is_null() {
                PostMessageW(hwnd, WM_SETICON, 0, icon as isize);
                PostMessageW(hwnd, WM_SETICON, 1, icon as isize);
            }

            0
        }
        WM_COMMAND => {
            let id = (wparam & 0xFFFF) as usize;
            match id {
                IDC_BTN_ADD => unsafe {
                    let wrong_h = GetDlgItem(hwnd, IDC_EDIT_WRONG as i32);
                    let corr_h = GetDlgItem(hwnd, IDC_EDIT_CORRECT as i32);
                    let wrong = get_edit_text(wrong_h);
                    let correct = get_edit_text(corr_h);
                    if !wrong.is_empty() && !correct.is_empty() {
                        send_cmd_tx(AppCommand::AddUserEntry { wrong, correct });
                    }
                    DestroyWindow(hwnd);
                }
                IDC_BTN_CANCEL => unsafe {
                    DestroyWindow(hwnd);
                }
                _ => {}
            }
            0
        }
        WM_CLOSE => unsafe {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => unsafe {
            DLG_ACTIVE.store(false, Ordering::SeqCst);
            let parent = GetParent(hwnd);
            if !parent.is_null() {
                EnableWindow(parent, 1);
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

pub fn show_add_word_dialog(parent_hwnd: *mut std::ffi::c_void, instance: *mut std::ffi::c_void) {
    if DLG_ACTIVE.swap(true, Ordering::SeqCst) {
        return;
    }

    unsafe {
        let class_name: Vec<u16> = "VoxMiMAddWordDlg\0".encode_utf16().collect();
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(dlg_wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: std::ptr::null_mut(),
            hCursor: std::ptr::null_mut(),
            hbrBackground: (5usize + 1) as *mut std::ffi::c_void,
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
        };

        RegisterClassW(&wc);

        let w = 400i32;
        let h = 260i32;
        let sw = GetSystemMetrics(0);
        let sh = GetSystemMetrics(1);
        let x = (sw - w) / 2;
        let y = (sh - h) / 2;

        let title: Vec<u16> = "Добавить пользовательское слово\0".encode_utf16().collect();

        let hwnd = CreateWindowExW(
            0, class_name.as_ptr(), title.as_ptr(),
            0x10CE0000, x, y, w, h,
            parent_hwnd, std::ptr::null_mut(), instance, std::ptr::null_mut(),
        );

        if hwnd.is_null() {
            DLG_ACTIVE.store(false, Ordering::SeqCst);
            return;
        }

        // Модальность: блокируем родительское окно трея
        EnableWindow(parent_hwnd, 0);

        ShowWindow(hwnd, 5);

        // Нет вложенного цикла! Трей-поток сам раздаёт сообщения всем окнам.
    }
}
