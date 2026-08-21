use chrono::Local;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};



pub static KEEP_WAV: AtomicBool = AtomicBool::new(false);
pub fn set_keep_wav_global(keep: bool) { KEEP_WAV.store(keep, Ordering::SeqCst); }

pub static WHISPER_TIMEOUT_SECS: AtomicU64 = AtomicU64::new(10);
pub fn set_whisper_timeout(secs: u64) { WHISPER_TIMEOUT_SECS.store(secs, Ordering::SeqCst); }

pub static ENGINE_MODE_SERVER: AtomicBool = AtomicBool::new(true);
pub fn set_engine_mode_server(is_server: bool) { ENGINE_MODE_SERVER.store(is_server, Ordering::SeqCst); }

/// Папка whisper из настроек (config.whisper_bins_path)
static WHISPER_BINS: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Задать папку whisper из настроек
pub fn set_bins_global(path: Option<String>) {
    *WHISPER_BINS.lock().unwrap() = path.map(PathBuf::from);
}

const SERVER_PORT: u16 = 8178;

/// Аварийное завершение сервера (таймаут, поток висит — ts.lock() недоступен).
/// Убивает только процесс на нашем порту — чужие whisper-server не трогает.
pub fn kill_server_global() {
    crate::stt::server::kill_own_and_wait(None, SERVER_PORT);
}

fn bins_dir() -> PathBuf {
    let configured = WHISPER_BINS.lock().unwrap().clone();
    if let Some(p) = configured {
        if p.is_dir() { return p; }
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("bins")))
        .unwrap_or_else(|| Path::new("bins").to_path_buf())
}

fn server_exe() -> PathBuf {
    let bins = bins_dir();
    #[cfg(target_os = "windows")]
    let names: &[&str] = &["whisper-server.exe"];
    #[cfg(not(target_os = "windows"))]
    let names: &[&str] = &["whisper-server"];
    for name in names {
        let path = bins.join(name);
        if path.exists() { return path; }
    }
    #[cfg(target_os = "windows")]
    { bins.join("whisper-server.exe") }
    #[cfg(not(target_os = "windows"))]
    { bins.join("whisper-server") }
}

fn cli_exe() -> PathBuf {
    let bins = bins_dir();
    #[cfg(target_os = "windows")]
    let names: &[&str] = &["whisper-cli.exe", "whisper-server.exe"];
    #[cfg(not(target_os = "windows"))]
    let names: &[&str] = &["whisper-cli", "whisper-server"];
    for name in names {
        let path = bins.join(name);
        if path.exists() { return path; }
    }
    #[cfg(target_os = "windows")]
    { bins.join("whisper-cli.exe") }
    #[cfg(not(target_os = "windows"))]
    { bins.join("whisper-cli") }
}

