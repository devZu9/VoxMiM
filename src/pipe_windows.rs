use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};

pub static SETTINGS_CHANGED: AtomicBool = AtomicBool::new(false);

unsafe extern "system" {
    fn CreateNamedPipeW(
        lpName: *const u16, dwOpenMode: u32, dwPipeMode: u32,
        nMaxInstances: u32, nOutBufferSize: u32, nInBufferSize: u32,
        nDefaultTimeOut: u32, lpSecurityAttributes: *mut c_void,
    ) -> isize;
    fn ConnectNamedPipe(hNamedPipe: isize, lpOverlapped: *mut c_void) -> i32;
    fn DisconnectNamedPipe(hNamedPipe: isize) -> i32;
    fn ReadFile(hFile: isize, lpBuffer: *mut c_void, nNumberOfBytesToRead: u32, lpNumberOfBytesRead: *mut u32, lpOverlapped: *mut c_void) -> i32;
    fn CloseHandle(hObject: isize) -> i32;
}

const PIPE_ACCESS_INBOUND: u32 = 0x00000001;
const PIPE_TYPE_BYTE: u32 = 0x00000000;
const PIPE_READMODE_BYTE: u32 = 0x00000000;
const PIPE_WAIT: u32 = 0x00000000;
const INVALID_HANDLE_VALUE: isize = -1;

pub fn start_listener() {
    std::thread::Builder::new()
        .name("pipe".into())
        .spawn(move || {
            loop {
                unsafe {
                    let name: Vec<u16> = "\\\\.\\pipe\\VoxMiMSettings\0".encode_utf16().collect();
                    let pipe = CreateNamedPipeW(
                        name.as_ptr(),
                        PIPE_ACCESS_INBOUND,
                        PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                        1, 0, 0, 0, std::ptr::null_mut(),
                    );
                    if pipe == INVALID_HANDLE_VALUE || pipe == 0 {
                        std::thread::sleep(std::time::Duration::from_millis(1000));
                        continue;
                    }

                    ConnectNamedPipe(pipe, std::ptr::null_mut());

                    let mut buf = [0u8; 64];
                    let mut read: u32 = 0;
                    let result = ReadFile(pipe, buf.as_mut_ptr() as *mut c_void, 64, &mut read, std::ptr::null_mut());
                    if result != 0 && read > 0 {
                        let msg = String::from_utf8_lossy(&buf[..read as usize]);
                        let msg = msg.trim();
                        if msg == "reload" {
                            SETTINGS_CHANGED.store(true, Ordering::SeqCst);
                        } else if msg == "debug" {
                            log::info!("Сигнал из окна настроек: debug — связь работает");
                        }
                    }

                    DisconnectNamedPipe(pipe);
                    CloseHandle(pipe);
                }
            }
        })
        .ok();
}

pub fn check_and_clear() -> bool {
    SETTINGS_CHANGED.swap(false, Ordering::SeqCst)
}
