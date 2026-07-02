use crate::audio::capture::AudioCapture;
use crate::commands::executor::{CommandAction, CommandExecutor};
use crate::config::Config;
use crate::download;
use crate::input::hotkeys::HotkeyListener;
use crate::input::inserter::TextInserter;
use crate::stt::engine::WhisperEngine;
use crate::text::fix_text;
use crate::text::user_dict::UserDict;
use crate::text::Dictionary;
use crate::ui::tray::TrayManager;
use crossbeam_channel::{Receiver, Sender};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum AppCommand {
    StartRecording,
    StopRecording,
    ChangeMic { name: String, index: usize },
    ChangeModel(String),
    ToggleGpu(bool),
    ToggleVad(bool),
    OpenSettings,
    ReloadDictionary,
    ReloadCommands,
    ToggleMathMode(bool),
    RecordingResult(String),
    AddUserEntry { wrong: String, correct: String },
    EditUserDict,
    Quit,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    Idle,
    Recording,
    Processing,
}

fn chunk_energy(samples: &[f32]) -> f32 {
    if samples.is_empty() { return 0.0; }
    samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32
}

pub struct App {
    state: AppState,
    config: Config,
    dict: Dictionary,
    user_dict: UserDict,
    inserter: TextInserter,
    executor: CommandExecutor,
    _hotkey: Option<HotkeyListener>,
    _audio: Option<AudioCapture>,
    recording: Arc<AtomicBool>,
    audio_buf: Arc<Mutex<Vec<f32>>>,
    whisper_tx: Sender<Vec<f32>>,
    cmd_tx: Sender<AppCommand>,
    cmd_rx: Receiver<AppCommand>,
}