pub fn wav_to_bytes(samples: &[f32], input_rate: u32) -> Result<Vec<u8>, String> {
    let pcm = resample_to_16khz(samples, input_rate);
    let pcm16: Vec<u8> = pcm.iter()
        .flat_map(|&s| ((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())
        .collect();
    let h = wav_header(pcm16.len() as u32, 16000);
    Ok(h.into_iter().chain(pcm16).collect())
}

pub(crate) fn write_wav(path: &Path, samples: &[f32], input_rate: u32) -> Result<(), String> {
    let bytes = wav_to_bytes(samples, input_rate)?;
    std::fs::write(path, &bytes).map_err(|e| format!("write_wav: {e}"))
}

pub fn save_pending(path: &Path, samples: &[f32], rate: u32) -> Result<(), String> {
    let mut data = rate.to_le_bytes().to_vec();
    for &s in samples {
        data.extend_from_slice(&s.to_le_bytes());
    }
    std::fs::write(path, &data).map_err(|e| format!("save_pending: {e}"))
}

pub fn load_pending(path: &Path) -> Result<(Vec<f32>, u32), String> {
    let data = std::fs::read(path).map_err(|e| format!("load_pending: {e}"))?;
    if data.len() < 4 {
        return Err("pending: файл повреждён (меньше 4 байт)".to_string());
    }
    let rate = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let samples: Vec<f32> = data[4..].chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    if samples.is_empty() {
        return Err("pending: нет семплов".to_string());
    }
    Ok((samples, rate))
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

fn http_post(path: &str, content_type: &str, body: &[u8]) -> Result<String, String> {
    let mut stream = TcpStream::connect_timeout(
        &"127.0.0.1:8178".parse().unwrap(), Duration::from_secs(5),
    ).map_err(|e| format!("TCP: {e}"))?;
    let timeout = WHISPER_TIMEOUT_SECS.load(Ordering::SeqCst);
    stream.set_read_timeout(Some(Duration::from_secs(timeout)))
        .map_err(|e| format!("set_read_timeout: {e}"))?;
    stream.set_write_timeout(Some(Duration::from_secs(timeout)))
        .map_err(|e| format!("set_write_timeout: {e}"))?;
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

pub(crate) fn wavs_dir() -> PathBuf {
    let dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("wavs")))
        .unwrap_or_else(|| PathBuf::from("wavs"));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn wav_path() -> PathBuf {
    if KEEP_WAV.load(Ordering::SeqCst) {
        let ts = Local::now().format("%Y-%m-%d_%H-%M-%S%.3f");
        wavs_dir().join(format!("voxmim_{ts}.wav"))
    } else {
        wavs_dir().join("voxmim_temp.wav")
    }
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
    server: Mutex<Option<Child>>,
    startup_lock: Mutex<()>,
}

impl WhisperEngine {
    pub fn new() -> Self {
        let _ = std::fs::create_dir_all(&wavs_dir());
        Self {
            model_path: String::new(),
            language: "ru".to_string(),
            input_rate: 48000,
            server: Mutex::new(None),
            startup_lock: Mutex::new(()),
        }
    }

    pub fn set_mode(&mut self, mode: EngineMode) {
        let is_server = mode == EngineMode::Server;
        crate::stt::engine::ENGINE_MODE_SERVER.store(is_server, Ordering::SeqCst);
        if !is_server {
            self.stop_server();
        }
    }

    pub fn load_model<P: AsRef<Path>>(&mut self, path: P) -> Result<(), String> {
        let path = path.as_ref();
        if !path.exists() { return Err(format!("Модель не найдена: {}", path.display())); }
        if !crate::stt::server::is_whisper_model(path) {
            return Err(format!(
                "Модель несовместима с whisper.cpp (нужен формат whisper/GGUF, не PyTorch): {}",
                path.display()
            ));
        }
        self.model_path = path.to_string_lossy().to_string();
        log::debug!("Engine: модель: {}", path.display());
        Ok(())
    }

    pub fn set_language(&mut self, lang: &str) { self.language = lang.to_string(); }
    pub fn is_loaded(&self) -> bool { !self.model_path.is_empty() }
    pub fn set_input_rate(&mut self, rate: u32) { self.input_rate = rate; }
    pub fn input_rate(&self) -> u32 { self.input_rate }

    pub fn transcribe(&self, samples: &[f32]) -> Result<String, String> {
        if ENGINE_MODE_SERVER.load(Ordering::SeqCst) {
            self.transcribe_server(samples)
        } else {
            self.transcribe_one_shot(samples)
        }
    }

    // === One-shot ===
    fn transcribe_one_shot(&self, samples: &[f32]) -> Result<String, String> {
        let exe = cli_exe();
        if !exe.exists() { return Err(format!("whisper-cli не найден: {}", exe.display())); }
        if self.model_path.is_empty() { return Err("Модель не загружена".to_string()); }

        let pcm = resample_to_16khz(samples, self.input_rate);
        let pcm16: Vec<u8> = pcm.iter()
            .flat_map(|&s| ((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())
            .collect();
        let h = wav_header(pcm16.len() as u32, 16000);
        let wav: Vec<u8> = h.into_iter().chain(pcm16).collect();
        let path = wav_path();
        std::fs::write(&path, &wav).map_err(|e| format!("WAV: {e}"))?;
        log::info!("WAV: {} (через one-shot CLI)", path.display());

        let bins = bins_dir();
        let output = Command::new(&exe)
            .args(["-m", &self.model_path, "-f", path.to_str().unwrap()])
            .args(["--language", &self.language, "--no-timestamps"])
            .stdout(Stdio::piped()).stderr(Stdio::null())
            .current_dir(&bins)
            .output().map_err(|e| format!("CLI: {e}"))?;

        if !KEEP_WAV.load(Ordering::SeqCst) { let _ = std::fs::remove_file(&path); }
        if !output.status.success() { return Err(format!("CLI код {}", output.status)); }
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(text)
    }

    // === Server ===
    fn transcribe_server(&self, samples: &[f32]) -> Result<String, String> {
        let exe = server_exe();
        if !exe.exists() { return Err(format!("whisper-server не найден: {}", exe.display())); }

        self.ensure_server(&exe)?;

        // Конвертируем в WAV в памяти, без диска
        let wav_bytes = wav_to_bytes(samples, self.input_rate)?;

        // Если keep_wav — сохраняем на диск для отладки
        if KEEP_WAV.load(Ordering::SeqCst) {
            let path = wav_path();
            if let Err(e) = std::fs::write(&path, &wav_bytes) {
                log::warn!("keep_wav: не удалось сохранить {e}");
            } else {
                log::info!("WAV: сохранён {}", path.display());
            }
        }

        log::info!("WAV: {} сэмплов (через server :{})", samples.len(), SERVER_PORT);

        let multipart = build_multipart(&wav_bytes, "audio.wav", &self.language);
        let ct = "multipart/form-data; boundary=----VoxMiMFormBoundary".to_string();
        let resp = http_post("/inference", &ct, &multipart)?;

        if !resp.contains("200 OK") && !resp.contains("200 ok") {
            let body = parse_http_body(&resp).trim().to_string();
            log::error!("Server: HTTP error — {body}");
            *self.server.lock().unwrap() = None;
            return Err(format!("Server error: {body}"));
        }

        let body = parse_http_body(&resp);
        let text = serde_json::from_str::<serde_json::Value>(body)
            .ok().and_then(|j| j["text"].as_str().map(|s| s.replace('\n', "").trim().to_string()))
            .unwrap_or_default();

        Ok(text)
    }

    fn ensure_server(&self, exe: &Path) -> Result<(), String> {
        if self.is_server_alive() { return Ok(()); }

        // Сериализуем запуск: два потока не должны стартовать сервер одновременно
        let _guard = self.startup_lock.lock().unwrap();
        if self.is_server_alive() { return Ok(()); }

        // Убиваем ТОЛЬКО свой сервер (по PID + остаток на нашем порту),
        // дожидаемся освобождения порта — на порту гарантированно новый процесс
        let own_pid = self.server.lock().unwrap().as_ref().map(|c| c.id());
        crate::stt::server::kill_own_and_wait(own_pid, SERVER_PORT);
        if !crate::stt::server::wait_port_free(SERVER_PORT, Duration::from_secs(5)) {
            return Err(format!("Порт {SERVER_PORT} занят другим процессом"));
        }

        if self.model_path.is_empty() { return Err("Модель не задана".to_string()); }
        if !crate::stt::server::is_whisper_model(Path::new(&self.model_path)) {
            return Err(format!("Модель несовместима с whisper.cpp: {}", self.model_path));
        }

        let bins = bins_dir();
        let start = Instant::now();
        let mut child = crate::stt::server::spawn_server(
            exe, &self.model_path, &self.language, 4, SERVER_PORT, &bins)?;
        log::info!("Server: PID={}", child.id());

        crate::stt::server::wait_ready(&mut child, SERVER_PORT, Duration::from_secs(30))?;
        log::info!("Server: готов за {}ms", start.elapsed().as_millis());
        *self.server.lock().unwrap() = Some(child);
        Ok(())
    }

    fn is_server_alive(&self) -> bool {
        let mut guard = self.server.lock().unwrap();
        if let Some(ref mut child) = *guard {
            match child.try_wait() { Ok(None) => true, _ => { *guard = None; false } }
        } else { false }
    }

    pub fn stop_server(&self) {
        let own_pid = self.server.lock().unwrap().take().map(|c| c.id());
        if own_pid.is_some() {
            crate::stt::server::kill_own_and_wait(own_pid, SERVER_PORT);
            log::info!("Server: stop (kill по PID/порту)");
        }
    }

    /// Перезапуск сервера после перезагрузки настроек.
    /// В one-shot-режиме ничего не делает.
    pub fn restart(&self) -> Result<(), String> {
        self.stop_server();
        if ENGINE_MODE_SERVER.load(Ordering::SeqCst) {
            let exe = server_exe();
            if !exe.exists() {
                return Err(format!("whisper-server не найден: {}", exe.display()));
            }
            self.ensure_server(&exe)?;
        }
        Ok(())
    }

    pub fn detect(&self, samples: &[f32]) -> Result<String, String> {
        if ENGINE_MODE_SERVER.load(Ordering::SeqCst) {
            self.transcribe_server(samples)
        } else {
            self.transcribe_one_shot(samples)
        }
    }

    pub fn warmup(&self) {
        let dummy = vec![0.0f32; 16000];
        match self.transcribe(&dummy) {
            Ok(t) => log::info!("Прогрев OK: {t:?}"),
            Err(e) => log::info!("Прогрев: {e}"),
        }
    }
}

impl Drop for WhisperEngine {
    fn drop(&mut self) { self.stop_server(); }
}

// ── Вспомогательные функции для распознавания файлов ──

/// Конвертирует аудиофайл (mp3/ogg/wav) во временный WAV 16kHz mono через ffmpeg.
/// Возвращает путь к временному WAV-файлу.
fn convert_to_wav(input: &Path) -> Result<std::path::PathBuf, String> {
    let out = input.parent().unwrap_or_else(|| Path::new(".")).join("__voxmim_temp.wav");
    let status = std::process::Command::new("ffmpeg")
        .args(["-y", "-i", input.to_str().unwrap()])
        .args(["-f", "wav", "-acodec", "pcm_s16le", "-ar", "16000", "-ac", "1"])
        .arg(out.to_str().unwrap())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| format!("ffmpeg: {e}"))?;
    if !status.success() {
        return Err("ffmpeg: ошибка конвертации".to_string());
    }
    Ok(out)
}

/// Распознаёт аудиофайл (mp3/ogg/wav) и возвращает текст.
pub fn transcribe_audio_file(
    input: &Path,
    model: &Path,
    lang: &str,
) -> Result<String, String> {
    let wav = convert_to_wav(input)?;
    let bins = bins_dir();
    let exe = bins.join("whisper-cli.exe");
    if !exe.exists() {
        return Err(format!("whisper-cli не найден: {}", exe.display()));
    }
    let output = std::process::Command::new(&exe)
        .args(["-m", model.to_str().unwrap(), "-f", wav.to_str().unwrap()])
        .args(["--language", lang, "--no-timestamps"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .current_dir(&bins)
        .output()
        .map_err(|e| format!("whisper-cli: {e}"))?;
    let _ = std::fs::remove_file(&wav);
    if !output.status.success() {
        return Err("whisper-cli: ошибка".to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Создаёт файл субтитров (.srt/.vtt) из аудиофайла.
pub fn subtitle_audio_file(
    input: &Path,
    model: &Path,
    lang: &str,
    format: &str,
) -> Result<(), String> {
    let wav = convert_to_wav(input)?;
    let bins = bins_dir();
    let exe = bins.join("whisper-cli.exe");
    if !exe.exists() {
        return Err(format!("whisper-cli не найден: {}", exe.display()));
    }
    let out_flag = match format {
        "vtt" => "--output-vtt",
        _ => "--output-srt",
    };
    let out_base = input.parent().unwrap_or_else(|| Path::new(".")).join("__voxmim_sub");
    let status = std::process::Command::new(&exe)
        .args(["-m", model.to_str().unwrap(), "-f", wav.to_str().unwrap()])
        .args(["--language", lang, out_flag, "-of", out_base.to_str().unwrap()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .current_dir(&bins)
        .status()
        .map_err(|e| format!("whisper-cli: {e}"))?;
    let _ = std::fs::remove_file(&wav);
    if !status.success() {
        return Err("whisper-cli: ошибка создания субтитров".to_string());
    }
    // Переименовываем из __voxmim_sub в имя исходного файла
    let ext = match format { "vtt" => "vtt", _ => "srt" };
    let src = out_base.with_extension(ext);
    let dst = input.with_extension(ext);
    if src != dst {
        let _ = std::fs::rename(&src, &dst);
    }
    log::info!("SubtitleFile: ✅ {}", dst.display());
    Ok(())
}
