pub fn show_add_word_dialog(_parent_hwnd: *mut std::ffi::c_void, _instance: *mut std::ffi::c_void) {
    log::info!("show_add_word_dialog: macOS stub — not yet implemented");
}

pub fn show_add_hall_dialog(_parent_hwnd: *mut std::ffi::c_void, _instance: *mut std::ffi::c_void) {
    log::info!("show_add_hall_dialog: macOS stub — not yet implemented");
}

pub fn pick_audio_file() -> Option<String> {
    log::info!("pick_audio_file: macOS stub — not yet implemented");
    None
}