impl App {
    pub fn new(mut config: Config) -> Self {
        let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded();
        let (whisper_tx, whisper_rx) = crossbeam_channel::unbounded::<Vec<f32>>();

        // Трей — запускаем сразу с иконкой загрузки
        let recording = Arc::new(AtomicBool::new(false));
        let ready = Arc::new(AtomicBool::new(false));
        {
            let tray_tx = cmd_tx.clone();
            let tray_rec = recording.clone();
            let tray_ready = ready.clone();
            std::thread::Builder::new()
                .name("tray".into())
                .spawn(move || {
                    let tray = TrayManager::new(tray_tx, tray_rec, tray_ready);
                    tray.run();
                })
                .ok();
        }

        let mut executor = CommandExecutor::new();
        if let Some(ref path) = config.commands_path {
            executor.load_commands(path);
        }
        if let Some(ref path) = config.aliases_path {
            executor.load_aliases(path);
        }

        // Whisper — общий доступ через Arc<Mutex>
        let bins_path = match download::ensure_whisper_bins(config.whisper_bins_path.as_deref()) {
            Ok(p) => {
                if config.whisper_bins_path.as_deref() != Some(&p) {
                    config.whisper_bins_path = Some(p.clone());
                    let _ = config.save();
                }
                p
            }
            Err(e) => {
                log::error!("Whisper binaries: {e}");
                String::new()
            }
        };
        let whisper = Arc::new(Mutex::new(WhisperEngine::new(&bins_path)));
        {
            let mut w = whisper.lock().unwrap();
            w.set_language(&config.language);
            w.set_n_threads(if config.threads > 0 { config.threads } else { 4 });

            if config.model_path.exists() {
                if let Err(e) = w.load_transcriber(&config.model_path, config.use_gpu) {
                    log::error!("{e}");
                }
            } else {
                log::warn!("Модель не найдена: {}", config.model_path.display());
            }

            if config.wake_mode && config.detector_model.exists() {
                if let Err(e) = w.load_detector(&config.detector_model) {
                    log::error!("Детектор: {e}");
                }
            }

            if w.is_loaded() && config.warmup_on_start {
                w.warmup();
            }
        }

        ready.store(true, Ordering::SeqCst);
        log::info!("Готов к работе");

        // Аудио-захват
        let mut audio = AudioCapture::new();
        let devices = AudioCapture::list_devices();
        log::info!("Доступные микрофоны:");
        for (name, idx) in &devices {
            log::info!("  [{idx}] {name}");
        }

        if let Some(ref name) = config.mic_name {
            if let Some(idx) = config.mic_index {
                let _ = audio.select_device(name, idx);
            }
        } else if let Some((name, idx)) = devices.first() {
            let _ = audio.select_device(name, *idx);
        }

        let (audio_tx, audio_rx) = mpsc::channel::<Vec<f32>>();
        let capture_rate = match audio.start_capture(audio_tx, config.capture_sample_rate) {
            Ok(rate) => {
                log::info!("Аудио-захват: {rate}Hz");
                Some(rate)
            }
            Err(e) => {
                log::error!("Аудио-захват: {e}");
                None
            }
        };

        let sample_rate = audio.sample_rate;
        if let Ok(mut w) = whisper.lock() {
            w.set_input_rate(sample_rate);
        }

        // Сохраняем частоту захвата, если подобрали новую
        if capture_rate != config.capture_sample_rate {
            config.capture_sample_rate = capture_rate;
            let _ = config.save();
        }

        // Push-to-talk: накопление
        let audio_buf: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
        let rec_flag = recording.clone();
        let buf = audio_buf.clone();

        std::thread::Builder::new()
            .name("audio-accum".into())
            .spawn(move || {
                while let Ok(chunk) = audio_rx.recv() {
                    if rec_flag.load(Ordering::SeqCst) {
                        if let Ok(mut b) = buf.lock() {
                            b.extend_from_slice(&chunk);
                        }
                    }
                }
            })
            .ok();

        // Whisper worker: транскрибация PTT
        let cmd_tx_w = cmd_tx.clone();
        let whisper_w = whisper.clone();
        std::thread::Builder::new()
            .name("whisper".into())
            .spawn(move || {
                while let Ok(samples) = whisper_rx.recv() {
                    if samples.len() < 16000 {
                        log::warn!("Короткое аудио ({} сэмплов)", samples.len());
                        continue;
                    }
                    let text = match whisper_w.lock().unwrap().transcribe(&samples) {
                        Ok(t) => t,
                        Err(e) => {
                            log::error!("Whisper: {e}");
                            continue;
                        }
                    };
                    let _ = cmd_tx_w.send(AppCommand::RecordingResult(text));
                }
            })
            .ok();

        // Wake word детекция (опционально)
        if config.wake_mode && {
            whisper.lock().unwrap().is_detector_loaded()
        } {
            let cmd_tx_d = cmd_tx.clone();
            let wake_words = config.wake_words.clone();
            let wake_words_c = wake_words.clone();
            let chunk_sz = (sample_rate / 2) as usize;

            let mut detect_audio = AudioCapture::new();
            if let Some(ref name) = config.mic_name {
                if let Some(idx) = config.mic_index {
                    let _ = detect_audio.select_device(name, idx);
                }
            } else if let Some((name, idx)) = AudioCapture::list_devices().first() {
                let _ = detect_audio.select_device(name, *idx);
            }

            let (detect_tx, detect_rx) = mpsc::channel::<Vec<f32>>();
            if detect_audio.start_capture(detect_tx, config.capture_sample_rate).is_ok() {
                let whisper_d = whisper.clone();
                let whisper_t = whisper.clone();

                std::thread::Builder::new()
                    .name("wake".into())
                    .spawn(move || {
                        let mut ring = Vec::new();
                        let mut awaiting = false;
                        let mut cmd_buf: Vec<f32> = Vec::new();

                        while let Ok(chunk) = detect_rx.recv() {
                            ring.extend_from_slice(&chunk);
                            if ring.len() < chunk_sz { continue; }

                            let energy = chunk_energy(&ring);
                            let test: Vec<f32> = ring.drain(..chunk_sz).collect();

                            if awaiting {
                                cmd_buf.extend_from_slice(&test);
                                if energy < 0.001 {
                                    let samples = std::mem::take(&mut cmd_buf);
                                    if samples.len() >= 16000 {
                                        let text = whisper_t.lock().unwrap()
                                            .transcribe(&samples).unwrap_or_default();
                                        let _ = cmd_tx_d.send(AppCommand::RecordingResult(text));
                                    }
                                    awaiting = false;
                                }
                                continue;
                            }

                            if energy < 0.002 { continue; }
                            if let Ok(text) = whisper_d.lock().unwrap().detect(&test) {
                                if wake_words_c.iter().any(|w| text.to_lowercase().contains(w)) {
                                    log::info!("Wake word: {text}");
                                    awaiting = true;
                                    cmd_buf.clear();
                                    cmd_buf.extend_from_slice(&test);
                                }
                            }
                        }
                    })
                    .ok();

                log::info!("Wake word: {wake_words:?}");
            }
        }

        // Хоткей
        let hotkey = HotkeyListener::new(cmd_tx.clone(), config.trigger.button.clone());

        // Словарь
        let dict = Dictionary::new();
        dict.load_lang(&config.language);

        // Пользовательский словарь
        let user_dict = UserDict::new();
        if let Some(ref path) = config.user_dict_path {
            user_dict.load(path);
        }

        // Кастомные фразы галлюцинаций
        let h_path = crate::config::dicts_path().join("hallucinations.txt");
        crate::text::load_custom_phrases(&h_path);

        Self {
            state: AppState::Idle,
            config,
            dict,
            user_dict,
            inserter: TextInserter::new(),
            executor,
            _hotkey: Some(hotkey),
            _audio: Some(audio),
            recording,
            audio_buf,
            whisper_tx,
            cmd_tx,
            cmd_rx,
        }
    }

    pub fn run(mut self) {
        log::info!("VoxMiM запущен");
        while let Ok(cmd) = self.cmd_rx.recv() {
            if !self.handle_command(cmd) {
                break;
            }
        }
        // Даём трей-потоку время удалить иконку
        std::thread::sleep(std::time::Duration::from_millis(200));
        log::info!("VoxMiM завершён");
    }

