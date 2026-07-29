#[cfg(target_os = "windows")]
#[path = "tray_windows.rs"]
mod imp;

#[cfg(target_os = "macos")]
#[path = "tray_macos.rs"]
mod imp;

pub use imp::*;
