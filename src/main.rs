mod app;
mod audio;
mod commands;
mod config;
mod download;
mod input;
mod stt;
mod text;
mod ui;
mod vad;

use config::Config;
use std::sync::atomic::AtomicIsize;

#[cfg(target_os = "windows")]
pub static CONSOLE_HWND: AtomicIsize = AtomicIsize::new(0);

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    if !single_instance() {
        log::error!("Другой экземпляр VoxMiM уже запущен");
        return;
    }

    set_dpi_awareness();
    hide_console();

    let config = Config::load();
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
    }

    let name: Vec<u16> = "Local\\VoxMiM-SingleInstance\0".encode_utf16().collect();
    let handle = unsafe { CreateMutexW(std::ptr::null(), 1, name.as_ptr()) };
    handle != 0
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
        fn ShowWindow(hWnd: isize, nCmdShow: i32) -> i32;
    }
    unsafe {
        let hwnd = GetConsoleWindow();
        if hwnd != 0 {
            CONSOLE_HWND.store(hwnd, std::sync::atomic::Ordering::SeqCst);
            ShowWindow(hwnd, 0); // SW_HIDE = 0
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn hide_console() {}
