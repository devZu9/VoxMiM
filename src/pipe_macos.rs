use std::io::{Read, Write};
use std::os::unix::net::UnixListener;
use std::sync::atomic::{AtomicBool, Ordering};

static SETTINGS_CHANGED: AtomicBool = AtomicBool::new(false);

fn socket_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    std::path::PathBuf::from(home)
        .join("Library")
        .join("Application Support")
        .join("VoxMiM")
        .join("settings.sock")
}

pub fn start_listener() {
    std::thread::Builder::new()
        .name("pipe".into())
        .spawn(move || {
            let path = socket_path();
            let _ = std::fs::remove_file(&path);
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            let listener = match UnixListener::bind(&path) {
                Ok(l) => l,
                Err(e) => {
                    log::error!("pipe: не удалось создать сокет {}: {e}", path.display());
                    return;
                }
            };

            log::info!("pipe: слушаю Unix socket {}", path.display());

            for stream in listener.incoming() {
                match stream {
                    Ok(mut s) => {
                        let mut buf = [0u8; 64];
                        match s.read(&mut buf) {
                            Ok(n) if n > 0 => {
                                let msg = String::from_utf8_lossy(&buf[..n]).trim().to_string();
                                if msg == "reload" {
                                    SETTINGS_CHANGED.store(true, Ordering::SeqCst);
                                } else if msg == "debug" {
                                    log::info!("Сигнал из окна настроек: debug — связь работает");
                                }
                                let _ = s.write_all(b"ok");
                            }
                            _ => {}
                        }
                    }
                    Err(e) => {
                        log::error!("pipe: ошибка соединения: {e}");
                    }
                }
            }
        })
        .ok();
}

pub fn check_and_clear() -> bool {
    SETTINGS_CHANGED.swap(false, Ordering::SeqCst)
}
