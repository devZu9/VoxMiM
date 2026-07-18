use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn dicts_dir() -> PathBuf {
    exe_dir().join("dicts")
}

pub fn models_dir() -> PathBuf {
    exe_dir().join("models")
}

fn bins_dir() -> PathBuf {
    exe_dir().join("bins")
}

pub fn logs_dir() -> PathBuf {
    exe_dir().join("logs")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub mic_name: Option<String>,
    pub mic_index: Option<usize>,
    pub model_path: PathBuf,
    pub use_gpu: bool,
    pub language: String,
    pub threads: u32,
    pub trigger: TriggerConfig,
    pub vad: VadConfig,
    pub text_fix: TextFixConfig,
    pub noise_filter_enabled: bool,
    pub math_mode: bool,
    pub show_result: bool,
    pub command_max_words: u32,
    pub warmup_on_start: bool,
    pub pre_buffer_secs: f32,
    pub max_duration_sec: u32,
    pub mouse_step: u32,
    pub log_enabled: bool,
    pub log_dir: Option<PathBuf>,
    pub whisper_bins_path: Option<String>,
    pub capture_sample_rate: Option<u32>,
    pub wake_mode: bool,
    pub wake_words: Vec<String>,
    pub detector_model: PathBuf,
    pub beam_size: i32,
    pub best_of: i32,
    pub entropy_thold: f32,
    pub temperature: f32,
    pub commands_path: Option<PathBuf>,
    pub aliases_path: Option<PathBuf>,
    pub user_dict_path: Option<PathBuf>,
    #[serde(default)]
    pub dark_mode: bool,
    #[serde(default = "default_engine_mode")]
    pub engine_mode: String,
    #[serde(default = "default_engine_mode")]
    pub detector_mode: String,
    #[serde(default, skip_serializing)]
    pub keep_model_loaded: Option<bool>,
    #[serde(default, skip_serializing)]
    pub keep_detector_loaded: Option<bool>,
    #[serde(default)]
    pub keep_wav: bool,
    #[serde(default = "default_true")]
    pub show_console_on_start: bool,
    #[serde(default)]
    pub window_x: i32,
    #[serde(default)]
    pub window_y: i32,
    #[serde(default)]
    pub cur_tab: usize,
    #[serde(default = "default_whisper_timeout")]
    pub whisper_timeout_secs: u64,
}

fn default_whisper_timeout() -> u64 { 10 }

fn default_true() -> bool { true }

