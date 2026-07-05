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
}

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
    pub aggressiveness: u32,
    pub silence_duration_secs: f32,
    pub accept_short_speech: bool,
    #[serde(default = "default_start_timeout")]
    pub start_timeout_secs: f32,
}

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
                aggressiveness: 1,
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
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let config_path = exe_dir().join("config.json");

        let mut cfg = 'load: {
            if config_path.exists() {
                match std::fs::read_to_string(&config_path) {
                    Ok(content) => match serde_json::from_str(&content) {
                        Ok(c) => break 'load c,
                        Err(e) => log::warn!("Ошибка парсинга config.json: {e}"),
                    },
                    Err(e) => log::warn!("Не удалось прочитать config.json: {e}"),
                }
            }
            log::info!("config.json не найден, создаю по умолчанию");
            let mut cfg = Config::default();
            cfg.migrate_from_voxbee();
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

        if let Err(e) = cfg.save() {
            log::warn!("Не удалось сохранить config.json: {e}");
        }

        if let Err(e) = crate::config::ensure_paths(&cfg) {
            log::warn!("Не удалось создать папки: {e}");
        }

        cfg
    }

    fn migrate_from_voxbee(&mut self) {
        let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
        let voxbee_dir = std::path::Path::new(&appdata).join("VoxBee");
        let voxbee_config = voxbee_dir.join("config.json");

        if !voxbee_config.exists() {
            return;
        }

        log::info!("Миграция из VoxBee: {}", voxbee_config.display());

        let content = match std::fs::read_to_string(&voxbee_config) {
            Ok(c) => c,
            Err(_) => return,
        };

        let v: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => return,
        };

        if let Some(idx) = v.get("microphone_index").and_then(|v| v.as_u64()) {
            self.mic_index = Some(idx as usize);
        }
        if let Some(name) = v.get("microphone_name").and_then(|v| v.as_str()) {
            self.mic_name = Some(name.to_string());
        }
        if let Some(gpu) = v.get("use_gpu").and_then(|v| v.as_bool()) {
            self.use_gpu = gpu;
        }
        if let Some(model) = v.get("model_name").and_then(|v| v.as_str()) {
            let path = models_dir().join(model);
            if path.exists() {
                self.model_path = path;
            } else {
                let known: [&std::path::Path; 3] = [
                    r"C:\_workPortable\WhisperCpp\models\ggml-large-v3-russian.bin".as_ref(),
                    r"C:\_workPortable\WhisperCpp\models\ggml-large-v3-turbo-q8_0.bin".as_ref(),
                    r"C:\_workPortable\WhisperCpp\models\ggml-medium-q8_0.bin".as_ref(),
                ];
                let model_file = std::path::Path::new(model).file_name()
                    .and_then(|f| f.to_str()).unwrap_or(model);
                if let Some(existing) = known.iter().find(|p| {
                    std::path::Path::new(p).file_name()
                        .and_then(|f| f.to_str()) == Some(model_file)
                }) {
                    self.model_path = std::path::PathBuf::from(existing);
                } else {
                    self.model_path = path;
                }
            }
        }
        if let Some(lang) = v.get("language").and_then(|v| v.as_str()) {
            self.language = lang.to_string();
        }
        if let Some(threads) = v.get("threads").and_then(|v| v.as_u64()) {
            self.threads = threads as u32;
        }
        if let Some(vad_aggr) = v.get("vad_aggressiveness").and_then(|v| v.as_u64()) {
            self.vad.aggressiveness = vad_aggr as u32;
        }
        if let Some(vad_sil) = v.get("vad_silence_duration").and_then(|v| v.as_f64()) {
            self.vad.silence_duration_secs = vad_sil as f64 as f32;
        }
        if let Some(short) = v.get("vad_accept_short_speech").and_then(|v| v.as_bool()) {
            self.vad.accept_short_speech = short;
        }
        if let Some(vad_mode) = v.get("vad_mode").and_then(|v| v.as_bool()) {
            self.vad.enabled = vad_mode;
        }
        if let Some(warmup) = v.get("warmup_on_start").and_then(|v| v.as_bool()) {
            self.warmup_on_start = warmup;
        }
        if let Some(pre) = v.get("pre_buffer_sec").and_then(|v| v.as_f64()) {
            self.pre_buffer_secs = pre as f64 as f32;
        }
        if let Some(max) = v.get("max_duration_sec").and_then(|v| v.as_u64()) {
            self.max_duration_sec = max as u32;
        }
        if let Some(step) = v.get("mouse_step").and_then(|v| v.as_u64()) {
            self.mouse_step = step as u32;
        }
        if let Some(noise) = v.get("noise_filter_enabled").and_then(|v| v.as_bool()) {
            self.noise_filter_enabled = noise;
        }
        if let Some(log) = v.get("log_enabled").and_then(|v| v.as_bool()) {
            self.log_enabled = log;
        }
        if let Some(log_dir) = v.get("log_directory").and_then(|v| v.as_str()) {
            if !log_dir.is_empty() {
                self.log_dir = Some(PathBuf::from(log_dir));
            }
        }
        if let Some(trig) = v.get("trigger_button").and_then(|v| v.as_str()) {
            self.trigger.button = TriggerButton::Keyboard;
            self.trigger.keyboard = Some(trig.replace("ctrl+key:", "ctrl+"));
        }
        if let Some(math) = v.get("math_mode").and_then(|v| v.as_bool()) {
            self.math_mode = math;
        }
        if let Some(show) = v.get("show_recognition_result").and_then(|v| v.as_bool()) {
            self.show_result = show;
        }

        if let Some(cmds) = v.get("text_fix_enabled").and_then(|v| v.as_bool()) {
            self.text_fix.fix_hallucinations = cmds;
            self.text_fix.fix_user_dict = cmds;
            self.text_fix.fix_punctuation = cmds;
            self.text_fix.fix_repetitions = cmds;
        }

        let _ = self.save();
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
