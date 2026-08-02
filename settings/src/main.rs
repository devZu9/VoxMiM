use fenestra::prelude::*;
use fenestra_kit::{checkbox, button, select, text_input, tabs, radio, icon_button, ControlSize, ButtonVariant};
use fenestra::{TextSize, TextAlign, Weight};
use std::collections::HashMap;
use std::ffi::c_void;

const EMBEDDED_RU: &str = include_str!("../../lang/ru.json");
const EMBEDDED_EN: &str = include_str!("../../lang/en.json");

#[derive(Clone)]
enum Msg {
    SetTab(usize),
    SetEngineMode(bool),
    SetDetMode(bool),
    ToggleGpu,
    BrowseFolder,
    BrowseWhisperBins,
    SelectTranscriberModel(usize),
    SelectDetectorModel(usize),
    ToggleWake,
    ToggleVad,
    SetSubtitleFormat(String),
    VadThresholdUp,
    VadThresholdDown,
    VadThresholdSet(String),
    VadThresholdReset,
    VadTimeoutUp,
    VadTimeoutDown,
    VadTimeoutSet(String),
    VadStartTimeoutUp,
    VadStartTimeoutDown,
    VadStartTimeoutSet(String),
    ToggleHall,
    ToggleUserDict,
    ToggleRep,
    TogglePunct,
    SetCmdMaxWords(String),
    ToggleMath,
    ToggleNoise,
    ToggleWarmup,
    ToggleShow,
    ToggleLog,
    ToggleTrail,
    SetLang(usize),
    ToggleDark,
    ToggleKeepWav,
    ToggleShowConsole,
    WhisperTimeoutUp,
    WhisperTimeoutDown,
    WhisperTimeoutSet(String),
    ReloadConfig,
    Debug,
    Close,
}

struct SettingsApp {
    cur_tab: usize,
    dark_mode: bool,
    engine_server: bool,
    det_server: bool,
    use_gpu: bool,
    model_dir: String,
    models: Vec<String>,
    transcriber_model_idx: usize,
    detector_model_idx: usize,
    wake_enable: bool,
    vad_enable: bool,
    vad_threshold: String,
    vad_timeout: String,
    vad_start_timeout: String,
    fix_hallucinations: bool,
    fix_user_dict: bool,
    fix_repetitions: bool,
    fix_punctuation: bool,
    cmd_max_words: String,
    math_mode: bool,
    noise_filter: bool,
    warmup: bool,
    show_result: bool,
    log_enable: bool,
    log_dir: String,
    trailing_space: bool,
    cur_lang: usize,
    keep_wav: bool,
    show_console: bool,
    whisper_timeout: String,
    whisper_bins: String,
    subtitle_format: String,
    locale: HashMap<String, String>,
    window_x: i32,
    window_y: i32,
    proxy: Option<Proxy<Msg>>,
    last_config_mtime: u64,
}

impl SettingsApp {
    fn t<'a>(&'a self, key: &'a str) -> &'a str {
        self.locale.get(key).map(|s| s.as_str()).unwrap_or(key)
    }

    fn load_locale(lang: &str) -> HashMap<String, String> {
        let embedded = match lang {
            "ru" => EMBEDDED_RU,
            _ => EMBEDDED_EN,
        };
        serde_json::from_str(embedded).unwrap_or_default()
    }
}

// ── Win32 FFI ─────────────────────────────────────────────────

fn send_pipe_message(msg: &[u8]) {
    unsafe extern "system" {
        fn CreateFileW(lpFileName: *const u16, dwDesiredAccess: u32, dwShareMode: u32,
            lpSecurityAttributes: *mut c_void, dwCreationDisposition: u32,
            dwFlagsAndAttributes: u32, hTemplateFile: isize) -> isize;
        fn WriteFile(hFile: isize, lpBuffer: *const c_void, nNumberOfBytesToWrite: u32,
            lpNumberOfBytesWritten: *mut u32, lpOverlapped: *mut c_void) -> i32;
        fn CloseHandle(hObject: isize) -> i32;
    }
    unsafe {
        let name: Vec<u16> = "\\\\.\\pipe\\VoxMiMSettings\0".encode_utf16().collect();
        let pipe = CreateFileW(name.as_ptr(), 0x40000000, 0, std::ptr::null_mut(), 3, 0, 0);
        if pipe == 0 || pipe == -1isize as isize { return; }
        let mut written: u32 = 0;
        WriteFile(pipe, msg.as_ptr() as *const c_void, msg.len() as u32, &mut written, std::ptr::null_mut());
        CloseHandle(pipe);
    }
}

fn ensure_single_instance() -> bool {
    unsafe extern "system" {
        fn CreateMutexW(lpMutexAttributes: *mut c_void, bInitialOwner: i32, lpName: *const u16) -> isize;
        fn GetLastError() -> u32;
        fn CloseHandle(hObject: isize) -> i32;
    }
    const ERROR_ALREADY_EXISTS: u32 = 183;
    unsafe {
        let name: Vec<u16> = "Local\\VoxMiMSettingsInstance\0".encode_utf16().collect();
        let mutex = CreateMutexW(std::ptr::null_mut(), 0, name.as_ptr());
        if mutex == 0 { return true; }
        if GetLastError() == ERROR_ALREADY_EXISTS {
            CloseHandle(mutex);
            return false;
        }
        true
    }
}

