//! Управление процессом whisper-server: запуск, остановка только своего
//! процесса (по PID/порту), ожидание освобождения порта и готовности.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// HTTP GET к 127.0.0.1:{port}. Возвращает сырой ответ.
pub fn http_get(port: u16, path: &str) -> Result<String, String> {
    let mut stream = TcpStream::connect_timeout(
        &format!("127.0.0.1:{port}").parse().unwrap(),
        Duration::from_secs(5),
    ).map_err(|e| format!("TCP: {e}"))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| format!("set_read_timeout: {e}"))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| format!("set_write_timeout: {e}"))?;
    let req = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
    stream.write_all(req.as_bytes()).map_err(|e| format!("HTTP write: {e}"))?;
    let mut resp = String::new();
    stream.read_to_string(&mut resp).map_err(|e| format!("HTTP read: {e}"))?;
    Ok(resp)
}

/// PID процесса, слушающего порт на локальной машине (netstat -ano).
#[cfg(target_os = "windows")]
fn pid_on_port(port: u16) -> Option<u32> {
    let out = Command::new("netstat").args(["-ano"]).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let needle = format!(":{port}");
    for line in text.lines() {
        if line.contains(&needle) && line.contains("LISTENING") {
            let pid = line.split_whitespace().last()?.parse::<u32>().ok()?;
            if pid > 0 { return Some(pid); }
        }
    }
    None
}

/// Имя процесса по PID (tasklist, CSV-формат: "имя","pid",...).
#[cfg(target_os = "windows")]
fn process_name(pid: u32) -> Option<String> {
    let filter = format!("PID eq {pid}");
    let out = Command::new("tasklist")
        .args(["/fi", &filter, "/fo", "csv", "/nh"])
        .output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let name = text.split(',').next()?.trim_matches('"').to_string();
    if name.is_empty() { None } else { Some(name) }
}

/// Жив ли процесс с данным PID.
#[cfg(target_os = "windows")]
fn is_pid_alive(pid: u32) -> bool {
    let filter = format!("PID eq {pid}");
    match Command::new("tasklist")
        .args(["/fi", &filter, "/fo", "csv", "/nh"])
        .output()
    {
        Ok(out) => {
            let text = String::from_utf8_lossy(&out.stdout);
            text.contains(&pid.to_string())
        }
        Err(_) => false,
    }
}

/// Убить процесс по PID и дождаться его завершения (до 3 с).
#[cfg(target_os = "windows")]
fn kill_pid(pid: u32) {
    let _ = Command::new("taskkill")
        .args(["/f", "/pid", &pid.to_string()])
        .stdout(Stdio::null()).stderr(Stdio::null())
        .status();
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline && is_pid_alive(pid) {
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Убить ТОЛЬКО свой сервер: по PID дочернего процесса + остаток на нашем
/// порту (если это whisper-server.exe). Чужие whisper-server не трогаем.
/// Затем ждём, пока порт освободится (до 5 с).
pub fn kill_own_and_wait(own_pid: Option<u32>, port: u16) {
    #[cfg(target_os = "windows")]
    {
        if let Some(pid) = own_pid {
            kill_pid(pid);
        }
        if let Some(pid) = pid_on_port(port) {
            if Some(pid) != own_pid {
                let is_ours = process_name(pid)
                    .map(|n| n.eq_ignore_ascii_case("whisper-server.exe"))
                    .unwrap_or(false);
                if is_ours {
                    kill_pid(pid);
                }
            }
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if pid_on_port(port).is_none() { return; }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = Command::new("killall")
            .arg("whisper-server")
            .stdout(Stdio::null()).stderr(Stdio::null())
            .status();
        let _ = wait_port_free(port, Duration::from_secs(5));
    }
}

/// Ждать, пока порт освободится (подключение должно проваливаться).
/// Возвращает true, если порт свободен.
pub fn wait_port_free(port: u16, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        match TcpStream::connect_timeout(
            &format!("127.0.0.1:{port}").parse().unwrap(),
            Duration::from_millis(200),
        ) {
            Ok(_) => std::thread::sleep(Duration::from_millis(100)),
            Err(_) => return true,
        }
    }
    false
}

/// Запустить whisper-server с моделью и портом.
pub fn spawn_server(
    exe: &Path,
    model_path: &str,
    language: &str,
    threads: u32,
    port: u16,
    bins: &Path,
) -> Result<Child, String> {
    Command::new(exe)
        .args(["-m", model_path, "--port", &port.to_string()])
        .args(["--language", language, "--threads", &threads.to_string()])
        .stdout(Stdio::null()).stderr(Stdio::piped())
        .current_dir(bins)
        .spawn()
        .map_err(|e| format!("spawn: {e}"))
}

/// Прочитать stderr дочернего процесса (для диагностики).
fn capture_stderr(child: &mut Child) -> String {
    if let Some(ref mut stderr) = child.stderr {
        let mut buf = String::new();
        let _ = stderr.read_to_string(&mut buf);
        buf
    } else { String::new() }
}

/// Ждать готовности сервера: процесс жив + /health отвечает 200.
/// Возвращает ошибку с stderr, если сервер умер или таймаут.
pub fn wait_ready(child: &mut Child, port: u16, timeout: Duration) -> Result<(), String> {
    let start = Instant::now();
    loop {
        if start.elapsed() > timeout {
            let stderr = capture_stderr(child);
            let _ = child.kill(); let _ = child.wait();
            return Err(format!("Таймаут. stderr: {stderr}"));
        }
        if !child.try_wait().map(|s| s.is_none()).unwrap_or(false) {
            let stderr = capture_stderr(child);
            let _ = child.kill(); let _ = child.wait();
            return Err(format!("Сервер умер. stderr: {stderr}"));
        }
        match http_get(port, "/health") {
            Ok(r) if r.contains("200 OK") || r.contains("200 ok") => return Ok(()),
            _ => { std::thread::sleep(Duration::from_millis(200)); }
        }
    }
}

/// Проверить, что файл модели совместим с whisper.cpp:
/// магик "GGUF" (новые модели) или "lmgg" (нативные ggml-модели whisper.cpp).
/// PyTorch (zip), safetensors и прочие форматы отклоняются.
pub fn is_whisper_model(path: &Path) -> bool {
    let mut f = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut magic = [0u8; 4];
    if f.read_exact(&mut magic).is_err() { return false; }
    // "GGUF" — gguf-контейнер; "lmgg" — нативный формат whisper.cpp
    magic == [0x47, 0x47, 0x55, 0x46] || magic == [0x6C, 0x6D, 0x67, 0x67]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whisper_magic_accepts_lmgg() {
        let dir = std::env::temp_dir();
        let path = dir.join("voxmim_test_lmgg.bin");
        std::fs::write(&path, b"lmgg\x9a\xca\x00\x00rest").unwrap();
        assert!(is_whisper_model(&path));
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn whisper_magic_accepts_gguf() {
        let dir = std::env::temp_dir();
        let path = dir.join("voxmim_test_valid.gguf");
        std::fs::write(&path, b"GGUF\x01rest").unwrap();
        assert!(is_whisper_model(&path));
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn whisper_magic_rejects_pytorch_zip() {
        let dir = std::env::temp_dir();
        let path = dir.join("voxmim_test_bad.bin");
        // PyTorch torch.save → zip-контейнер
        std::fs::write(&path, b"PK\x03\x04torch-model-data").unwrap();
        assert!(!is_whisper_model(&path));
        std::fs::remove_file(&path).unwrap();
    }
}