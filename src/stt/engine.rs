use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

use crate::dlog;

const SERVER_PORT: u16 = 8178;

fn bins_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("bins")))
        .unwrap_or_else(|| Path::new("bins").to_path_buf())
}

fn server_exe() -> PathBuf {
    let bins = bins_dir();
    for name in &["whisper-server.exe"] {
        let path = bins.join(name);
        if path.exists() { return path; }
    }
    bins.join("whisper-server.exe")
}

fn cli_exe() -> PathBuf {
    let bins = bins_dir();
    for name in &["whisper-cli.exe", "whisper-server.exe"] {
        let path = bins.join(name);
        if path.exists() { return path; }
    }
    bins.join("whisper-cli.exe")
}

fn write_wav(path: &Path, samples: &[f32], input_rate: u32) -> Result<(), String> {
    let pcm = resample_to_16khz(samples, input_rate);
    let pcm16: Vec<u8> = pcm.iter()
        .flat_map(|&s| ((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())
        .collect();
    let h = wav_header(pcm16.len() as u32, 16000);
    let wav: Vec<u8> = h.into_iter().chain(pcm16).collect();
    std::fs::write(path, &wav).map_err(|e| format!("Ошибка записи WAV: {e}"))
}

fn resample_to_16khz(samples: &[f32], input_rate: u32) -> Vec<f32> {
    if input_rate == 16000 || input_rate == 0 { return samples.to_vec(); }
    let ratio = (input_rate / 16000) as usize;
    if ratio <= 1 { return samples.to_vec(); }
    samples.chunks_exact(ratio).map(|c| c.iter().sum::<f32>() / ratio as f32).collect()
}

fn wav_header(data_len: u32, sample_rate: u32) -> Vec<u8> {
    let br = sample_rate * 2;
    let mut h = Vec::with_capacity(44);
    h.extend(b"RIFF"); h.extend(&(data_len + 36).to_le_bytes()); h.extend(b"WAVE");
    h.extend(b"fmt "); h.extend(&16u32.to_le_bytes());
    h.extend(&1u16.to_le_bytes()); h.extend(&1u16.to_le_bytes());
    h.extend(&sample_rate.to_le_bytes()); h.extend(&br.to_le_bytes());
    h.extend(&2u16.to_le_bytes()); h.extend(&16u16.to_le_bytes());
    h.extend(b"data"); h.extend(&data_len.to_le_bytes());
    h
}

fn build_multipart(file_data: &[u8], file_name: &str, lang: &str) -> Vec<u8> {
    let boundary = "----VoxMiMFormBoundary";
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(format!("Content-Disposition: form-data; name=\"file\"; filename=\"{file_name}\"\r\n").as_bytes());
    body.extend_from_slice(b"Content-Type: audio/wav\r\n\r\n");
    body.extend_from_slice(file_data); body.extend_from_slice(b"\r\n");
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(format!("Content-Disposition: form-data; name=\"language\"\r\n\r\n{lang}\r\n").as_bytes());
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"response_format\"\r\n\r\njson\r\n");
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    body
}

fn http_get(path: &str) -> Result<String, String> {
    let mut stream = TcpStream::connect_timeout(
        &"127.0.0.1:8178".parse().unwrap(), Duration::from_secs(5),
    ).map_err(|e| format!("TCP: {e}"))?;
    let req = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:8178\r\nConnection: close\r\n\r\n");
    stream.write_all(req.as_bytes()).map_err(|e| format!("HTTP write: {e}"))?;
    let mut resp = String::new();
    stream.read_to_string(&mut resp).map_err(|e| format!("HTTP read: {e}"))?;
    Ok(resp)
}

fn http_post(path: &str, content_type: &str, body: &[u8]) -> Result<String, String> {
    let mut stream = TcpStream::connect_timeout(
        &"127.0.0.1:8178".parse().unwrap(), Duration::from_secs(5),
    ).map_err(|e| format!("TCP: {e}"))?;
    let headers = format!(
        "POST {path} HTTP/1.1\r\nHost: 127.0.0.1:8178\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(headers.as_bytes()).map_err(|e| format!("HTTP write: {e}"))?;
    stream.write_all(body).map_err(|e| format!("HTTP write body: {e}"))?;
    let mut resp = String::new();
    stream.read_to_string(&mut resp).map_err(|e| format!("HTTP read: {e}"))?;
    Ok(resp)
}

fn parse_http_body(resp: &str) -> &str {
    if let Some(pos) = resp.find("\r\n\r\n") { &resp[pos + 4..] }
    else if let Some(pos) = resp.find("\n\n") { &resp[pos + 2..] }
    else { resp }
}

fn capture_stderr(child: &mut Child) -> String {
    if let Some(ref mut stderr) = child.stderr {
        let mut buf = String::new();
        let _ = stderr.read_to_string(&mut buf);
        buf
    } else { String::new() }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EngineMode {
    OneShot,
    Server,
}

impl EngineMode {
    pub fn from_str(s: &str) -> Self {
        match s {
            "server" => EngineMode::Server,
            _ => EngineMode::OneShot,
        }
    }
}

pub struct WhisperEngine {
    model_path: String,
    language: String,
    input_rate: u32,
    mode: EngineMode,
    wav_path: PathBuf,
    server: Mutex<Option<Child>>,
}

impl WhisperEngine {
    pub fn new(_bins_path: &str) -> Self {
        Self {
            model_path: String::new(),
            language: "ru".to_string(),
            input_rate: 48000,
            mode: EngineMode::OneShot,
            wav_path: std::env::temp_dir().join("voxmim_request.wav"),
            server: Mutex::new(None),
        }
    }

    pub fn set_mode(&mut self, mode: EngineMode) {
        self.mode = mode;
        if mode != EngineMode::Server {
            self.stop_server();
        }
    }

    pub fn load_model<P: AsRef<Path>>(&mut self, path: P) -> Result<(), String> {
        let path = path.as_ref();
        if !path.exists() { return Err(format!("Модель не найдена: {}", path.display())); }
        self.model_path = path.to_string_lossy().to_string();
        dlog!("Engine: модель: {}", path.display());
        Ok(())
    }

    pub fn set_language(&mut self, lang: &str) { self.language = lang.to_string(); }
    pub fn is_loaded(&self) -> bool { !self.model_path.is_empty() }
    pub fn set_input_rate(&mut self, rate: u32) { self.input_rate = rate; }

    pub fn transcribe(&self, samples: &[f32]) -> Result<String, String> {
        match self.mode {
            EngineMode::Server => self.transcribe_server(samples),
            EngineMode::OneShot => self.transcribe_one_shot(samples),
        }
    }

    // === One-shot ===
    fn transcribe_one_shot(&self, samples: &[f32]) -> Result<String, String> {
        let exe = cli_exe();
        if !exe.exists() { return Err("whisper-cli.exe не найден".to_string()); }
        if self.model_path.is_empty() { return Err("Модель не загружена".to_string()); }

        let pcm = resample_to_16khz(samples, self.input_rate);
        let pcm16: Vec<u8> = pcm.iter()
            .flat_map(|&s| ((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())
            .collect();
        let h = wav_header(pcm16.len() as u32, 16000);
        let wav: Vec<u8> = h.into_iter().chain(pcm16).collect();
        let wav_path = std::env::temp_dir().join(format!("voxmim_{}.wav", std::process::id()));
        std::fs::write(&wav_path, &wav).map_err(|e| format!("WAV: {e}"))?;

        let bins = bins_dir();
        let output = Command::new(&exe)
            .args(["-m", &self.model_path, "-f", wav_path.to_str().unwrap()])
            .args(["--language", &self.language, "--no-timestamps"])
            .stdout(Stdio::piped()).stderr(Stdio::null())
            .current_dir(&bins)
            .output().map_err(|e| format!("CLI: {e}"))?;

        let _ = std::fs::remove_file(&wav_path);
        if !output.status.success() { return Err(format!("CLI код {}", output.status)); }
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(text)
    }

    // === Server ===
    fn transcribe_server(&self, samples: &[f32]) -> Result<String, String> {
        let exe = server_exe();
        if !exe.exists() { return Err("whisper-server.exe не найден".to_string()); }

        self.ensure_server(&exe)?;
        write_wav(&self.wav_path, samples, self.input_rate)?;

        let file_data = std::fs::read(&self.wav_path).map_err(|e| format!("Read: {e}"))?;
        let multipart = build_multipart(&file_data, "audio.wav", &self.language);
        let ct = "multipart/form-data; boundary=----VoxMiMFormBoundary".to_string();
        let resp = http_post("/inference", &ct, &multipart)?;

        if !resp.contains("200 OK") && !resp.contains("200 ok") {
            let body = parse_http_body(&resp).trim().to_string();
            dlog!("Server: HTTP error — {body}");
            *self.server.lock().unwrap() = None;
            return Err(format!("Server error: {body}"));
        }

        let body = parse_http_body(&resp);
        let text = serde_json::from_str::<serde_json::Value>(body)
            .ok().and_then(|j| j["text"].as_str().map(|s| s.replace('\n', "").trim().to_string()))
            .unwrap_or_default();

        let _ = std::fs::remove_file(&self.wav_path);
        Ok(text)
    }

    fn ensure_server(&self, exe: &Path) -> Result<(), String> {
        if self.is_server_alive() { return Ok(()); }

        // Убиваем старые копии whisper-server на порту 8178
        let _ = Command::new("taskkill")
            .args(["/f", "/im", "whisper-server.exe"])
            .stdout(Stdio::null()).stderr(Stdio::null())
            .spawn().map(|mut c| { let _ = c.wait(); });

        dlog!("Server: запуск {}", exe.display());
        if self.model_path.is_empty() { return Err("Модель не задана".to_string()); }

        let bins = bins_dir();
        let port = SERVER_PORT.to_string();
        let mut child = Command::new(exe)
            .args(["-m", &self.model_path, "--port", &port])
            .args(["--language", &self.language, "--threads", "4"])
            .stdout(Stdio::null()).stderr(Stdio::piped())
            .current_dir(&bins).spawn()
            .map_err(|e| format!("spawn: {e}"))?;

        dlog!("Server: PID={}", child.id());

        let start = std::time::Instant::now();
        loop {
            if start.elapsed() > Duration::from_secs(30) {
                let stderr = capture_stderr(&mut child);
                self.kill(&mut child);
                return Err(format!("Таймаут. stderr: {stderr}"));
            }
            if !child.try_wait().map(|s| s.is_none()).unwrap_or(false) {
                let stderr = capture_stderr(&mut child);
                self.kill(&mut child);
                return Err(format!("Сервер умер. stderr: {stderr}"));
            }
            match http_get("/health") {
                Ok(r) if r.contains("200 OK") || r.contains("200 ok") => {
                    *self.server.lock().unwrap() = Some(child);
                    dlog!("Server: готов за {}ms", start.elapsed().as_millis());
                    return Ok(());
                }
                _ => { std::thread::sleep(Duration::from_millis(200)); }
            }
        }
    }

    fn is_server_alive(&self) -> bool {
        let mut guard = self.server.lock().unwrap();
        if let Some(ref mut child) = *guard {
            match child.try_wait() { Ok(None) => true, _ => { *guard = None; false } }
        } else { false }
    }

    fn kill(&self, child: &mut Child) {
        let _ = child.kill(); let _ = child.wait();
    }

    fn stop_server(&self) {
        if let Some(mut child) = self.server.lock().unwrap().take() {
            let _ = child.kill(); let _ = child.wait();
            dlog!("Server: stop");
        }
    }

    pub fn detect(&self, samples: &[f32]) -> Result<String, String> {
        // Детектор всегда one-shot (не стартует сервер)
        self.transcribe_one_shot(samples)
    }

    pub fn warmup(&self) {
        let dummy = vec![0.0f32; 16000];
        match self.transcribe(&dummy) {
            Ok(t) => dlog!("Прогрев OK: {t:?}"),
            Err(e) => dlog!("Прогрев: {e}"),
        }
    }
}

impl Drop for WhisperEngine {
    fn drop(&mut self) { self.stop_server(); }
}