fn browse_for_folder(initial: &str) -> Option<String> {
    use std::ffi::c_void;

    unsafe extern "system" {
        fn CoInitializeEx(pvReserved: *mut c_void, dwCoInit: u32) -> i32;
        fn CoUninitialize();
        fn CoCreateInstance(rclsid: *const u8, pUnkOuter: *mut c_void, dwClsContext: u32, riid: *const u8, ppv: *mut *mut c_void) -> i32;
        fn CoTaskMemFree(pv: *mut c_void);
        fn SHCreateItemFromParsingName(pszPath: *const u16, pbc: *mut c_void, riid: *const u8, ppv: *mut *mut c_void) -> i32;
    }

    // CLSID_FileOpenDialog {DC1C5A9C-E88A-4DDE-A5A1-60F82A20AEF7}
    const CLSID_FILE_OPEN_DIALOG: [u8; 16] = [0x9C, 0x5A, 0x1C, 0xDC, 0x8A, 0xE8, 0xDE, 0x4D, 0xA5, 0xA1, 0x60, 0xF8, 0x2A, 0x20, 0xAE, 0xF7];
    // IID_IFileDialog {42F85136-DB7E-439C-85F1-E4075D135FC8}
    const IID_IFILE_DIALOG: [u8; 16] = [0x36, 0x51, 0xF8, 0x42, 0x7E, 0xDB, 0x9C, 0x43, 0x85, 0xF1, 0xE4, 0x07, 0x5D, 0x13, 0x5F, 0xC8];
    // IID_IShellItem {43826D1E-E718-42EE-BC55-A1E261C37BFE}
    const IID_ISHELL_ITEM: [u8; 16] = [0x1E, 0x6D, 0x82, 0x43, 0x18, 0xE7, 0xEE, 0x42, 0xBC, 0x55, 0xA1, 0xE2, 0x61, 0xC3, 0x7B, 0xFE];

    const CLSCTX_INPROC_SERVER: u32 = 0x1;
    const COINIT_APARTMENTTHREADED: u32 = 0x2;
    const FOS_PICKFOLDERS: u32 = 0x00000020;
    const FOS_FORCEFILESYSTEM: u32 = 0x00000040;
    const SIGDN_FILESYSPATH: u32 = 0x80058000;

    unsafe {
        let coinit = CoInitializeEx(std::ptr::null_mut(), COINIT_APARTMENTTHREADED);
        let must_uninit = coinit == 0; // S_OK — инициализировали сами

        let mut dialog: *mut c_void = std::ptr::null_mut();
        let hr = CoCreateInstance(
            CLSID_FILE_OPEN_DIALOG.as_ptr(), std::ptr::null_mut(), CLSCTX_INPROC_SERVER,
            IID_IFILE_DIALOG.as_ptr(), &mut dialog,
        );
        if hr < 0 || dialog.is_null() {
            if must_uninit { CoUninitialize(); }
            return None;
        }

        // Vtable IFileDialog: IUnknown(0-2), IModalWindow::Show(3),
        // SetOptions(9), SetFolder(12), SetTitle(17), GetResult(20)
        let vtbl = *(dialog as *const *const usize);
        type ComSetOptions = unsafe extern "system" fn(*mut c_void, u32) -> i32;
        type ComSetPtr = unsafe extern "system" fn(*mut c_void, *mut c_void) -> i32;
        type ComSetTitle = unsafe extern "system" fn(*mut c_void, *const u16) -> i32;
        type ComGetPtr = unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32;
        type ComShow = unsafe extern "system" fn(*mut c_void) -> i32;
        type ComRelease = unsafe extern "system" fn(*mut c_void) -> u32;
        type ComGetName = unsafe extern "system" fn(*mut c_void, u32, *mut *mut u16) -> i32;

        let set_options: ComSetOptions = std::mem::transmute(*vtbl.add(9));
        let _ = set_options(dialog, FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM);

        // Открываем на выбранной ранее папке (если она задана)
        if !initial.is_empty() {
            let wide: Vec<u16> = initial.encode_utf16().chain(std::iter::once(0)).collect();
            let mut item: *mut c_void = std::ptr::null_mut();
            if SHCreateItemFromParsingName(wide.as_ptr(), std::ptr::null_mut(), IID_ISHELL_ITEM.as_ptr(), &mut item) >= 0 && !item.is_null() {
                let set_folder: ComSetPtr = std::mem::transmute(*vtbl.add(12));
                let _ = set_folder(dialog, item);
                let release: ComRelease = std::mem::transmute(*(*(item as *const *const usize)).add(2));
                let _ = release(item);
            }
        }

        let title: Vec<u16> = "Выберите папку\0".encode_utf16().collect();
        let set_title: ComSetTitle = std::mem::transmute(*vtbl.add(17));
        let _ = set_title(dialog, title.as_ptr());

        let show: ComShow = std::mem::transmute(*vtbl.add(3));
        let hr_show = show(dialog);

        let mut path: Option<String> = None;
        let mut result: *mut c_void = std::ptr::null_mut();
        if hr_show >= 0 {
            let get_result: ComGetPtr = std::mem::transmute(*vtbl.add(20));
            if get_result(dialog, &mut result) >= 0 && !result.is_null() {
                let rvtbl = *(result as *const *const usize);
                let get_name: ComGetName = std::mem::transmute(*rvtbl.add(5));
                let mut name_ptr: *mut u16 = std::ptr::null_mut();
                if get_name(result, SIGDN_FILESYSPATH, &mut name_ptr) >= 0 && !name_ptr.is_null() {
                    let mut len = 0usize;
                    while *name_ptr.add(len) != 0 { len += 1; }
                    let wide_str: Vec<u16> = std::slice::from_raw_parts(name_ptr, len).to_vec();
                    CoTaskMemFree(name_ptr as *mut c_void);
                    let s = String::from_utf16_lossy(&wide_str);
                    if !s.is_empty() { path = Some(s); }
                }
                let release: ComRelease = std::mem::transmute(*rvtbl.add(2));
                let _ = release(result);
            }
        }

        let release: ComRelease = std::mem::transmute(*vtbl.add(2));
        let _ = release(dialog);

        if must_uninit { CoUninitialize(); }
        path
    }
}

fn scan_models(dir: &str) -> Vec<String> {
    let mut models = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().map(|e| e == "bin").unwrap_or(false) {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    models.push(name.to_string());
                }
            }
        }
    }
    models.sort();
    models
}

// TODO: remove_window_caption с EnumWindows + задержка — костыль.
// Нужно: сделать caption невидимым при создании окна (через Fenestra или SetWindowLong до ShowWindow),
// чтобы не было мелькания заголовка. Или использовать кастомную немаскированную область перетаскивания (HTCAPTION).
fn remove_window_caption() {
    unsafe extern "system" {
        fn EnumWindows(lpEnumFunc: unsafe extern "system" fn(isize, isize) -> i32, lParam: isize) -> i32;
        fn GetWindowThreadProcessId(hWnd: isize, lpdwProcessId: *mut u32) -> u32;
        fn SetWindowLongW(hWnd: isize, nIndex: i32, dwNewLong: i32) -> i32;
        fn GetWindowLongW(hWnd: isize, nIndex: i32) -> i32;
        fn SetWindowPos(
            hWnd: isize,
            hWndInsertAfter: isize,
            X: i32,
            Y: i32,
            cx: i32,
            cy: i32,
            uFlags: u32,
        ) -> i32;
    }

    const GWL_STYLE: i32 = -16;
    const GWL_EXSTYLE: i32 = -20;
    const WS_CAPTION: i32 = 0x00C00000;
    const WS_THICKFRAME: i32 = 0x00040000;
    const WS_MINIMIZEBOX: i32 = 0x00020000;
    const WS_MAXIMIZEBOX: i32 = 0x00010000;
    const WS_EX_TOPMOST: i32 = 0x00000008;
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_FRAMECHANGED: u32 = 0x0020;
    const HWND_TOPMOST: isize = -1;

    let pid = std::process::id();

    unsafe extern "system" fn callback(hwnd: isize, lparam: isize) -> i32 {
        unsafe {
            let target_pid = lparam as u32;
            let mut window_pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, &mut window_pid);
            if window_pid == target_pid {
                let style = GetWindowLongW(hwnd, GWL_STYLE);
                SetWindowLongW(hwnd, GWL_STYLE, style & !(WS_CAPTION | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX));
                let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
                SetWindowLongW(hwnd, GWL_EXSTYLE, ex_style | WS_EX_TOPMOST);
                SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_FRAMECHANGED);
            }
            1
        }
    }

    unsafe {
        EnumWindows(callback, pid as isize);
    }
}