    fn handle_command(&mut self, cmd: AppCommand) -> bool {
        match cmd {
            AppCommand::StartRecording => { self.on_start(); true }
            AppCommand::StopRecording => { self.on_stop(); true }
            AppCommand::RecordingResult(text) => { self.on_result(&text); true }
            AppCommand::OpenSettings => { self.on_open_settings(); true }
            AppCommand::AddUserEntry { wrong, correct } => {
                self.user_dict.add_entry(&wrong, &correct);
                log::info!("Добавлено в словарь: «{wrong}» → «{correct}»");
                true
            }
            AppCommand::EditUserDict => { self.on_edit_user_dict(); true }
            AppCommand::Quit => {
                crate::ui::tray::request_exit();
                false
            }
            _ => { log::debug!("Команда: {:?}", cmd); true }
        }
    }

    fn on_open_settings(&self) {
        log::info!("Открытие настроек (будет egui)");
        #[cfg(target_os = "windows")]
        unsafe {
            let title: Vec<u16> = "VoxMiM\0".encode_utf16().collect();
            let msg: Vec<u16> = "Окно настроек появится в следующей версии.\0".encode_utf16().collect();
            windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW(
                std::ptr::null_mut(),
                msg.as_ptr(),
                title.as_ptr(),
                0,
            );
        }
    }

    fn on_edit_user_dict(&self) {
        let path = self.user_dict.path();
        if path.as_os_str().is_empty() {
            log::warn!("Путь пользовательского словаря не задан");
            return;
        }
        if !path.exists() {
            if let Err(e) = std::fs::write(&path, "{}") {
                log::error!("Не удалось создать user_dict.json: {e}");
                return;
            }
        }
        match std::process::Command::new("notepad.exe").arg(&path).spawn() {
            Ok(_) => log::info!("Открыт user_dict.json в блокноте"),
            Err(e) => log::error!("Не удалось открыть блокнот: {e}"),
        }
    }

    fn on_start(&mut self) {
        if self.state != AppState::Idle { return; }
        self.state = AppState::Recording;
        self.recording.store(true, Ordering::SeqCst);
        log::info!("▶ Запись");
    }

    fn on_stop(&mut self) {
        if self.state != AppState::Recording { return; }
        self.state = AppState::Processing;
        self.recording.store(false, Ordering::SeqCst);

        let samples = {
            let mut buf = self.audio_buf.lock().unwrap();
            std::mem::take(&mut *buf)
        };

        log::info!("⏹ Запись ({} сэмплов)", samples.len());
        if samples.len() < 16000 {
            log::warn!("Слишком короткая запись");
            self.state = AppState::Idle;
            return;
        }

        let _ = self.whisper_tx.send(samples);
    }

    fn on_result(&mut self, text: &str) {
        let text = text.trim();
        if text.is_empty() {
            self.state = AppState::Idle;
            return;
        }

        let fixed = fix_text(text, &self.config.text_fix, &self.dict, &self.user_dict);
        log::info!("📝 {fixed}");

        if let Some(action) = self.executor.try_execute(&fixed, self.config.command_max_words) {
            self.execute_action(action);
            self.state = AppState::Idle;
            return;
        }

        if self.config.math_mode {
            let math = crate::commands::math::convert_math(&fixed);
            if math != fixed {
                self.inserter.insert_text(&math);
                self.state = AppState::Idle;
                return;
            }
        }

        self.inserter.insert_text(&fixed);
        self.state = AppState::Idle;
    }

    fn execute_action(&self, action: &CommandAction) {
        match action {
            CommandAction::Paste(t) => self.inserter.insert_text(t),
            CommandAction::Hotkey(v) => log::info!("Хоткей: {v}"),
            CommandAction::MouseMove(v) => log::info!("Мышь: {v}"),
            CommandAction::MouseClick(v) => log::info!("Клик: {v}"),
            CommandAction::MouseScroll(v) => log::info!("Скролл: {v}"),
            CommandAction::MouseScrollMax(v) => log::info!("Скролл макс: {v}"),
            CommandAction::MouseMonitor(v) => log::info!("Монитор: {v}"),
            CommandAction::MouseContinuous(v) => log::info!("Движение: {v}"),
            CommandAction::MouseStop => log::info!("Стоп"),
            CommandAction::FocusSwitch => log::info!("Фокус"),
            CommandAction::FocusSave => log::info!("Сохранить фокус"),
            CommandAction::Grid(v) => log::info!("Сетка: {v}"),
            CommandAction::GridZoom(v) => log::info!("Уточнение: {v}"),
            CommandAction::SelectionMore => log::info!("Больше"),
            CommandAction::SelectionLess => log::info!("Меньше"),
            CommandAction::ToggleMathMode(e) => {
                let _ = self.cmd_tx.send(AppCommand::ToggleMathMode(*e));
            }
            CommandAction::None => {}
        }
    }
}
