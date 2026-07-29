mod app;
mod audio;
mod commands;
mod config;

mod download;
mod input;
mod lang;
mod pipe;
mod stt;
mod text;
mod ui;
mod vad;

use config::Config;
#[cfg(target_os = "windows")]
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
    use std::io::Write;
    builder.format(|buf, record| {
        use chrono::Local;
        let ts = Local::now().format("%Y-%m-%dT%H:%M:%S%.3f");
        let level = buf.default_level_style(record.level());
        writeln!(buf, "[{ts} {start}{lvl:<5}{end:#} {target}] {args}",
            start = level, lvl = record.level(), end = level,
            target = record.target(), args = record.args())
    });

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
    // Panic hook
    std::panic::set_hook(Box::new(|info| {
        #[cfg(target_os = "windows")]
        unsafe {
            unsafe extern "system" {
                fn GetConsoleWindow() -> isize;
                fn ShowWindow(hWnd: *mut std::ffi::c_void, nCmdShow: i32) -> i32;
            }
            let hwnd = GetConsoleWindow() as *mut std::ffi::c_void;
            if !hwnd.is_null() { ShowWindow(hwnd, 5); }
        }
        log::error!("PANIC: {info}");
        eprintln!("VoxMiM упала. Лог: logs/voxmim.log");
        #[cfg(target_os = "windows")]
        let _ = std::io::stdin().read_line(&mut String::new());
    }));

    let config = Config::load();

    log::info!("VoxMiM v{} старт", env!("CARGO_PKG_VERSION"));

    init_logger(&config);

    pipe::start_listener();

    if !single_instance() {
        log::error!("Другой экземпляр VoxMiM уже запущен");
        return;
    }

    set_dpi_awareness();
    hide_console(&config);

    log::info!("VoxMiM v{}", env!("CARGO_PKG_VERSION"));
    log::info!("Конфиг загружен: {:?}", config);

    #[cfg(target_os = "macos")]
    {
        let app = app::App::new(config);
        std::thread::Builder::new()
            .name("app".into())
            .spawn(move || {
                app.run();
            })
            .ok();
        crate::ui::tray::run_tray_main();
    }

    #[cfg(not(target_os = "macos"))]
    {
        let app = app::App::new(config);
        app.run();
    }
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

#[cfg(target_os = "macos")]
fn single_instance() -> bool {
    let lock_dir = dirs_lock_path();
    let _ = std::fs::create_dir_all(&lock_dir);
    let lock_file = lock_dir.join("voxmim.lock");
    match std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&lock_file)
    {
        Ok(_) => true,
        Err(e) => {
            log::error!("Другой экземпляр VoxMiM уже запущен ({e})");
            false
        }
    }
}

#[cfg(target_os = "macos")]
fn dirs_lock_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    std::path::PathBuf::from(home)
        .join("Library")
        .join("Application Support")
        .join("VoxMiM")
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
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
fn hide_console(config: &Config) {
    unsafe extern "system" {
        fn GetConsoleWindow() -> isize;
        fn ShowWindow(hWnd: *mut std::ffi::c_void, nCmdShow: i32) -> i32;
    }
    unsafe {
        let hwnd = GetConsoleWindow();
        if hwnd != 0 {
            CONSOLE_HWND.store(hwnd, std::sync::atomic::Ordering::SeqCst);
            if !config.show_console_on_start {
                ShowWindow(hwnd as *mut std::ffi::c_void, 0);
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn hide_console(_config: &Config) {}
