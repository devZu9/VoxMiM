mod app;
mod audio;
mod commands;
mod config;
mod debug_log;
mod download;
mod input;
mod lang;
mod pipe;
mod stt;
mod text;
mod ui;
mod vad;

use config::Config;
use std::sync::atomic::AtomicIsize;

#[cfg(target_os = "windows")]
pub static CONSOLE_HWND: AtomicIsize = AtomicIsize::new(0);

struct TeeWriter {
    file: std::sync::Arc<std::sync::Mutex<std::fs::File>>,
}

impl std::io::Write for TeeWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let _ = std::io::stdout().write(buf);
        self.file.lock().unwrap().write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        let _ = std::io::stdout().flush();
        self.file.lock().unwrap().flush()
    }
}

fn init_logger(config: &Config) {
    let mut builder = env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info"),
    );
    builder.format_timestamp_millis();

    if config.log_enabled {
        let dir = config
            .log_dir
            .clone()
            .unwrap_or_else(config::logs_dir);
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("voxmim.log");
        if let Ok(file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            let writer = TeeWriter {
                file: std::sync::Arc::new(std::sync::Mutex::new(file)),
            };
            builder.target(env_logger::Target::Pipe(Box::new(writer)));
            log::info!("Лог-файл: {}", path.display());
        }
    }

    let _ = builder.try_init();
}

fn main() {
    // Panic hook — показывает консоль и выводит ошибку перед падением
    std::panic::set_hook(Box::new(|info| {
        unsafe {
            unsafe extern "system" {
                fn GetConsoleWindow() -> isize;
                fn ShowWindow(hWnd: *mut std::ffi::c_void, nCmdShow: i32) -> i32;
            }
            let hwnd = GetConsoleWindow() as *mut std::ffi::c_void;
            if !hwnd.is_null() { ShowWindow(hwnd, 5); /* SW_SHOW */ }
        }
        dlog!("PANIC: {info}");
        eprintln!("VoxMiM упала. Лог: logs/voxmim_debug.log");
        let _ = std::io::stdin().read_line(&mut String::new());
    }));

    let config = Config::load();

    // Debug-лог — пишет всё с самого старта в logs/voxmim_debug.log
    debug_log::init();
    dlog!("VoxMiM v{} старт", env!("CARGO_PKG_VERSION"));

    init_logger(&config);

    // Named Pipe — слушаем сигналы перезагрузки настроек
    pipe::start_listener();

    if !single_instance() {
        log::error!("Другой экземпляр VoxMiM уже запущен");
        return;
    }

    set_dpi_awareness();
    hide_console();

    log::info!("VoxMiM v{}", env!("CARGO_PKG_VERSION"));
    log::info!("Конфиг загружен: {:?}", config);

    let app = app::App::new(config);
    app.run();
}

#[cfg(target_os = "windows")]
fn single_instance() -> bool {
    unsafe extern "system" {
        fn CreateMutexW(
            lpMutexAttributes: *const std::ffi::c_void,
            bInitialOwner: i32,
            lpName: *const u16,
        ) -> isize;
        fn GetLastError() -> u32;
        fn WaitForSingleObject(hHandle: isize, dwMilliseconds: u32) -> u32;
        fn CloseHandle(hObject: isize) -> i32;
    }

    const ERROR_ALREADY_EXISTS: u32 = 183;
    const WAIT_ABANDONED: u32 = 0x00000080;

    let name: Vec<u16> = "Local\\VoxMiM-SingleInstance\0".encode_utf16().collect();
    let handle = unsafe { CreateMutexW(std::ptr::null(), 1, name.as_ptr()) };
    if handle == 0 {
        return false;
    }

    let err = unsafe { GetLastError() };
    if err == ERROR_ALREADY_EXISTS {
        let wait = unsafe { WaitForSingleObject(handle, 0) };
        unsafe { CloseHandle(handle) };
        // WAIT_ABANDONED — старый процесс крашнулся, можем запускаться
        // WAIT_TIMEOUT — другой процесс жив
        wait == WAIT_ABANDONED
    } else {
        true
    }
}

#[cfg(not(target_os = "windows"))]
fn single_instance() -> bool {
    true
}

#[cfg(target_os = "windows")]
fn set_dpi_awareness() {
    unsafe extern "system" {
        fn SetProcessDPIAware() -> i32;
    }
    unsafe {
        SetProcessDPIAware();
    }
}

#[cfg(not(target_os = "windows"))]
fn set_dpi_awareness() {}

#[cfg(target_os = "windows")]
fn hide_console() {
    unsafe extern "system" {
        fn GetConsoleWindow() -> isize;
        fn ShowWindow(hWnd: *mut std::ffi::c_void, nCmdShow: i32) -> i32;
    }
    unsafe {
        let hwnd = GetConsoleWindow();
        if hwnd != 0 {
            CONSOLE_HWND.store(hwnd, std::sync::atomic::Ordering::SeqCst);
            ShowWindow(hwnd as *mut std::ffi::c_void, 0);
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn hide_console() {}
