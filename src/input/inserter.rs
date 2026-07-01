#[cfg(target_os = "windows")]
#[allow(non_snake_case, dead_code)]
mod win32 {
    use std::mem::ManuallyDrop;
    use std::ptr;

    unsafe extern "system" {
        fn OpenClipboard(hwnd: isize) -> i32;
        fn CloseClipboard() -> i32;
        fn EmptyClipboard() -> i32;
        fn SetClipboardData(uFormat: u32, hMem: isize) -> isize;
        fn GetClipboardData(uFormat: u32) -> isize;
        fn GlobalAlloc(uFlags: u32, dwBytes: usize) -> isize;
        fn GlobalLock(hMem: isize) -> *mut std::ffi::c_void;
        fn GlobalUnlock(hMem: isize) -> i32;
        fn GlobalFree(hMem: isize) -> isize;
        fn GetForegroundWindow() -> isize;
        fn SetForegroundWindow(hwnd: isize) -> i32;
        fn AttachThreadInput(idAttach: u32, idAttachTo: u32, fAttach: i32) -> i32;
        fn GetWindowThreadProcessId(hwnd: isize, lpdwProcessId: *mut u32) -> u32;
        fn SendInput(cInputs: u32, pInputs: *const INPUT, cbSize: i32) -> u32;
        fn SendMessageW(hwnd: isize, msg: u32, wparam: isize, lparam: isize) -> isize;
        fn GetGUIThreadInfo(idThread: u32, lpgui: *mut GUITHREADINFO) -> i32;
    }

    const WM_GETTEXT: u32 = 0x000D;
    const WM_GETTEXTLENGTH: u32 = 0x000E;
    const EM_GETSEL: u32 = 0x00B0;

    #[repr(C)]
    struct INPUT {
        type_: u32,
        u: INPUT_UNION,
    }

    #[repr(C)]
    union INPUT_UNION {
        ki: ManuallyDrop<KEYBDINPUT>,
        mi: ManuallyDrop<MOUSEINPUT>,
        hi: ManuallyDrop<HARDWAREINPUT>,
    }

    #[repr(C)]
    #[derive(Clone)]
    struct KEYBDINPUT {
        wVk: u16,
        wScan: u16,
        dwFlags: u32,
        time: u32,
        dwExtraInfo: isize,
    }

    #[repr(C)]
    #[derive(Clone)]
    struct MOUSEINPUT {
        dx: i32,
        dy: i32,
        mouseData: u32,
        dwFlags: u32,
        time: u32,
        dwExtraInfo: isize,
    }

    #[repr(C)]
    struct HARDWAREINPUT {
        uMsg: u32,
        wParamL: u16,
        wParamH: u16,
    }

    #[repr(C)]
    struct GUITHREADINFO {
        cbSize: u32,
        flags: u32,
        hwndActive: isize,
        hwndFocus: isize,
        hwndCapture: isize,
        hwndMenuOwner: isize,
        hwndMoveSize: isize,
        hwndCaret: isize,
        rcCaret: RECT,
    }

    #[repr(C)]
    struct RECT {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    const CF_UNICODETEXT: u32 = 13;
    const GMEM_MOVEABLE: u32 = 0x0002;
    const VK_CONTROL: u16 = 0x11;
    const VK_V: u16 = 0x56;
    const INPUT_KEYBOARD: u32 = 1;

    pub fn copy_to_clipboard(text: &str) -> Result<(), String> {
        let wide: Vec<u16> = text.encode_utf16().collect();
        let bytes = (wide.len() + 1) * 2;

        let hmem = unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes) };
        if hmem == 0 {
            return Err("GlobalAlloc failed".to_string());
        }

        let lock = unsafe { GlobalLock(hmem) };
        if lock.is_null() {
            unsafe { GlobalFree(hmem) };
            return Err("GlobalLock failed".to_string());
        }

        unsafe {
            ptr::copy_nonoverlapping(wide.as_ptr(), lock as *mut u16, wide.len());
            GlobalUnlock(hmem);
        }

        if unsafe { OpenClipboard(0) } == 0 {
            unsafe { GlobalFree(hmem) };
            return Err("OpenClipboard failed".to_string());
        }

        unsafe {
            EmptyClipboard();
            SetClipboardData(CF_UNICODETEXT, hmem);
            CloseClipboard();
        }