fn install_topmost_hook() {
    unsafe extern "system" {
        fn SetWindowsHookExW(
            idHook: i32,
            lpfn: unsafe extern "system" fn(i32, isize, isize) -> isize,
            hmod: isize,
            dwThreadId: u32,
        ) -> isize;
        fn CallNextHookEx(hhk: isize, nCode: i32, wParam: isize, lParam: isize) -> isize;
        fn GetCurrentThreadId() -> u32;
        fn GetWindowLongW(hWnd: isize, nIndex: i32) -> i32;
        fn SetWindowLongW(hWnd: isize, nIndex: i32, dwNewLong: i32) -> i32;
        fn SetWindowPos(
            hWnd: isize,
            hWndInsertAfter: isize,
            X: i32,
            Y: i32,
            cx: i32,
            cy: i32,
            uFlags: u32,
        ) -> i32;
    }

    const WH_CBT: i32 = 5;
    const HCBT_CREATEWND: i32 = 3;
    const HCBT_ACTIVATE: i32 = 5;
    const GWL_EXSTYLE: i32 = -20;
    const WS_EX_TOPMOST: i32 = 0x00000008;
    const HWND_TOPMOST: isize = -1;
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOSIZE: u32 = 0x0001;

    #[allow(non_snake_case)]
    unsafe extern "system" fn topmost_hook(nCode: i32, wParam: isize, lParam: isize) -> isize {
        unsafe {
            if nCode == HCBT_CREATEWND || nCode == HCBT_ACTIVATE {
                let hwnd = wParam;
                let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
                SetWindowLongW(hwnd, GWL_EXSTYLE, ex_style | WS_EX_TOPMOST);
                SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
            }
            CallNextHookEx(0, nCode, wParam, lParam)
        }
    }

    unsafe {
        let tid = GetCurrentThreadId();
        SetWindowsHookExW(WH_CBT, topmost_hook, 0, tid);
    }
}

fn config_path() -> std::path::PathBuf {
    std::env::current_exe()
        .ok().and_then(|p| p.parent().map(|p| p.join("config.json")))
        .unwrap_or_else(|| std::path::PathBuf::from("config.json"))
}

fn load_config() -> serde_json::Value {
    let cp = config_path();
    if let Ok(content) = std::fs::read_to_string(&cp) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) { return v; }
    }
    serde_json::json!({})
}

fn save_config(cfg: &serde_json::Value) {
    let cp = config_path();
    if let Ok(content) = serde_json::to_string_pretty(cfg) {
        let _ = std::fs::write(&cp, &content);
    }
}

