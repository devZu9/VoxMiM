use fenestra::prelude::*;
use fenestra_kit::{checkbox, button, select, text_input, tabs, radio, icon_button, ControlSize, ButtonVariant};
use fenestra::TextSize;
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
    SelectTranscriberModel(usize),
    SelectDetectorModel(usize),
    ToggleWake,
    ToggleVad,
    SetVadAggr(usize),
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
    vad_aggr: usize,
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
    locale: HashMap<String, String>,
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

fn browse_for_folder() -> Option<String> {
    unsafe extern "system" {
        fn SHBrowseForFolderW(lpbi: *const BROWSEINFOW) -> isize;
        fn SHGetPathFromIDListW(pidl: isize, pszPath: *mut u16) -> i32;
        fn CoTaskMemFree(pv: isize);
    }
    #[repr(C)]
    #[allow(non_snake_case)]
    struct BROWSEINFOW {
        hwndOwner: isize, pidlRoot: isize, pszDisplayName: *mut u16,
        lpszTitle: *const u16, ulFlags: u32, lpfn: isize,
        lParam: isize, iImage: i32,
    }
    const BIF_RETURNONLYFSDIRS: u32 = 0x0001;
    const BIF_NEWDIALOGSTYLE: u32 = 0x0040;
    unsafe {
        let title: Vec<u16> = "Выберите папку с моделями\0".encode_utf16().collect();
        let mut display_buf = [0u16; 260];
        let bi = BROWSEINFOW {
            hwndOwner: 0, pidlRoot: 0, pszDisplayName: display_buf.as_mut_ptr(),
            lpszTitle: title.as_ptr(), ulFlags: BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE,
            lpfn: 0, lParam: 0, iImage: 0,
        };
        let pidl = SHBrowseForFolderW(&bi);
        if pidl == 0 { return None; }
        let mut path_buf = [0u16; 260];
        SHGetPathFromIDListW(pidl, path_buf.as_mut_ptr());
        CoTaskMemFree(pidl);
        let path = String::from_utf16_lossy(&path_buf);
        let path = path.trim_matches('\0').to_string();
        if path.is_empty() { None } else { Some(path) }
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

fn set_from_value(app: &mut SettingsApp, cfg: &serde_json::Value) {
    app.engine_server = cfg.get("engine_mode").and_then(|v| v.as_str()).map_or(false, |s| s == "server");
    app.det_server = cfg.get("detector_mode").and_then(|v| v.as_str()).map_or(false, |s| s == "server");
    app.use_gpu = cfg.get("use_gpu").and_then(|v| v.as_bool()).unwrap_or(true);
    app.wake_enable = cfg.get("wake_mode").and_then(|v| v.as_bool()).unwrap_or(false);
    app.vad_enable = cfg.get("vad").and_then(|v| v.get("enabled")).and_then(|v| v.as_bool()).unwrap_or(false);
    app.vad_aggr = cfg.get("vad").and_then(|v| v.get("aggressiveness")).and_then(|v| v.as_i64()).unwrap_or(1) as usize;
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
    set(cfg, &["vad", "aggressiveness"], serde_json::json!(app.vad_aggr));
    if let Ok(secs) = app.vad_timeout.trim().parse::<f32>() {
        set(cfg, &["vad", "silence_duration_secs"], serde_json::json!(secs));
    }
    if let Ok(secs) = app.vad_start_timeout.trim().parse::<f32>() {
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
    set(cfg, &["language"], serde_json::json!(if app.cur_lang == 1 { "en" } else { "ru" }));
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
            Msg::SetTab(t) => self.cur_tab = t,
            Msg::SetEngineMode(v) => { self.engine_server = v; self.apply(); }
            Msg::SetDetMode(v) => { self.det_server = v; self.apply(); }
            Msg::ToggleGpu => { self.use_gpu = !self.use_gpu; self.apply(); }
            Msg::BrowseFolder => {
                if let Some(dir) = browse_for_folder() {
                    self.model_dir = dir;
                    self.models = scan_models(&self.model_dir);
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
            Msg::SetVadAggr(i) => { self.vad_aggr = i; self.apply(); }
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
            Msg::ToggleKeepWav => { self.keep_wav = !self.keep_wav; self.apply(); }
            Msg::ToggleShowConsole => { self.show_console = !self.show_console; self.apply(); }
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
                button("×").on_click(Msg::Close).into(),
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

    fn init(&mut self, _proxy: Proxy<Self::Msg>) {
        let cfg = load_config();
        set_from_value(self, &cfg);
    }
}

impl SettingsApp {
    fn apply(&self) {
        let mut cfg = load_config();
        save_from_ui(self, &mut cfg);
        save_config(&cfg);
        send_pipe_message(b"reload");
    }

    fn tab_basic(&self) -> Element<Msg> {
        let model_refs: Vec<&str> = self.models.iter().map(|s| s.as_str()).collect();
        let en = self.engine_server;
        let de = self.det_server;
        col().gap(SP2).p(SP3).children(vec![
            text(self.t("settings.engine_section")).into(),
            radio(!en).label(self.t("settings.engine_one_shot")).on_select(Msg::SetEngineMode(false)).into(),
            radio(en).label(self.t("settings.engine_server")).on_select(Msg::SetEngineMode(true)).into(),
            row().gap(SP2).children([
                text(self.t("settings.models_dir")),
                text_input(&self.model_dir).width(250.0).into(),
                button(self.t("settings.browse")).on_click(Msg::BrowseFolder).into(),
            ]),
            row().gap(SP2).children([
                text(self.t("settings.model")),
                select(self.transcriber_model_idx, model_refs.clone()).width(350.0).on_change(Msg::SelectTranscriberModel).into(),
            ]),
            divider(),
            text(self.t("settings.detector_section")).into(),
            radio(!de).label(self.t("settings.engine_one_shot")).on_select(Msg::SetDetMode(false)).into(),
            radio(de).label(self.t("settings.engine_server")).on_select(Msg::SetDetMode(true)).into(),
            row().gap(SP2).children([
                text(self.t("settings.models_dir")),
                text_input(&self.model_dir).width(250.0).into(),
                button(self.t("settings.browse")).on_click(Msg::BrowseFolder).into(),
            ]),
            row().gap(SP2).children([
                text(self.t("settings.model")),
                select(self.detector_model_idx, model_refs).width(350.0).on_change(Msg::SelectDetectorModel).into(),
            ]),
            divider(),
            checkbox(self.use_gpu).label(self.t("settings.gpu")).on_toggle(Msg::ToggleGpu).into(),
            divider(),
            text(self.t("settings.language_section")).into(),
            select(self.cur_lang, ["Русский", "English"]).width(150.0).on_change(Msg::SetLang).into(),
            spacer(),
        ])
    }

    fn tab_recording(&self) -> Element<Msg> {
        let to = self.vad_timeout.clone();
        let tso = self.vad_start_timeout.clone();
        let sec = self.t("settings.seconds");

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

        col().gap(SP2).p(SP3).children(vec![
            checkbox(self.wake_enable).label(self.t("settings.wake_enable")).on_toggle(Msg::ToggleWake).into(),
            checkbox(self.vad_enable).label(self.t("settings.vad_enable")).on_toggle(Msg::ToggleVad).into(),
            row().gap(SP2).children([
                text(self.t("settings.vad_aggressiveness")),
                select(self.vad_aggr, ["0", "1", "2", "3"]).width(100.0).on_change(Msg::SetVadAggr).into(),
            ]),
            row().gap(SP2).items_center().children([
                text(self.t("settings.vad_start_timeout")),
                col().gap(SP1).items_center().children(st_s).into(),
            ]),
            row().gap(SP2).items_center().children([
                text(self.t("settings.vad_timeout")),
                col().gap(SP1).items_center().children(st_t).into(),
            ]),
            spacer(),
        ])
    }

    fn tab_text(&self) -> Element<Msg> {
        col().gap(SP2).p(SP3).children(vec![
            checkbox(self.fix_hallucinations).label(self.t("settings.fix_hallucinations")).on_toggle(Msg::ToggleHall).into(),
            checkbox(self.fix_user_dict).label(self.t("settings.fix_user_dict")).on_toggle(Msg::ToggleUserDict).into(),
            checkbox(self.fix_repetitions).label(self.t("settings.fix_repetitions")).on_toggle(Msg::ToggleRep).into(),
            checkbox(self.fix_punctuation).label(self.t("settings.fix_punctuation")).on_toggle(Msg::TogglePunct).into(),
            row().gap(SP2).children([
                text(self.t("settings.command_max_words")),
                text_input(&self.cmd_max_words).width(60.0).on_input(|s| Msg::SetCmdMaxWords(s)).into(),
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
        wake_enable: false, vad_enable: false, vad_aggr: 1,
        vad_timeout: "1.5".into(), vad_start_timeout: "2.0".into(), fix_hallucinations: true, fix_user_dict: true,
        fix_repetitions: true, fix_punctuation: true, cmd_max_words: "3".into(),
        math_mode: false, noise_filter: true, warmup: true, show_result: false,
        log_enable: false, log_dir: String::new(), trailing_space: false,
        keep_wav: false, show_console: true, cur_lang: 0, locale: HashMap::new(),
    };
    fenestra::run(app, opts);
}
