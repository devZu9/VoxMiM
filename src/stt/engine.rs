use std::path::Path;
use std::process::{Command, Stdio};

fn samples_to_pcm16(samples: &[f32]) -> Vec<u8> {
    samples
        .iter()
        .map(|&s| ((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())
        .flatten()
        .collect()
}

/// WAV-заголовок: 44 байта, 16kHz, mono, 16 bit PCM
fn wav_header(data_len: u32, sample_rate: u32) -> Vec<u8> {
    let byte_rate = sample_rate * 2;
    let mut h = Vec::with_capacity(44);
    h.extend(b"RIFF");
    h.extend(&(data_len + 36).to_le_bytes());
    h.extend(b"WAVE");
    h.extend(b"fmt ");
    h.extend(&16u32.to_le_bytes());
    h.extend(&1u16.to_le_bytes());
    h.extend(&1u16.to_le_bytes());
    h.extend(&sample_rate.to_le_bytes());
    h.extend(&byte_rate.to_le_bytes());
    h.extend(&2u16.to_le_bytes());
    h.extend(&16u16.to_le_bytes());
    h.extend(b"data");
    h.extend(&data_len.to_le_bytes());
    h
}

/// Ресемплинг 48kHz → 16kHz (усреднение блоков по 3 сэмпла)
fn resample_to_16khz(samples: &[f32], input_rate: u32) -> Vec<f32> {
    if input_rate == 16000 || input_rate == 0 {
        return samples.to_vec();
    }
    let ratio = (input_rate / 16000) as usize;
    if ratio <= 1 {
        return samples.to_vec();
    }
    let out_len = samples.len() / ratio;
    let mut out = Vec::with_capacity(out_len);
    for chunk in samples.chunks_exact(ratio) {
        let sum: f32 = chunk.iter().sum();
        out.push(sum / ratio as f32);
    }
    out
}

fn find_whisper_bin() -> String {
    let candidates = [
        r"C:\_workPortable\WhisperCpp\bins\cu-bin-blas12.4\whisper-cli.exe",
        r"C:\_workPortable\WhisperCpp\bins\cu-bin-blas11.8\whisper-cli.exe",
        r"C:\_workPortable\WhisperCpp\bins\bin\whisper-cli.exe",
        r"C:\_workPortable\WhisperCpp\bins\bin-blas\whisper-cli.exe",
    ];
    for path in &candidates {
        if Path::new(path).exists() {
            return path.to_string();
        }
    }
    "whisper-cli.exe".to_string()
}

/// Одно распознавание через whisper-cli subprocess
fn run_whisper(
    cli_path: &str,
    model_path: &str,
    samples: &[f32],
    language: &str,
    input_rate: u32,
) -> Result<String, String> {
    let samples = resample_to_16khz(samples, input_rate);
    let pcm = samples_to_pcm16(&samples);
    let wav: Vec<u8> = wav_header(pcm.len() as u32, 16000)
        .into_iter()
        .chain(pcm.into_iter())
        .collect();

    // whisper-cli не поддерживает stdin через --file -
    // Пишем во временный файл
    let wav_path = std::env::temp_dir().join(format!("voxmim_{}.wav", std::process::id()));
    std::fs::write(&wav_path, &wav)
        .map_err(|e| format!("Ошибка записи WAV: {e}"))?;

    let output = Command::new(cli_path)
        .args([
            "--file",
            wav_path.to_str().unwrap(),
            "--model",
            model_path,
            "--language",
            language,
            "--no-timestamps",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Не удалось запустить whisper-cli: {e}"))?;

    // Удаляем временный файл
    let _ = std::fs::remove_file(&wav_path);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        log::warn!("whisper-cli stderr: {stderr}");
        return Err(format!("whisper-cli: {stderr}"));
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.trim().is_empty() {
        log::debug!("whisper-cli: {stderr}");
    }

    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    log::info!("whisper-cli ({}) -> {text:?}", wav_path.display());
    Ok(text)
}

pub struct WhisperEngine {
    cli_path: String,
    detector_model: String,
    transcriber_model: String,
    language: String,
    n_threads: u32,
    input_rate: u32,
}

impl WhisperEngine {
    pub fn new() -> Self {
        Self {
            cli_path: find_whisper_bin(),
            detector_model: String::new(),
            transcriber_model: String::new(),
            language: "ru".to_string(),
            n_threads: 4,
            input_rate: 48000, // по умолчанию USB микрофон
        }
    }

    pub fn load_detector<P: AsRef<Path>>(&mut self, path: P) -> Result<(), String> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(format!("Модель детектора не найдена: {}", path.display()));
        }
        self.detector_model = path.to_string_lossy().to_string();
        log::info!("Детектор загружен: {}", path.display());
        Ok(())
    }

    pub fn load_transcriber<P: AsRef<Path>>(&mut self, path: P, use_gpu: bool) -> Result<(), String> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(format!("Модель не найдена: {}", path.display()));
        }
        if !Path::new(&self.cli_path).exists() {
            return Err(format!("whisper-cli не найден: {}", self.cli_path));
        }
        self.transcriber_model = path.to_string_lossy().to_string();
        log::info!("Транскрайбер загружен: {} (GPU: {use_gpu})", path.display());
        Ok(())
    }

    /// Быстрая транскрипция на детекторе (small модель)
    pub fn detect(&self, samples: &[f32]) -> Result<String, String> {
        if self.detector_model.is_empty() {
            return Err("Детектор не загружен".to_string());
        }
        run_whisper(&self.cli_path, &self.detector_model, samples, &self.language, self.input_rate)
    }

    /// Полная транскрипция на основной модели (large)
    pub fn transcribe(&self, samples: &[f32]) -> Result<String, String> {
        if self.transcriber_model.is_empty() {
            return Err("Модель не загружена".to_string());
        }
        run_whisper(&self.cli_path, &self.transcriber_model, samples, &self.language, self.input_rate)
    }

    pub fn set_input_rate(&mut self, rate: u32) {
        self.input_rate = rate;
    }

    pub fn warmup(&self) {
        let dummy = vec![0.0f32; 16000];

        if self.is_detector_loaded() {
            log::info!("Прогрев детектора...");
            match self.detect(&dummy) {
                Ok(_) => log::info!("Прогрев детектора OK"),
                Err(e) => log::warn!("Прогрев детектора: {e}"),
            }
        }

        log::info!("Прогрев транскрайбера...");
        match self.transcribe(&dummy) {
            Ok(_) => log::info!("Прогрев транскрайбера OK"),
            Err(e) => log::warn!("Прогрев транскрайбера: {e}"),
        }
    }

    pub fn set_language(&mut self, lang: &str) {
        self.language = lang.to_string();
    }

    pub fn set_n_threads(&mut self, n: u32) {
        self.n_threads = n.max(1);
    }

    pub fn is_loaded(&self) -> bool {
        !self.transcriber_model.is_empty()
    }

    pub fn is_detector_loaded(&self) -> bool {
        !self.detector_model.is_empty()
    }
}
