use std::fs::{self, OpenOptions};
use std::path::PathBuf;
use std::sync::Mutex;

pub static LOG_FILE: Mutex<Option<std::fs::File>> = Mutex::new(None);

pub fn init() {
    let dir = if let Ok(exe) = std::env::current_exe() {
        exe.parent().map(|p| p.join("logs")).unwrap_or_else(|| PathBuf::from("logs"))
    } else {
        PathBuf::from("logs")
    };
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("voxmim_debug.log");
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .ok();
    *LOG_FILE.lock().unwrap() = file;
}

#[macro_export]
macro_rules! dlog {
    ($($arg:tt)*) => {{
        let msg = format!(
            "[{}] {}\n",
            chrono::Local::now().format("%H:%M:%S%.3f"),
            format!($($arg)*)
        );
        eprint!("{}", msg);
        if let Ok(mut guard) = $crate::debug_log::LOG_FILE.lock() {
            if let Some(ref mut f) = *guard {
                let _ = std::io::Write::write(f, msg.as_bytes());
                let _ = std::io::Write::flush(f);
            }
        }
    }};
}