fn default_engine_mode() -> String { "server".to_string() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerConfig {
    pub button: TriggerButton,
    pub keyboard: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TriggerButton {
    Middle,
    Right,
    Extra,
    Keyboard,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VadConfig {
    pub enabled: bool,
    #[serde(default = "default_vad_threshold")]
    pub threshold: f32,
    pub silence_duration_secs: f32,
    pub accept_short_speech: bool,
    #[serde(default = "default_start_timeout")]
    pub start_timeout_secs: f32,
}

fn default_vad_threshold() -> f32 { 0.008 }
fn default_start_timeout() -> f32 { 2.0 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextFixConfig {
    pub fix_hallucinations: bool,
    pub fix_user_dict: bool,
    pub fix_repetitions: bool,
    pub fix_punctuation: bool,
    #[serde(default)]
    pub trailing_space: bool,
}

impl Default for Config {
    fn default() -> Self {
        let models = models_dir();
        let dicts = dicts_dir();

        let known_models = [
            r"C:\_workPortable\WhisperCpp\models\ggml-large-v3-russian.bin",
            r"C:\_workPortable\WhisperCpp\models\ggml-large-v3-turbo-q8_0.bin",
            r"C:\_workPortable\WhisperCpp\models\ggml-medium-q8_0.bin",
        ];
        let model_path = known_models.iter()
            .find(|p| std::path::Path::new(p).exists())
            .map(|p| std::path::PathBuf::from(p))
            .unwrap_or_else(|| models.join("ggml-tiny.bin"));

        let has_cuda = std::path::Path::new(r"C:\_workPortable\WhisperCpp\bins\cu-bin-blas12.4\ggml-cuda.dll").exists();
        let detector_default = model_path.clone();

        Self {
            mic_name: None,
            mic_index: None,
            model_path,
            use_gpu: has_cuda,
            language: "ru".to_string(),
            threads: 0,
            trigger: TriggerConfig {
                button: TriggerButton::Keyboard,
                keyboard: Some("ctrl+insert".to_string()),
            },
            vad: VadConfig {
                enabled: false,
                threshold: 0.008,
                silence_duration_secs: 1.5,
                accept_short_speech: true,
                start_timeout_secs: 2.0,
            },
            text_fix: TextFixConfig {
                fix_hallucinations: true,
                fix_user_dict: true,
                fix_repetitions: true,
                fix_punctuation: true,
                trailing_space: false,
            },
            noise_filter_enabled: true,
            math_mode: false,
            show_result: false,
            command_max_words: 3,
            warmup_on_start: true,
            pre_buffer_secs: 0.5,
            max_duration_sec: 180,
            mouse_step: 150,
            log_enabled: false,
            log_dir: None,
            whisper_bins_path: Some(bins_dir().to_string_lossy().to_string()),
            capture_sample_rate: None,
            wake_mode: false,
            wake_words: vec!["слушай".to_string(), "бро запиши".to_string(), "записывай".to_string()],
            detector_model: known_models.iter()
                .find(|p| {
                    let name = std::path::Path::new(p).file_name()
                        .and_then(|f| f.to_str()).unwrap_or("");
                    name.contains("small")
                })
                .map(|p| std::path::PathBuf::from(p))
                .unwrap_or_else(|| detector_default.clone()),
            beam_size: 5,
            best_of: 5,
            entropy_thold: 2.4,
            temperature: 0.0,
            commands_path: Some(dicts.join("commands.json")),
            aliases_path: Some(dicts.join("aliases.json")),
            user_dict_path: Some(dicts.join("user_dict.json")),
            engine_mode: "one-shot".to_string(),
            detector_mode: "one-shot".to_string(),
            dark_mode: false,
            keep_model_loaded: None,
            keep_detector_loaded: None,
            keep_wav: false,
            show_console_on_start: true,
            window_x: 0,
            window_y: 0,
            cur_tab: 0,
            whisper_timeout_secs: 10,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let config_path = exe_dir().join("config.json");

        let mut cfg = 'load: {
            if config_path.exists() {
                match std::fs::read_to_string(&config_path) {
                    Ok(content) => {
                        // Миграция: aggressiveness → threshold
                        let content = Self::migrate_vad(&content);
                        match serde_json::from_str(&content) {
                            Ok(c) => break 'load c,
                            Err(e) => log::warn!("Ошибка парсинга config.json: {e}"),
                        }
                    }
                    Err(e) => log::warn!("Не удалось прочитать config.json: {e}"),
                }
            }
            log::info!("config.json не найден, создаю по умолчанию");
            let cfg = Config::default();
            cfg
        };

        if !cfg.model_path.exists() {
            let fallbacks = [
                r"C:\_workPortable\WhisperCpp\models\ggml-large-v3-russian.bin",
                r"C:\_workPortable\WhisperCpp\models\ggml-large-v3-turbo-q8_0.bin",
                r"C:\_workPortable\WhisperCpp\models\ggml-medium-q8_0.bin",
            ];
            if let Some(found) = fallbacks.iter().find(|p| std::path::Path::new(p).exists()) {
                log::info!("Модель найдена: {found}");
                cfg.model_path = std::path::PathBuf::from(found);
            }
        }

        if !cfg.detector_model.exists() {
            let detector_fallbacks = [
                r"C:\_workPortable\WhisperCpp\models\ggml-small-q8_0.bin",
                r"C:\_workPortable\WhisperCpp\models\ggml-medium-q8_0.bin",
            ];
            if let Some(found) = detector_fallbacks.iter().find(|p| std::path::Path::new(p).exists()) {
                log::info!("Детектор: {found}");
                cfg.detector_model = std::path::PathBuf::from(found);
            }
        }

        // Миграция со старых полей keep_model_loaded на engine_mode
        if cfg.keep_model_loaded.unwrap_or(false) {
            if cfg.engine_mode == "one-shot" {
                cfg.engine_mode = "server".to_string();
                log::info!("Миграция: keep_model_loaded=true → engine_mode=server");
            }
        }
        if cfg.keep_detector_loaded.unwrap_or(false) {
            if cfg.detector_mode == "one-shot" {
                cfg.detector_mode = "server".to_string();
            }
        }


        if let Err(e) = crate::config::ensure_paths(&cfg) {
            log::warn!("Не удалось создать папки: {e}");
        }

        cfg
    }

    fn migrate_vad(content: &str) -> String {
        let mut raw: serde_json::Value = match serde_json::from_str(content) {
            Ok(v) => v,
            Err(_) => return content.to_string(),
        };
        if let Some(vad) = raw.get_mut("vad") {
            // Мигрируем только если есть старый aggressiveness И нет нового threshold
            if vad.get("threshold").is_none() {
                if let Some(aggr) = vad.get("aggressiveness").and_then(|v| v.as_i64()) {
                    let threshold = match aggr {
                        0 => 0.05,
                        1 => 0.03,
                        2 => 0.015,
                        _ => 0.008,
                    };
                    let _ = vad.as_object_mut().map(|o| o.remove("aggressiveness"));
                    vad["threshold"] = serde_json::json!(threshold);
                    log::info!("Миграция: vad.aggressiveness={aggr} → vad.threshold={threshold}");
                }
            }
        }
        serde_json::to_string_pretty(&raw).unwrap_or_else(|_| content.to_string())
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let exe = exe_dir();
        std::fs::create_dir_all(&exe)?;
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(exe.join("config.json"), content)?;
        Ok(())
    }
}

pub fn ensure_paths(cfg: &Config) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = cfg.model_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::create_dir_all(dicts_dir())?;
    std::fs::create_dir_all(bins_dir())?;
    Ok(())
}

pub fn dicts_path() -> PathBuf {
    dicts_dir()
}
