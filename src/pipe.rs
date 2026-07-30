#[cfg(target_os = "windows")]
#[path = "pipe_windows.rs"]
mod imp;

#[cfg(target_os = "macos")]
#[path = "pipe_macos.rs"]
mod imp;

pub use imp::*;
