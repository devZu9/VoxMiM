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
        fn IsClipboardFormatAvailable(format: u32) -> i32;
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
    const CF_BITMAP: u32 = 2;
    const CF_DIB: u32 = 8;
    const CF_HDROP: u32 = 15;
    const CF_DIBV5: u32 = 17;
    const GMEM_MOVEABLE: u32 = 0x0002;
    const VK_CONTROL: u16 = 0x11;
    const VK_V: u16 = 0x56;
    const VK_RETURN: u16 = 0x0D;
    const VK_SHIFT: u16 = 0x10;
    const INPUT_KEYBOARD: u32 = 1;
    const KEYEVENTF_KEYUP: u32 = 0x0002;
    const KEYEVENTF_UNICODE: u32 = 0x0004;

    /// Можно ли перезаписать буфер обмена на время вставки без потерь.
    /// Нельзя, если в нём картинка или файлы — их сохранить/вернуть нечем.
    /// Пустой буфер безопасен: после вставки его очистим обратно.
    pub fn clipboard_can_paste() -> bool {
        unsafe {
            !(IsClipboardFormatAvailable(CF_DIB) != 0
                || IsClipboardFormatAvailable(CF_DIBV5) != 0
                || IsClipboardFormatAvailable(CF_BITMAP) != 0
                || IsClipboardFormatAvailable(CF_HDROP) != 0)
        }
    }

    fn key_input(vk: u16, up: bool) -> INPUT {
        let ki = KEYBDINPUT {
            wVk: vk,
            wScan: 0,
            dwFlags: if up { KEYEVENTF_KEYUP } else { 0 },
            time: 0,
            dwExtraInfo: 0,
        };
        INPUT {
            type_: INPUT_KEYBOARD,
            u: INPUT_UNION {
                ki: ManuallyDrop::new(ki),
            },
        }
    }

    fn unicode_input(unit: u16, up: bool) -> INPUT {
        let mut flags = KEYEVENTF_UNICODE;
        if up {
            flags |= KEYEVENTF_KEYUP;
        }
        let ki = KEYBDINPUT {
            wVk: 0,
            wScan: unit,
            dwFlags: flags,
            time: 0,
            dwExtraInfo: 0,
        };
        INPUT {
            type_: INPUT_KEYBOARD,
            u: INPUT_UNION {
                ki: ManuallyDrop::new(ki),
            },
        }
    }

    /// Набрать текст как клавиатуру, не трогая буфер обмена.
    /// Перевод строки — Shift+Enter: в чатах это перенос, а не отправка.
    pub fn send_unicode_text(text: &str) {
        let mut events: Vec<INPUT> = Vec::with_capacity(text.chars().count() * 2);
        for ch in text.chars() {
            match ch {
                '\r' => {}
                '\n' => {
                    events.push(key_input(VK_SHIFT, false));
                    events.push(key_input(VK_RETURN, false));
                    events.push(key_input(VK_RETURN, true));
                    events.push(key_input(VK_SHIFT, true));
                }
                _ => {
                    let mut buf = [0u16; 2];
                    for unit in ch.encode_utf16(&mut buf) {
                        events.push(unicode_input(*unit, false));
                        events.push(unicode_input(*unit, true));
                    }
                }
            }
        }
        for chunk in events.chunks(64) {
            unsafe {
                SendInput(
                    chunk.len() as u32,
                    chunk.as_ptr(),
                    std::mem::size_of::<INPUT>() as i32,
                );
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }

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

    /// Очистить буфер обмена. OpenClipboard может быть занят другим
    /// приложением — пробуем несколько раз.
    pub fn clear_clipboard() -> Result<(), String> {
        for _ in 0..3 {
            unsafe {
                if OpenClipboard(0) != 0 {
                    EmptyClipboard();
                    CloseClipboard();
                    return Ok(());
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
        Err("буфер обмена занят другим приложением".to_string())
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

    /// Восстанавливает сохранённый буфер обмена.
    /// Пустой снимок означает «буфера не было» — возвращаем его в пустое состояние,
    /// иначе после вставки там остался бы наш текст.
    pub fn restore_clipboard(data: &[u16]) {
        if data.is_empty() {
            let _ = clear_clipboard();
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

/// Режим вставки: "auto" (по умолчанию), "clipboard" или "typing".
static INSERT_MODE: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

pub fn set_insert_mode(mode: &str) {
    *INSERT_MODE.lock().unwrap() = Some(mode.to_string());
}

fn current_insert_mode() -> String {
    INSERT_MODE
        .lock()
        .ok()
        .and_then(|m| m.clone())
        .unwrap_or_else(|| "auto".to_string())
}

/// Нужно ли вставлять через буфер обмена.
/// `auto` — буфер, если в нём нет картинки/файлов (пустой тоже годится);
/// иначе набор, чтобы не потерять скриншот.
pub fn use_clipboard(mode: &str, clipboard_can_paste: bool) -> bool {
    match mode {
        "typing" => false,
        "clipboard" => true,
        _ => clipboard_can_paste,
    }
}

pub struct TextInserter;

impl TextInserter {
    pub fn new() -> Self {
        Self
    }

    /// Положить текст в буфер обмена, ничего не вставляя в активное окно.
    pub fn set_clipboard_text(text: &str) -> Result<(), String> {        #[cfg(target_os = "windows")]
        {
            win32::copy_to_clipboard(text)
        }

        #[cfg(target_os = "macos")]
        {
            use std::io::Write;
            let mut child = std::process::Command::new("pbcopy")
                .stdin(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| format!("pbcopy: {e}"))?;
            child
                .stdin
                .take()
                .ok_or("pbcopy: нет stdin")?
                .write_all(text.as_bytes())
                .map_err(|e| format!("pbcopy: {e}"))?;
            let _ = child.wait();
            Ok(())
        }

        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            let _ = text;
            Err("буфер обмена не поддержан".to_string())
        }
    }

    /// Очистить буфер обмена (например, убрать скриншот).
    pub fn clear_clipboard() -> Result<(), String> {
        #[cfg(target_os = "windows")]
        {
            win32::clear_clipboard()
        }

        #[cfg(target_os = "macos")]
        {
            let mut child = std::process::Command::new("pbcopy")
                .stdin(std::process::Stdio::null())
                .spawn()
                .map_err(|e| format!("pbcopy: {e}"))?;
            let _ = child.wait();
            Ok(())
        }

        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            Err("буфер обмена не поддержан".to_string())
        }
    }

    pub fn insert_text(&self, text: &str) {
        #[cfg(target_os = "windows")]
        {
            let mode = current_insert_mode();
            if use_clipboard(&mode, win32::clipboard_can_paste()) {
                self.insert_via_clipboard(text);
            } else {
                log::info!("Вставка без буфера обмена (режим {mode})");
                self.insert_via_typing(text);
            }
        }

        #[cfg(target_os = "macos")]
        {
            // На macOS пока только буфер обмена
            self.insert_via_clipboard(text);
        }

        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            let _ = text;
        }
    }

    /// Набрать текст клавиатурой — буфер обмена не трогается.
    #[cfg(target_os = "windows")]
    fn insert_via_typing(&self, text: &str) {
        let hwnd = get_foreground_hwnd();
        if hwnd == 0 {
            log::warn!("Не удалось получить HWND активного окна");
            return;
        }
        win32::ensure_foreground(hwnd);
        let final_text = if win32::needs_prepend_space() {
            format!(" {text}")
        } else {
            text.to_string()
        };
        win32::send_unicode_text(&final_text);
        log::info!("Набрано без буфера обмена: {} символов", final_text.chars().count());
    }

    fn insert_via_clipboard(&self, text: &str) {
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

        #[cfg(target_os = "macos")]
        {
            use std::io::Write;
            use std::process::Command;
            use std::sync::Mutex;
            static INS_LOCK: Mutex<()> = Mutex::new(());
            let _lock = INS_LOCK.lock().unwrap();

            let saved = {
                let output = Command::new("pbpaste").output();
                output.ok().map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            };

            let mut child = Command::new("pbcopy")
                .stdin(std::process::Stdio::piped())
                .spawn();
            if let Ok(mut child) = child {
                let _ = child.stdin.take().unwrap().write_all(text.as_bytes());
                let _ = child.wait();
            }

            // Send Cmd+V via CGEventPost
            use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventFlags};
            use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

            let source = match CGEventSource::new(CGEventSourceStateID::CombinedSessionState) {
                Ok(s) => s,
                Err(()) => {
                    log::error!("CGEventSource failed");
                    return;
                }
            };

            let keycode = 0x09u16; // kVK_ANSI_V
            if let Ok(down) = CGEvent::new_keyboard_event(source.clone(), keycode, true) {
                down.set_flags(CGEventFlags::CGEventFlagCommand);
                down.post(CGEventTapLocation::HID);
            }
            if let Ok(up) = CGEvent::new_keyboard_event(source, keycode, false) {
                up.set_flags(CGEventFlags::CGEventFlagCommand);
                up.post(CGEventTapLocation::HID);
            }

            std::thread::sleep(std::time::Duration::from_millis(100));

            if let Some(prev) = saved.filter(|p| !p.is_empty()) {
                if let Ok(mut child) = Command::new("pbcopy")
                    .stdin(std::process::Stdio::piped())
                    .spawn()
                {
                    let _ = child.stdin.take().unwrap().write_all(prev.as_bytes());
                    let _ = child.wait();
                }
            } else {
                // Буфера не было — возвращаем пустое состояние
                let _ = TextInserter::clear_clipboard();
            }
        }

        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_use_clipboard_by_mode() {
        // Авто: буфер, если в нём нет картинки/файлов (текст и пустой буфер годятся)
        assert!(use_clipboard("auto", true));
        assert!(!use_clipboard("auto", false));
        // Явные режимы не зависят от содержимого буфера
        assert!(use_clipboard("clipboard", false));
        assert!(!use_clipboard("typing", true));
        // Неизвестное значение ведёт себя как авто
        assert!(use_clipboard("", true));
        assert!(!use_clipboard("", false));
    }
}
