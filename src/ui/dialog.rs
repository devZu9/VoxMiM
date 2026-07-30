#[cfg(target_os = "windows")]
#[path = "dialog_windows.rs"]
mod imp;

#[cfg(target_os = "macos")]
#[path = "dialog_macos.rs"]
mod imp;

pub use imp::*;