        Ok(())
    }

    fn send_key_down(vk: u16) {
        let ki = KEYBDINPUT {
            wVk: vk,
            wScan: 0,
            dwFlags: 0,
            time: 0,
            dwExtraInfo: 0,
        };
        let input = INPUT {
            type_: INPUT_KEYBOARD,
            u: INPUT_UNION {
                ki: ManuallyDrop::new(ki),
            },
        };
        unsafe {
            SendInput(1, &input, std::mem::size_of::<INPUT>() as i32);
        }
    }

    fn send_key_up(vk: u16) {
        let ki = KEYBDINPUT {
            wVk: vk,
            wScan: 0,
            dwFlags: 0x0002,
            time: 0,
            dwExtraInfo: 0,
        };
        let input = INPUT {
            type_: INPUT_KEYBOARD,
            u: INPUT_UNION {
                ki: ManuallyDrop::new(ki),
            },
        };
        unsafe {
            SendInput(1, &input, std::mem::size_of::<INPUT>() as i32);
        }
    }

    pub fn send_ctrl_v() {
        send_key_down(VK_CONTROL);
        send_key_down(VK_V);
        send_key_up(VK_V);
        send_key_up(VK_CONTROL);
    }

    pub fn send_backspace(count: usize) {
        for _ in 0..count {
            send_key_down(0x08);
            send_key_up(0x08);
        }
    }

    /// Сохраняет текущий текст из буфера обмена
    pub fn save_clipboard() -> Vec<u16> {
        unsafe {
            if OpenClipboard(0) == 0 {
                return Vec::new();
            }
            let hmem = GetClipboardData(CF_UNICODETEXT);
            if hmem == 0 {
                CloseClipboard();
                return Vec::new();
            }
            let lock = GlobalLock(hmem);
            if lock.is_null() {
                CloseClipboard();
                return Vec::new();
            }
            let mut data = Vec::new();
            let ptr = lock as *const u16;
            let mut i = 0;
            while *ptr.add(i) != 0 {
                data.push(*ptr.add(i));
                i += 1;
            }
            GlobalUnlock(hmem);
            CloseClipboard();
            data
        }
    }

    /// Восстанавливает сохранённый буфер обмена
    pub fn restore_clipboard(data: &[u16]) {
        if data.is_empty() {
            return;
        }
        let bytes = (data.len() + 1) * 2;
        unsafe {
            let hmem = GlobalAlloc(GMEM_MOVEABLE, bytes);
            if hmem == 0 {
                return;
            }
            let lock = GlobalLock(hmem);
            if lock.is_null() {
                GlobalFree(hmem);
                return;
            }
            ptr::copy_nonoverlapping(data.as_ptr(), lock as *mut u16, data.len());
            // завершающий null
            *(lock as *mut u16).add(data.len()) = 0;
            GlobalUnlock(hmem);

            OpenClipboard(0);
            EmptyClipboard();
            SetClipboardData(CF_UNICODETEXT, hmem);
            CloseClipboard();
        }
    }

    fn get_window_thread(hwnd: isize) -> u32 {
        unsafe { GetWindowThreadProcessId(hwnd, ptr::null_mut()) }
    }

    pub fn ensure_foreground(hwnd: isize) {
        unsafe {
            let cur = GetForegroundWindow();
            if cur == hwnd {
                return;
            }
            let tid = get_window_thread(hwnd);
            let cur_tid = get_window_thread(cur);
            if tid != cur_tid {
                AttachThreadInput(cur_tid, tid, 1);
                SetForegroundWindow(hwnd);
                AttachThreadInput(cur_tid, tid, 0);
            } else {
                SetForegroundWindow(hwnd);
            }
        }
    }

    /// Определяет, нужно ли вставить пробел слева от курсора
    pub fn needs_prepend_space() -> bool {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd == 0 {
                return false;
            }

            let mut gui = GUITHREADINFO {
                cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
                flags: 0,
                hwndActive: 0,
                hwndFocus: 0,
                hwndCapture: 0,
                hwndMenuOwner: 0,
                hwndMoveSize: 0,
                hwndCaret: 0,
                rcCaret: RECT { left: 0, top: 0, right: 0, bottom: 0 },
            };

            let tid = GetWindowThreadProcessId(hwnd, ptr::null_mut());
            if GetGUIThreadInfo(tid, &mut gui) == 0 || gui.hwndFocus == 0 {
                return false;
            }

            let focus = gui.hwndFocus;

            // Получаем длину текста в контроле
            let len = SendMessageW(focus, WM_GETTEXTLENGTH, 0, 0);
            if len <= 0 {
                return false;
            }

            // Получаем позицию курсора (EM_GETSEL)
            let mut sel_start: isize = 0;
            let mut sel_end: isize = 0;
            SendMessageW(
                focus,
                EM_GETSEL,
                (&mut sel_start) as *mut isize as isize,
                (&mut sel_end) as *mut isize as isize,
            );

            if sel_start <= 0 {
                return false;
            }

            // Получаем символ перед курсором
            let buf_len = (sel_start + 2) as usize;
            let mut buf: Vec<u16> = vec![0u16; buf_len];
            SendMessageW(focus, WM_GETTEXT, buf_len as isize, buf.as_mut_ptr() as isize);

            let char_before = buf.get((sel_start - 1) as usize).copied().unwrap_or(0);
            // Если предыдущий символ — пробел или начало строки → не нужно
            char_before != 0 && char_before != ' ' as u16 && char_before != '\t' as u16
        }
    }
}

pub struct TextInserter;

impl TextInserter {
    pub fn new() -> Self {
        Self
    }

    pub fn insert_text(&self, text: &str) {
        #[cfg(target_os = "windows")]
        {
            let saved = win32::save_clipboard();

            let hwnd = get_foreground_hwnd();
            if hwnd == 0 {
                log::warn!("Не удалось получить HWND активного окна");
                return;
            }

            win32::ensure_foreground(hwnd);

            let final_text = if win32::needs_prepend_space() {
                let mut s = String::with_capacity(text.len() + 1);
                s.push(' ');
                s.push_str(text);
                s
            } else {
                text.to_string()
            };

            if let Err(e) = win32::copy_to_clipboard(&final_text) {
                log::error!("Ошибка копирования в буфер: {e}");
                win32::restore_clipboard(&saved);
                return;
            }

            win32::send_ctrl_v();

            // Восстанавливаем старый буфер через небольшой таймаут
            std::thread::sleep(std::time::Duration::from_millis(100));
            win32::restore_clipboard(&saved);
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = text;
        }
    }
}

#[cfg(target_os = "windows")]
fn get_foreground_hwnd() -> isize {
    unsafe {
        unsafe extern "system" {
            fn GetForegroundWindow() -> isize;
        }
        GetForegroundWindow()
    }
}