fn get_config_mtime() -> u64 {
    std::fs::metadata(config_path())
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn set_from_value(app: &mut SettingsApp, cfg: &serde_json::Value) {
    app.engine_server = cfg.get("engine_mode").and_then(|v| v.as_str()).map_or(false, |s| s == "server");
    app.det_server = cfg.get("detector_mode").and_then(|v| v.as_str()).map_or(false, |s| s == "server");
    app.use_gpu = cfg.get("use_gpu").and_then(|v| v.as_bool()).unwrap_or(true);
    app.wake_enable = cfg.get("wake_mode").and_then(|v| v.as_bool()).unwrap_or(false);
    app.vad_enable = cfg.get("vad").and_then(|v| v.get("enabled")).and_then(|v| v.as_bool()).unwrap_or(false);
    app.vad_threshold = cfg.get("vad").and_then(|v| v.get("threshold")).and_then(|v| v.as_f64()).map_or("0.008".into(), |v| format!("{:.3}", v));
    app.vad_timeout = cfg.get("vad").and_then(|v| v.get("silence_duration_secs")).and_then(|v| v.as_f64()).map_or("1.5".into(), |v| format!("{:.1}", v));
    app.vad_start_timeout = cfg.get("vad").and_then(|v| v.get("start_timeout_secs")).and_then(|v| v.as_f64()).map_or("2.0".into(), |v| format!("{:.1}", v));
    if let Some(tf) = cfg.get("text_fix") {
        app.trailing_space = tf.get("trailing_space").and_then(|v| v.as_bool()).unwrap_or(false);
        app.fix_hallucinations = tf.get("fix_hallucinations").and_then(|v| v.as_bool()).unwrap_or(true);
        app.fix_user_dict = tf.get("fix_user_dict").and_then(|v| v.as_bool()).unwrap_or(true);
        app.fix_repetitions = tf.get("fix_repetitions").and_then(|v| v.as_bool()).unwrap_or(true);
        app.fix_punctuation = tf.get("fix_punctuation").and_then(|v| v.as_bool()).unwrap_or(true);
    }
    app.math_mode = cfg.get("math_mode").and_then(|v| v.as_bool()).unwrap_or(false);
    app.noise_filter = cfg.get("noise_filter_enabled").and_then(|v| v.as_bool()).unwrap_or(true);
    app.warmup = cfg.get("warmup_on_start").and_then(|v| v.as_bool()).unwrap_or(true);
    app.show_result = cfg.get("show_result").and_then(|v| v.as_bool()).unwrap_or(false);
    app.log_enable = cfg.get("log_enabled").and_then(|v| v.as_bool()).unwrap_or(false);
    app.log_dir = cfg.get("log_dir").and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or_default();
    app.dark_mode = cfg.get("dark_mode").and_then(|v| v.as_bool()).unwrap_or(false);
    app.keep_wav = cfg.get("keep_wav").and_then(|v| v.as_bool()).unwrap_or(false);
    app.show_console = cfg.get("show_console_on_start").and_then(|v| v.as_bool()).unwrap_or(true);
    app.cur_lang = if cfg.get("language").and_then(|v| v.as_str()).unwrap_or("ru") == "en" { 1 } else { 0 };
    app.cmd_max_words = cfg.get("command_max_words").and_then(|v| v.as_i64()).unwrap_or(3).to_string();
    app.whisper_timeout = cfg.get("whisper_timeout_secs").and_then(|v| v.as_i64()).unwrap_or(120).to_string();
    app.whisper_bins = cfg.get("whisper_bins_path").and_then(|v| v.as_str()).unwrap_or_default().to_string();
    app.cur_tab = cfg.get("cur_tab").and_then(|v| v.as_i64()).unwrap_or(0) as usize;
    app.subtitle_format = cfg.get("subtitle_format").and_then(|v| v.as_str()).unwrap_or("srt").to_string();
    app.window_x = cfg.get("window_x").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    app.window_y = cfg.get("window_y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    app.locale = SettingsApp::load_locale(if app.cur_lang == 1 { "en" } else { "ru" });

    let model_path = cfg.get("model_path").and_then(|v| v.as_str()).unwrap_or("");
    let det_model = cfg.get("detector_model").and_then(|v| v.as_str()).unwrap_or("");
    app.model_dir = std::path::Path::new(model_path).parent()
        .and_then(|p| p.to_str()).unwrap_or("").to_string();
    app.models = scan_models(&app.model_dir);
    app.transcriber_model_idx = app.models.iter().position(|m| {
        std::path::Path::new(model_path).file_name().and_then(|n| n.to_str()).map_or(false, |f| m == f)
    }).unwrap_or(0);
    app.detector_model_idx = app.models.iter().position(|m| {
        std::path::Path::new(det_model).file_name().and_then(|n| n.to_str()).map_or(false, |f| m == f)
    }).unwrap_or(0);
    if app.models.is_empty() && !model_path.is_empty() {
        app.model_dir = std::path::Path::new(model_path).parent()
            .and_then(|p| p.to_str()).unwrap_or("").to_string();
    }
}

fn save_from_ui(app: &SettingsApp, cfg: &mut serde_json::Value) {
    fn set(obj: &mut serde_json::Value, path: &[&str], val: serde_json::Value) {
        if path.len() == 1 { obj[path[0]] = val; return; }
        if !obj[path[0]].is_object() { obj[path[0]] = serde_json::json!({}); }
        set(&mut obj[path[0]], &path[1..], val);
    }
    set(cfg, &["engine_mode"], serde_json::json!(if app.engine_server { "server" } else { "one-shot" }));
    set(cfg, &["detector_mode"], serde_json::json!(if app.det_server { "server" } else { "one-shot" }));
    set(cfg, &["use_gpu"], serde_json::json!(app.use_gpu));
    set(cfg, &["wake_mode"], serde_json::json!(app.wake_enable));
    set(cfg, &["vad", "enabled"], serde_json::json!(app.vad_enable));
    if let Ok(val) = app.vad_threshold.trim().parse::<f64>() {
        set(cfg, &["vad", "threshold"], serde_json::json!(val));
    }
    // Удаляем старое поле aggressiveness, если осталось
    if let Some(vad) = cfg.get_mut("vad").and_then(|v| v.as_object_mut()) {
        vad.remove("aggressiveness");
    }
    if let Ok(secs) = app.vad_timeout.trim().parse::<f64>() {
        set(cfg, &["vad", "silence_duration_secs"], serde_json::json!(secs));
    }
    if let Ok(secs) = app.vad_start_timeout.trim().parse::<f64>() {
        set(cfg, &["vad", "start_timeout_secs"], serde_json::json!(secs));
    }
    set(cfg, &["text_fix", "trailing_space"], serde_json::json!(app.trailing_space));
    set(cfg, &["text_fix", "fix_hallucinations"], serde_json::json!(app.fix_hallucinations));
    set(cfg, &["text_fix", "fix_user_dict"], serde_json::json!(app.fix_user_dict));
    set(cfg, &["text_fix", "fix_repetitions"], serde_json::json!(app.fix_repetitions));
    set(cfg, &["text_fix", "fix_punctuation"], serde_json::json!(app.fix_punctuation));
    set(cfg, &["math_mode"], serde_json::json!(app.math_mode));
    set(cfg, &["noise_filter_enabled"], serde_json::json!(app.noise_filter));
    set(cfg, &["warmup_on_start"], serde_json::json!(app.warmup));
    set(cfg, &["show_result"], serde_json::json!(app.show_result));
    set(cfg, &["log_enabled"], serde_json::json!(app.log_enable));
    set(cfg, &["dark_mode"], serde_json::json!(app.dark_mode));
    set(cfg, &["keep_wav"], serde_json::json!(app.keep_wav));
    set(cfg, &["show_console_on_start"], serde_json::json!(app.show_console));
    if let Ok(n) = app.whisper_timeout.trim().parse::<u64>() {
        let n = n.clamp(10, 180);
        set(cfg, &["whisper_timeout_secs"], serde_json::json!(n));
    }
    let bins = app.whisper_bins.trim();
    set(cfg, &["whisper_bins_path"], if bins.is_empty() { serde_json::Value::Null } else { serde_json::json!(bins) });
    set(cfg, &["language"], serde_json::json!(if app.cur_lang == 1 { "en" } else { "ru" }));
    set(cfg, &["cur_tab"], serde_json::json!(app.cur_tab));
    set(cfg, &["subtitle_format"], serde_json::json!(app.subtitle_format));
    set(cfg, &["window_x"], serde_json::json!(app.window_x));
    set(cfg, &["window_y"], serde_json::json!(app.window_y));
    if let Ok(n) = app.cmd_max_words.trim().parse::<u32>() {
        set(cfg, &["command_max_words"], serde_json::json!(n));
    }
    if !app.models.is_empty() {
        let dir = &app.model_dir;
        if app.transcriber_model_idx < app.models.len() {
            let full = std::path::Path::new(dir).join(&app.models[app.transcriber_model_idx]);
            set(cfg, &["model_path"], serde_json::json!(full.to_string_lossy().to_string()));
        }
        if app.detector_model_idx < app.models.len() {
            let full = std::path::Path::new(dir).join(&app.models[app.detector_model_idx]);
            set(cfg, &["detector_model"], serde_json::json!(full.to_string_lossy().to_string()));
        }
    }
}

// ── App ────────────────────────────────────────────────────────

impl App for SettingsApp {
    type Msg = Msg;

    fn update(&mut self, msg: Msg) {
        match msg {
            Msg::SetTab(t) => { self.cur_tab = t; self.apply(); }
            Msg::SetEngineMode(v) => { self.engine_server = v; self.apply(); }
            Msg::SetDetMode(v) => { self.det_server = v; self.apply(); }
            Msg::ToggleGpu => { self.use_gpu = !self.use_gpu; self.apply(); }
            Msg::BrowseFolder => {
                if let Some(dir) = browse_for_folder(&self.model_dir) {
                    send_pipe_message(format!("folder:{dir}").as_bytes());
                    self.model_dir = dir;
                    self.models = scan_models(&self.model_dir);
                }
            }
            Msg::BrowseWhisperBins => {
                if let Some(dir) = browse_for_folder(&self.whisper_bins) {
                    send_pipe_message(format!("folder:{dir}").as_bytes());
                    self.whisper_bins = dir;
                    self.apply();
                }
            }
            Msg::SelectTranscriberModel(i) => {
                self.transcriber_model_idx = i;
                if i < self.models.len() { self.apply(); }
            }
            Msg::SelectDetectorModel(i) => {
                self.detector_model_idx = i;
                if i < self.models.len() { self.apply(); }
            }
            Msg::ToggleWake => { self.wake_enable = !self.wake_enable; self.apply(); }
            Msg::ToggleVad => { self.vad_enable = !self.vad_enable; self.apply(); }
            Msg::VadThresholdUp => {
                let v: f64 = self.vad_threshold.parse().unwrap_or(0.008);
                let v = ((v + 0.002) * 1000.0).round() / 1000.0;
                if v <= 0.05 { self.vad_threshold = format!("{:.3}", v); self.apply(); }
            }
            Msg::VadThresholdDown => {
                let v: f64 = self.vad_threshold.parse().unwrap_or(0.008);
                let v = ((v - 0.002) * 1000.0).round() / 1000.0;
                if v >= 0.002 { self.vad_threshold = format!("{:.3}", v); self.apply(); }
            }
            Msg::VadThresholdSet(s) => {
                if let Ok(v) = s.trim().parse::<f64>() {
                    let v = v.clamp(0.002, 0.05);
                    self.vad_threshold = format!("{:.3}", v);
                    self.apply();
                }
            }
            Msg::VadThresholdReset => {
                self.vad_threshold = "0.008".to_string();
                self.apply();
            }
            Msg::VadTimeoutUp => {
                let v: f64 = self.vad_timeout.parse().unwrap_or(1.5);
                let v = ((v + 0.1) * 100.0).round() / 100.0;
                if v <= 10.0 { self.vad_timeout = format!("{:.1}", v); self.apply(); }
            }
            Msg::VadTimeoutDown => {
                let v: f64 = self.vad_timeout.parse().unwrap_or(1.5);
                let v = ((v - 0.1) * 100.0).round() / 100.0;
                if v >= 0.5 { self.vad_timeout = format!("{:.1}", v); self.apply(); }
            }
            Msg::VadTimeoutSet(s) => {
                if let Ok(v) = s.trim().parse::<f64>() {
                    let v = v.clamp(0.5, 10.0);
                    self.vad_timeout = format!("{:.1}", v);
                    self.apply();
                }
            }
            Msg::VadStartTimeoutUp => {
                let v: f64 = self.vad_start_timeout.parse().unwrap_or(2.0);
                let v = ((v + 0.1) * 100.0).round() / 100.0;
                if v <= 15.0 { self.vad_start_timeout = format!("{:.1}", v); self.apply(); }
            }
            Msg::VadStartTimeoutDown => {
                let v: f64 = self.vad_start_timeout.parse().unwrap_or(2.0);
                let v = ((v - 0.1) * 100.0).round() / 100.0;
                if v >= 0.0 { self.vad_start_timeout = format!("{:.1}", v); self.apply(); }
            }
            Msg::VadStartTimeoutSet(s) => {
                if let Ok(v) = s.trim().parse::<f64>() {
                    let v = v.clamp(0.0, 15.0);
                    self.vad_start_timeout = format!("{:.1}", v);
                    self.apply();
                }
            }
            Msg::ToggleHall => { self.fix_hallucinations = !self.fix_hallucinations; self.apply(); }
            Msg::ToggleUserDict => { self.fix_user_dict = !self.fix_user_dict; self.apply(); }
            Msg::ToggleRep => { self.fix_repetitions = !self.fix_repetitions; self.apply(); }
            Msg::TogglePunct => { self.fix_punctuation = !self.fix_punctuation; self.apply(); }
            Msg::SetCmdMaxWords(s) => self.cmd_max_words = s,
            Msg::ToggleMath => { self.math_mode = !self.math_mode; self.apply(); }
            Msg::ToggleNoise => { self.noise_filter = !self.noise_filter; self.apply(); }
            Msg::ToggleWarmup => { self.warmup = !self.warmup; self.apply(); }
            Msg::ToggleShow => { self.show_result = !self.show_result; self.apply(); }
            Msg::ToggleLog => { self.log_enable = !self.log_enable; self.apply(); }
            Msg::ToggleTrail => { self.trailing_space = !self.trailing_space; self.apply(); }
            Msg::SetLang(i) => {
                self.cur_lang = i;
                self.locale = SettingsApp::load_locale(if i == 1 { "en" } else { "ru" });
                self.apply();
            }
            Msg::ToggleDark => { self.dark_mode = !self.dark_mode; self.apply(); }
            Msg::SetSubtitleFormat(f) => { self.subtitle_format = f; self.apply(); }
            Msg::ToggleKeepWav => { self.keep_wav = !self.keep_wav; self.apply(); }
            Msg::ToggleShowConsole => { self.show_console = !self.show_console; self.apply(); }
            Msg::WhisperTimeoutUp => {
                let v: i64 = self.whisper_timeout.trim().parse().unwrap_or(120);
                let v = (v + 10).min(180);
                self.whisper_timeout = v.to_string();
                self.apply();
            }
            Msg::WhisperTimeoutDown => {
                let v: i64 = self.whisper_timeout.trim().parse().unwrap_or(120);
                let v = (v - 10).max(10);
                self.whisper_timeout = v.to_string();
                self.apply();
            }
            Msg::WhisperTimeoutSet(s) => {
                if let Ok(v) = s.trim().parse::<i64>() {
                    let v = v.clamp(10, 180);
                    self.whisper_timeout = v.to_string();
                    self.apply();
                }
            }
            Msg::ReloadConfig => {
                let new_cfg = load_config();
                set_from_value(self, &new_cfg);
            }
            Msg::Debug => send_pipe_message(b"debug"),
            Msg::Close => { self.apply(); std::process::exit(0); }
        }
    }

    fn view(&self) -> Element<Msg> {
        let mut c: Vec<Element<Msg>> = Vec::new();

        // Custom title bar with close button
        let ver = env!("CARGO_PKG_VERSION");
        c.push(
            row().gap(SP2).items_center().children([
                text(format!("VoxMiM — Settings v{ver}")).size(TextSize::Lg),
                spacer(),
                icon_button(fenestra::text("×").color(Color::from_rgb8(255, 0, 0))).size(ControlSize::Xs).variant(ButtonVariant::Ghost).on_click(Msg::Close).into(),
            ])
        );
        c.push(divider());

        // Tabs
        let g = self.t("settings.tab.general").to_string();
        let r = self.t("settings.tab.recording").to_string();
        let x = self.t("settings.tab.text").to_string();
        let o = self.t("settings.tab.other").to_string();
        c.push(tabs(self.cur_tab, [&*g, &*r, &*x, &*o], |i| Msg::SetTab(i)));
        c.push(divider());

        match self.cur_tab {
            0 => c.push(self.tab_basic()),
            1 => c.push(self.tab_recording()),
            2 => c.push(self.tab_text()),
            _ => c.push(self.tab_other()),
        }
        col().gap(SP2).p(SP3).children(c)
    }

    fn theme(&self) -> Theme {
        if self.dark_mode { Theme::dark() } else { Theme::light() }
    }

    fn init(&mut self, proxy: Proxy<Self::Msg>) {
        let cfg = load_config();
        set_from_value(self, &cfg);
        self.proxy = Some(proxy.clone());
        self.last_config_mtime = get_config_mtime();

        // Следим за изменениями config.json извне (трей)
        std::thread::Builder::new()
            .name("config-watch".into())
            .spawn(move || {
                let mut last_mtime = get_config_mtime();
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    let mtime = get_config_mtime();
                    if mtime != 0 && mtime != last_mtime {
                        last_mtime = mtime;
                        proxy.send(Msg::ReloadConfig);
                    }
                }
            })
            .ok();

        if self.window_x != 0 || self.window_y != 0 {
            unsafe extern "system" {
                fn EnumWindows(lpEnumFunc: unsafe extern "system" fn(isize, isize) -> i32, lParam: isize) -> i32;
                fn GetWindowThreadProcessId(hWnd: isize, lpdwProcessId: *mut u32) -> u32;
                fn SetWindowPos(hWnd: isize, hWndInsertAfter: isize, X: i32, Y: i32, cx: i32, cy: i32, uFlags: u32) -> i32;
            }
            const SWP_NOSIZE: u32 = 0x0001;
            const SWP_NOZORDER: u32 = 0x0004;
            let (x, y) = (self.window_x, self.window_y);
            let pid = std::process::id();
            let cb_data = (x, y, pid);
            unsafe extern "system" fn setpos_cb(hwnd: isize, lparam: isize) -> i32 {
                unsafe {
                    let (x, y, target_pid) = *(lparam as *const (i32, i32, u32));
                    let mut window_pid: u32 = 0;
                    GetWindowThreadProcessId(hwnd, &mut window_pid);
                    if window_pid == target_pid {
                        SetWindowPos(hwnd, 0, x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER);
                    }
                }
                1
            }
            unsafe {
                EnumWindows(setpos_cb, &cb_data as *const _ as isize);
            }
        }
    }
}

impl SettingsApp {
    fn apply(&mut self) {
        self.save_window_position();
        let mut cfg = load_config();
        save_from_ui(self, &mut cfg);
        save_config(&cfg);
        send_pipe_message(b"reload");
    }

    fn save_window_position(&mut self) {
        unsafe extern "system" {
            fn GetActiveWindow() -> isize;
            fn GetWindowRect(hWnd: isize, lpRect: *mut RECT) -> i32;
        }
        #[repr(C)]
        struct RECT { left: i32, top: i32, right: i32, bottom: i32 }
        unsafe {
            let hwnd = GetActiveWindow();
            if hwnd != 0 {
                let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
                if GetWindowRect(hwnd, &mut r) != 0 {
                    self.window_x = r.left;
                    self.window_y = r.top;
                }
            }
        }
    }

    fn tab_basic(&self) -> Element<Msg> {
        let model_refs: Vec<&str> = self.models.iter().map(|s| s.as_str()).collect();
        let en = self.engine_server;
        let de = self.det_server;
        col().gap(SP2).p(SP3).children(vec![
            text(self.t("settings.language_section")).weight(Weight::Semibold).text_align(TextAlign::End).into(),
            row().gap(SP2).children([
                spacer(),
                select(self.cur_lang, ["Русский", "English"]).width(150.0).on_change(Msg::SetLang).into(),
            ]),
            divider(),
            checkbox(self.use_gpu).label(self.t("settings.gpu")).on_toggle(Msg::ToggleGpu).into(),
            divider(),
            text(self.t("settings.engine_section")).weight(Weight::Semibold).text_align(TextAlign::End).into(),
            radio(!en).label(self.t("settings.engine_one_shot")).on_select(Msg::SetEngineMode(false)).into(),
            radio(en).label(self.t("settings.engine_server")).on_select(Msg::SetEngineMode(true)).into(),
            row().gap(SP2).items_center().children([
                text(self.t("settings.models_dir")),
                text_input(&self.model_dir).width(250.0).into(),
                button(self.t("settings.browse")).on_click(Msg::BrowseFolder).into(),
            ]),
            row().gap(SP2).items_center().children([
                text(self.t("settings.model")),
                select(self.transcriber_model_idx, model_refs.clone()).width(350.0).on_change(Msg::SelectTranscriberModel).into(),
            ]),
            divider(),
            text(self.t("settings.detector_section")).weight(Weight::Semibold).text_align(TextAlign::End).into(),
            radio(!de).label(self.t("settings.engine_one_shot")).on_select(Msg::SetDetMode(false)).into(),
            radio(de).label(self.t("settings.engine_server")).on_select(Msg::SetDetMode(true)).into(),
            row().gap(SP2).items_center().children([
                text(self.t("settings.models_dir")),
                text_input(&self.model_dir).width(250.0).into(),
                button(self.t("settings.browse")).on_click(Msg::BrowseFolder).into(),
            ]),
            row().gap(SP2).items_center().children([
                text(self.t("settings.model")),
                select(self.detector_model_idx, model_refs).width(350.0).on_change(Msg::SelectDetectorModel).into(),
            ]),
            spacer(),
        ])
    }

    fn tab_recording(&self) -> Element<Msg> {
        let to = self.vad_timeout.clone();
        let tso = self.vad_start_timeout.clone();
        let thr = self.vad_threshold.clone();
        let sec = self.t("settings.seconds");
        let bins = self.whisper_bins.clone();

        // Собираем каждый stepper как Element до вложения
        let a = text(sec);
        let b = text(sec);
        let field_start: Element<Msg> = row().gap(SP1).items_center().children(vec![
            text_input(&tso).width(60.0).on_input(Msg::VadStartTimeoutSet).into(),
            a,
        ]);
        let field_timeout: Element<Msg> = row().gap(SP1).items_center().children(vec![
            text_input(&to).width(60.0).on_input(Msg::VadTimeoutSet).into(),
            b,
        ]);
        let st_s: Vec<Element<Msg>> = vec![
            icon_button(fenestra::text("▲")).size(ControlSize::Xs).variant(ButtonVariant::Ghost).on_click(Msg::VadStartTimeoutUp).into(),
            field_start,
            icon_button(fenestra::text("▼")).size(ControlSize::Xs).variant(ButtonVariant::Ghost).on_click(Msg::VadStartTimeoutDown).into(),
        ];
        let st_t: Vec<Element<Msg>> = vec![
            icon_button(fenestra::text("▲")).size(ControlSize::Xs).variant(ButtonVariant::Ghost).on_click(Msg::VadTimeoutUp).into(),
            field_timeout,
            icon_button(fenestra::text("▼")).size(ControlSize::Xs).variant(ButtonVariant::Ghost).on_click(Msg::VadTimeoutDown).into(),
        ];

        let thr_text: Element<Msg> = text_input(&thr).width(60.0).on_input(Msg::VadThresholdSet).into();
        let thr_reset: Element<Msg> = icon_button(fenestra::text("↩")).size(ControlSize::Xs).variant(ButtonVariant::Ghost).on_click(Msg::VadThresholdReset).into();
        let field_thr: Element<Msg> = row().gap(SP1).items_center().children(vec![
            thr_text,
            thr_reset,
        ]);
        let st_thr: Vec<Element<Msg>> = vec![
            icon_button(fenestra::text("▲")).size(ControlSize::Xs).variant(ButtonVariant::Ghost).on_click(Msg::VadThresholdUp).into(),
            field_thr,
            icon_button(fenestra::text("▼")).size(ControlSize::Xs).variant(ButtonVariant::Ghost).on_click(Msg::VadThresholdDown).into(),
        ];

        col().gap(SP2).p(SP3).children(vec![
            checkbox(self.wake_enable).label(self.t("settings.wake_enable")).on_toggle(Msg::ToggleWake).into(),
            checkbox(self.vad_enable).label(self.t("settings.vad_enable")).on_toggle(Msg::ToggleVad).into(),
            row().gap(SP2).items_center().children([
                text(self.t("settings.vad_sensitivity")),
                col().gap(SP1).items_center().children(st_thr).into(),
            ]),
            row().gap(SP2).items_center().children([
                text(self.t("settings.vad_start_timeout")),
                col().gap(SP1).items_center().children(st_s).into(),
            ]),
            row().gap(SP2).items_center().children([
                text(self.t("settings.vad_timeout")),
                col().gap(SP1).items_center().children(st_t).into(),
            ]),
            divider(),
            row().gap(SP2).items_center().children([
                text(self.t("settings.whisper_bins")),
                text_input(&bins).width(250.0).into(),
                button(self.t("settings.browse")).on_click(Msg::BrowseWhisperBins).into(),
            ]),
            spacer(),
        ])
    }

    fn tab_text(&self) -> Element<Msg> {
        let to = self.whisper_timeout.clone();
        let field: Element<Msg> = text_input(&to).width(60.0).on_input(Msg::WhisperTimeoutSet).into();
        let stepper: Vec<Element<Msg>> = vec![
            icon_button(fenestra::text("▲")).size(ControlSize::Xs).variant(ButtonVariant::Ghost).on_click(Msg::WhisperTimeoutUp).into(),
            field,
            icon_button(fenestra::text("▼")).size(ControlSize::Xs).variant(ButtonVariant::Ghost).on_click(Msg::WhisperTimeoutDown).into(),
        ];

        col().gap(SP2).p(SP3).children(vec![
            checkbox(self.fix_hallucinations).label(self.t("settings.fix_hallucinations")).on_toggle(Msg::ToggleHall).into(),
            checkbox(self.fix_user_dict).label(self.t("settings.fix_user_dict")).on_toggle(Msg::ToggleUserDict).into(),
            checkbox(self.fix_repetitions).label(self.t("settings.fix_repetitions")).on_toggle(Msg::ToggleRep).into(),
            checkbox(self.fix_punctuation).label(self.t("settings.fix_punctuation")).on_toggle(Msg::TogglePunct).into(),
            row().gap(SP2).items_center().children([
                text(self.t("settings.command_max_words")),
                text_input(&self.cmd_max_words).width(60.0).on_input(|s| Msg::SetCmdMaxWords(s)).into(),
            ]),
            divider(),
            row().gap(SP2).items_center().children([
                text(self.t("settings.whisper_timeout")),
                col().gap(SP1).items_center().children(stepper).into(),
            ]),
            spacer(),
        ])
    }

    fn tab_other(&self) -> Element<Msg> {
        col().gap(SP2).p(SP3).children(vec![
            checkbox(self.math_mode).label(self.t("settings.math_mode")).on_toggle(Msg::ToggleMath).into(),
            checkbox(self.noise_filter).label(self.t("settings.noise_filter")).on_toggle(Msg::ToggleNoise).into(),
            checkbox(self.warmup).label(self.t("settings.warmup")).on_toggle(Msg::ToggleWarmup).into(),
            checkbox(self.show_result).label(self.t("settings.show_result")).on_toggle(Msg::ToggleShow).into(),
            checkbox(self.log_enable).label(self.t("settings.log_enable")).on_toggle(Msg::ToggleLog).into(),
            text(self.t("settings.log_dir")),
            text_input(&self.log_dir).into(),
            checkbox(self.trailing_space).label(self.t("settings.trailing_space")).on_toggle(Msg::ToggleTrail).into(),
            checkbox(self.show_console).label(self.t("settings.show_console")).on_toggle(Msg::ToggleShowConsole).into(),
            checkbox(self.keep_wav).label(self.t("settings.keep_wav")).on_toggle(Msg::ToggleKeepWav).into(),
            text(self.t("settings.subtitle_format")),
            radio_group(
                if self.subtitle_format == "vtt" { 1 } else { 0 },
                ["SRT", "VTT"],
                |i| Msg::SetSubtitleFormat(if i == 1 { "vtt".into() } else { "srt".into() }),
            ).into(),
            checkbox(self.dark_mode).label(self.t("settings.dark_mode")).on_toggle(Msg::ToggleDark).into(),
            divider(),
            button(self.t("settings.debug_test")).on_click(Msg::Debug).into(),
            spacer(),
        ])
    }
}

// ── Main ────────────────────────────────────────────────────────

fn main() {
    if !ensure_single_instance() {
        println!("Окно настроек уже открыто");
        return;
    }

    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(100));
        remove_window_caption();
    });

    install_topmost_hook();

    let ver = env!("CARGO_PKG_VERSION");
    let opts = WindowOptions::titled(&format!("VoxMiM — Settings v{ver}"))
        .with_size(520.0, 620.0)
        .with_resizable(false);

    let app = SettingsApp {
        cur_tab: 0, dark_mode: false, engine_server: false, det_server: false,
        use_gpu: true, model_dir: String::new(), models: Vec::new(),
        transcriber_model_idx: 0, detector_model_idx: 0,
        wake_enable: false, vad_enable: false, vad_threshold: "0.008".into(),
        vad_timeout: "1.5".into(), vad_start_timeout: "2.0".into(), fix_hallucinations: true, fix_user_dict: true,
        fix_repetitions: true, fix_punctuation: true, cmd_max_words: "3".into(),
        math_mode: false, noise_filter: true, warmup: true, show_result: false,
        log_enable: false, log_dir: String::new(), trailing_space: false,
        keep_wav: false, show_console: true, whisper_timeout: "120".into(), whisper_bins: String::new(), subtitle_format: "srt".into(), cur_lang: 0, locale: HashMap::new(),
        window_x: 0, window_y: 0, proxy: None, last_config_mtime: 0,
    };
    fenestra::run(app, opts);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_app() -> SettingsApp {
        SettingsApp {
            cur_tab: 0, dark_mode: false, engine_server: false, det_server: false,
            use_gpu: true, model_dir: String::new(), models: Vec::new(),
            transcriber_model_idx: 0, detector_model_idx: 0,
            wake_enable: false, vad_enable: false, vad_threshold: "0.008".into(),
            vad_timeout: "1.5".into(), vad_start_timeout: "2.0".into(), fix_hallucinations: true, fix_user_dict: true,
            fix_repetitions: true, fix_punctuation: true, cmd_max_words: "3".into(),
            math_mode: false, noise_filter: true, warmup: true, show_result: false,
            log_enable: false, log_dir: String::new(), trailing_space: false,
            keep_wav: false, show_console: true, whisper_timeout: "120".into(), whisper_bins: String::new(), subtitle_format: "srt".into(), cur_lang: 0, locale: SettingsApp::load_locale("ru"),
            window_x: 0, window_y: 0, proxy: None, last_config_mtime: 0,
        }
    }

    #[test]
    fn test_toggle_vad_flips_field() {
        let mut app = test_app();
        app.update(Msg::ToggleVad);
        assert!(app.vad_enable);
        app.update(Msg::ToggleVad);
        assert!(!app.vad_enable);
    }

    #[test]
    fn test_vad_threshold_up_and_cap() {
        let mut app = test_app();
        app.update(Msg::VadThresholdUp);
        assert_eq!(app.vad_threshold, "0.010");
        // потолок 0.05: 0.049+0.002 = 0.050999… > 0.05 → без изменений
        app.vad_threshold = "0.049".into();
        app.update(Msg::VadThresholdUp);
        assert_eq!(app.vad_threshold, "0.049");
        app.vad_threshold = "0.050".into();
        app.update(Msg::VadThresholdUp);
        assert_eq!(app.vad_threshold, "0.050");
    }

    #[test]
    fn test_vad_threshold_down_and_floor() {
        let mut app = test_app();
        app.vad_threshold = "0.003".into();
        app.update(Msg::VadThresholdDown);
        // 0.003-0.002 = 0.000999… < 0.002 → без изменений
        assert_eq!(app.vad_threshold, "0.003");
        app.vad_threshold = "0.002".into();
        app.update(Msg::VadThresholdDown);
        assert_eq!(app.vad_threshold, "0.002");
    }

    #[test]
    fn test_vad_threshold_set_clamps() {
        let mut app = test_app();
        app.update(Msg::VadThresholdSet("0.5".into()));
        assert_eq!(app.vad_threshold, "0.050");
        app.update(Msg::VadThresholdSet("0.0001".into()));
        assert_eq!(app.vad_threshold, "0.002");
        app.update(Msg::VadThresholdSet("не число".into()));
        assert_eq!(app.vad_threshold, "0.002");
    }

    #[test]
    fn test_vad_threshold_reset() {
        let mut app = test_app();
        app.vad_threshold = "0.042".into();
        app.update(Msg::VadThresholdReset);
        assert_eq!(app.vad_threshold, "0.008");
    }

    #[test]
    fn test_vad_timeout_up_and_cap() {
        let mut app = test_app();
        app.update(Msg::VadTimeoutUp);
        assert_eq!(app.vad_timeout, "1.6");
        app.vad_timeout = "9.9".into();
        app.update(Msg::VadTimeoutUp);
        assert_eq!(app.vad_timeout, "10.0");
        app.update(Msg::VadTimeoutUp);
        assert_eq!(app.vad_timeout, "10.0");
    }

    #[test]
    fn test_vad_timeout_down_and_floor() {
        let mut app = test_app();
        app.vad_timeout = "0.5".into();
        app.update(Msg::VadTimeoutDown);
        assert_eq!(app.vad_timeout, "0.5");
        app.update(Msg::VadTimeoutUp);
        app.update(Msg::VadTimeoutDown);
        assert_eq!(app.vad_timeout, "0.5");
    }

    #[test]
    fn test_whisper_timeout_steppers() {
        let mut app = test_app();
        app.update(Msg::WhisperTimeoutUp);
        assert_eq!(app.whisper_timeout, "130");
        app.update(Msg::WhisperTimeoutDown);
        assert_eq!(app.whisper_timeout, "120");
        app.whisper_timeout = "10".into();
        app.update(Msg::WhisperTimeoutDown);
        assert_eq!(app.whisper_timeout, "10");
    }

    #[test]
    fn test_whisper_timeout_set_clamps() {
        let mut app = test_app();
        app.update(Msg::WhisperTimeoutSet("999".into()));
        assert_eq!(app.whisper_timeout, "180");
        app.update(Msg::WhisperTimeoutSet("1".into()));
        assert_eq!(app.whisper_timeout, "10");
    }

    #[test]
    fn test_toggle_text_fixes() {
        let mut app = test_app();
        app.update(Msg::ToggleTrail);
        assert!(app.trailing_space);
        app.update(Msg::ToggleHall);
        assert!(!app.fix_hallucinations);
        app.update(Msg::ToggleUserDict);
        assert!(!app.fix_user_dict);
        app.update(Msg::ToggleRep);
        assert!(!app.fix_repetitions);
        app.update(Msg::TogglePunct);
        assert!(!app.fix_punctuation);
    }

    #[test]
    fn test_set_lang_switches_locale() {
        let mut app = test_app();
        app.update(Msg::SetLang(1));
        assert_eq!(app.cur_lang, 1);
        // en-локаль загружена — ключ вкладки существует
        assert!(app.locale.contains_key("settings.tab.general"));
        app.update(Msg::SetLang(0));
        assert_eq!(app.cur_lang, 0);
    }

    #[test]
    fn test_set_subtitle_format() {
        let mut app = test_app();
        app.update(Msg::SetSubtitleFormat("vtt".into()));
        assert_eq!(app.subtitle_format, "vtt");
    }

    #[test]
    fn test_set_cmd_max_words_no_apply() {
        let mut app = test_app();
        app.update(Msg::SetCmdMaxWords("7".into()));
        assert_eq!(app.cmd_max_words, "7");
    }

    #[test]
    fn test_set_from_value_parses_config() {
        let mut app = test_app();
        let cfg = serde_json::json!({
            "engine_mode": "server",
            "detector_mode": "one-shot",
            "use_gpu": false,
            "wake_mode": true,
            "vad": {"enabled": true, "threshold": 0.012, "silence_duration_secs": 2.5, "start_timeout_secs": 3.0},
            "text_fix": {"trailing_space": true, "fix_hallucinations": false},
            "math_mode": true,
            "language": "en",
            "command_max_words": 5,
            "whisper_timeout_secs": 60,
            "whisper_bins_path": "C:\\whisper",
            "subtitle_format": "vtt",
            "dark_mode": true,
            "cur_tab": 2,
            "keep_wav": true
        });
        set_from_value(&mut app, &cfg);
        assert!(app.engine_server);
        assert!(!app.det_server);
        assert!(!app.use_gpu);
        assert!(app.wake_enable);
        assert!(app.vad_enable);
        assert_eq!(app.vad_threshold, "0.012");
        assert_eq!(app.vad_timeout, "2.5");
        assert_eq!(app.vad_start_timeout, "3.0");
        assert!(app.trailing_space);
        assert!(!app.fix_hallucinations);
        assert!(app.math_mode);
        assert_eq!(app.cur_lang, 1);
        assert_eq!(app.cmd_max_words, "5");
        assert_eq!(app.whisper_timeout, "60");
        assert_eq!(app.whisper_bins, "C:\\whisper");
        assert_eq!(app.subtitle_format, "vtt");
        assert!(app.dark_mode);
        assert_eq!(app.cur_tab, 2);
        assert!(app.keep_wav);
    }

    #[test]
    fn test_save_from_ui_writes_config() {
        let mut app = test_app();
        app.engine_server = true;
        app.vad_enable = true;
        app.vad_threshold = "0.02".into();
        app.vad_timeout = "2.0".into();
        app.trailing_space = true;
        app.math_mode = true;
        app.cur_lang = 1;
        app.subtitle_format = "vtt".into();
        app.whisper_timeout = "45".into();
        app.whisper_bins = "C:\\whisper".into();
        let mut cfg = serde_json::json!({});
        save_from_ui(&app, &mut cfg);
        assert_eq!(cfg["engine_mode"], "server");
        assert_eq!(cfg["vad"]["enabled"], true);
        assert_eq!(cfg["vad"]["threshold"], 0.02);
        assert_eq!(cfg["vad"]["silence_duration_secs"], 2.0);
        assert_eq!(cfg["text_fix"]["trailing_space"], true);
        assert_eq!(cfg["math_mode"], true);
        assert_eq!(cfg["language"], "en");
        assert_eq!(cfg["subtitle_format"], "vtt");
        assert_eq!(cfg["whisper_timeout_secs"], 45);
        assert_eq!(cfg["whisper_bins_path"], "C:\\whisper");
    }
}
