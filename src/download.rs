#[cfg(target_os = "windows")]
#[path = "download_windows.rs"]
mod imp;

#[cfg(target_os = "macos")]
#[path = "download_macos.rs"]
mod imp;

pub use imp::ensure_whisper_bins;
