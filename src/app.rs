use crate::audio::capture::AudioCapture;
use crate::audio::processor::AudioProcessor;
use crate::commands::executor::{CommandAction, CommandExecutor};
use crate::config::Config;

use crate::download;
use crate::input::hotkeys::HotkeyListener;
use crate::input::inserter::TextInserter;
use crate::lang;
use crate::stt::engine::WhisperEngine;
use crate::text::fix_text;
use crate::text::user_dict::UserDict;
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
    ToggleVad,
    ToggleWake,
    OpenSettings,
    ReloadDictionary,
    ReloadCommands,
    ToggleMathMode,
    RecordingResult(String),
    AddUserEntry { wrong: String, correct: String },
    EditUserDict,
    AddHallEntry { phrase: String },
    EditHallDict,
    ApplyConfig(Box<Config>),
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
    user_dict: UserDict,
    inserter: TextInserter,
    executor: CommandExecutor,
    _hotkey: Option<HotkeyListener>,
    _audio: Option<AudioCapture>,
    recording: Arc<AtomicBool>,
    audio_buf: Arc<Mutex<Vec<f32>>>,
    vad_enabled: Arc<AtomicBool>,
    whisper_tx: Sender<Vec<f32>>,
    cmd_tx: Sender<AppCommand>,
    cmd_rx: Receiver<AppCommand>,
    settings_process: Option<std::process::Child>,
}

impl App {
    pub fn new(mut config: Config) -> Self {
        let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded();
        let (whisper_tx, whisper_rx) = crossbeam_channel::unbounded::<Vec<f32>>();

        // Локализация — до трея, чтобы меню читало правильные строки
        lang::load_locale(&config.language);

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

        // Whisper — два независимых движка (транскрайбер + детектор)
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

        let mut trans = WhisperEngine::new(&bins_path);
        trans.set_language(&config.language);
        trans.set_mode(crate::stt::engine::EngineMode::from_str(&config.engine_mode));
        crate::stt::engine::set_keep_wav_global(config.keep_wav);
        if config.model_path.exists() {
            if let Err(e) = trans.load_model(&config.model_path) {
                log::error!("Транскрайбер: {e}");
            }
        } else {
            log::warn!("Модель не найдена: {}", config.model_path.display());
        }
        if config.engine_mode == "server" && trans.is_loaded() && config.warmup_on_start {
            trans.warmup();
        }
        let transcriber = Arc::new(Mutex::new(trans));

        let mut det = WhisperEngine::new(&bins_path);
        det.set_language(&config.language);
        det.set_mode(crate::stt::engine::EngineMode::from_str(&config.detector_mode));
        if config.detector_model.exists() {
            if let Err(e) = det.load_model(&config.detector_model) {
                log::error!("Детектор: {e}");
            }
            if let Ok(meta) = std::fs::metadata(&config.detector_model) {
                if meta.len() > 500_000_000 {
                    log::warn!("Модель детектора ({}) > 500 МБ. Используйте ggml-small-q8_0.bin для скорости", config.detector_model.display());
                }
            }
        }
        if config.wake_mode && det.is_loaded() && config.warmup_on_start {
            let dummy = vec![0.0f32; 16000];
            match det.detect(&dummy) {
                Ok(t) => log::info!("Прогрев детектора OK: {t:?}"),
                Err(e) => log::info!("Прогрев детектора: {e}"),
            }
        }
        let detector = Arc::new(Mutex::new(det));

        // Начальное состояние для трея
        crate::ui::tray::set_vad_state(config.vad.enabled);
        crate::ui::tray::set_wake_state(config.wake_mode);

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
        let (wake_tx, wake_rx) = mpsc::channel::<Vec<f32>>();
        let mut capture_txs = vec![audio_tx];
        if config.wake_mode {
            capture_txs.push(wake_tx);
        }
        let capture_rate = match audio.start_capture_multi(capture_txs, config.capture_sample_rate) {
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
        if let Ok(mut t) = transcriber.lock() {
            t.set_input_rate(sample_rate);
        }
        if let Ok(mut d) = detector.lock() {
            d.set_input_rate(sample_rate);
        }

        // Сохраняем частоту захвата, если подобрали новую
        if capture_rate != config.capture_sample_rate {
            config.capture_sample_rate = capture_rate;
            let _ = config.save();
        }

        // Push-to-talk: накопление + VAD (автостоп)
        let audio_buf: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
        let vad_enabled = Arc::new(AtomicBool::new(config.vad.enabled));
        AudioProcessor::spawn(
            audio_rx,
            cmd_tx.clone(),
            recording.clone(),
            audio_buf.clone(),
            vad_enabled.clone(),
            sample_rate,
            config.vad.aggressiveness,
            config.vad.silence_duration_secs,
            config.vad.start_timeout_secs,
        );

        // Whisper worker: транскрибация PTT
        let cmd_tx_w = cmd_tx.clone();
        let ts = transcriber.clone();
        std::thread::Builder::new()
            .name("whisper".into())
            .spawn(move || {
                while let Ok(samples) = whisper_rx.recv() {
                    if samples.len() < 16000 {
                        log::warn!("Короткое аудио ({} сэмплов)", samples.len());
                        continue;
                    }
                    let text = match ts.lock().unwrap().transcribe(&samples) {
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
        if config.wake_mode && detector.lock().unwrap().is_loaded() {
            let cmd_tx_d = cmd_tx.clone();
            let wake_words_c = config.wake_words.clone();
            let det = detector.clone();
            let ts = transcriber.clone();

            std::thread::Builder::new()
                .name("wake".into())
                .spawn(move || {
                    let mut ring = Vec::new();
                    let mut awaiting = false;
                    let mut cmd_buf: Vec<f32> = Vec::new();
                    let chunk_sz = sample_rate as usize / 2;

                    while let Ok(chunk) = wake_rx.recv() {
                        ring.extend_from_slice(&chunk);
                        if ring.len() < chunk_sz { continue; }

                        let energy = chunk_energy(&ring);
                        let test: Vec<f32> = ring.drain(..chunk_sz).collect();

                        if awaiting {
                            cmd_buf.extend_from_slice(&test);
                            if energy < 0.001 {
                                let samples = std::mem::take(&mut cmd_buf);
                                if samples.len() >= 16000 {
                                    let text = ts.lock().unwrap()
                                        .transcribe(&samples).unwrap_or_default();
                                    let _ = cmd_tx_d.send(AppCommand::RecordingResult(text));
                                }
                                awaiting = false;
                            }
                            continue;
                        }

                        if energy < 0.002 { continue; }
                        if let Ok(text) = det.lock().unwrap().detect(&test) {
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

            log::info!("Голосовая активация: {:?}", config.wake_words);
        }

        // Хоткей — синхронизируем начальное состояние VAD
        crate::input::hotkeys::set_vad_enabled(config.vad.enabled);
        let hotkey = HotkeyListener::new(cmd_tx.clone(), config.trigger.button.clone());

        // Словарь
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
            user_dict,
            inserter: TextInserter::new(),
            executor,
            _hotkey: Some(hotkey),
            _audio: Some(audio),
            recording,
            audio_buf,
            vad_enabled,
            whisper_tx,
            cmd_tx,
            cmd_rx,
            settings_process: None,
        }
    }

    pub fn run(mut self) {
        log::info!("VoxMiM запущен");
        loop {
            // Проверяем pipe-сигнал каждые 500ms
            match self.cmd_rx.recv_timeout(std::time::Duration::from_millis(500)) {
                Ok(cmd) => {
                    if !self.handle_command(cmd) {
                        break;
                    }
                }
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                    if crate::pipe::check_and_clear() {
                        self.reload_config();
                    }
                }
                Err(_) => break,
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
            AppCommand::ToggleVad => {
                self.config.vad.enabled = !self.config.vad.enabled;
                self.vad_enabled.store(self.config.vad.enabled, Ordering::SeqCst);
                crate::input::hotkeys::set_vad_enabled(self.config.vad.enabled);
                crate::ui::tray::set_vad_state(self.config.vad.enabled);
                let _ = self.config.save();
                log::info!("Автостоп: {}", if self.config.vad.enabled { "вкл" } else { "выкл" });
                true
            }
            AppCommand::ToggleWake => {
                self.config.wake_mode = !self.config.wake_mode;
                crate::ui::tray::set_wake_state(self.config.wake_mode);
                let _ = self.config.save();
                log::info!("Голосовая активация: {}", if self.config.wake_mode { "вкл" } else { "выкл" });
                true
            }
            AppCommand::ToggleMathMode => {
                self.config.math_mode = !self.config.math_mode;
                let _ = self.config.save();
                log::info!("Math Mode: {}", if self.config.math_mode { "вкл" } else { "выкл" });
                true
            }
            AppCommand::AddUserEntry { wrong, correct } => {
                self.user_dict.add_entry(&wrong, &correct);
                log::info!("Добавлено в словарь: «{wrong}» → «{correct}»");
                true
            }
            AppCommand::ApplyConfig(cfg) => {
                let old_lang = self.config.language.clone();
                let old_vad = self.config.vad.enabled;
                let old_wake = self.config.wake_mode;

                self.config = *cfg;
                let _ = self.config.save();

                if self.config.vad.enabled != old_vad {
                    self.vad_enabled
                        .store(self.config.vad.enabled, Ordering::SeqCst);
                    crate::input::hotkeys::set_vad_enabled(self.config.vad.enabled);
                    crate::ui::tray::set_vad_state(self.config.vad.enabled);
                }
                if self.config.wake_mode != old_wake {
                    crate::ui::tray::set_wake_state(self.config.wake_mode);
                }
                if self.config.language != old_lang {
                    crate::lang::load_locale(&self.config.language);
                }
                log::info!("Applied: engine_mode={}, trailing_space={}, lang={}, dark={}",
                    self.config.engine_mode,
                    self.config.text_fix.trailing_space,
                    self.config.language,
                    self.config.dark_mode);
                true
            }
            AppCommand::EditUserDict => { self.on_edit_user_dict(); true }
            AppCommand::AddHallEntry { phrase } => {
                if !phrase.trim().is_empty() {
                    crate::text::hallucinations::add_custom_phrase(&phrase);
                }
                true
            }
            AppCommand::EditHallDict => { self.on_edit_hall_dict(); true }
            AppCommand::Quit => {
                crate::ui::tray::request_exit();
                false
            }
            _ => { log::debug!("Команда: {:?}", cmd); true }
        }
    }

    fn on_open_settings(&mut self) {
        // Если процесс настроек ещё жив — не запускаем второй
        if let Some(ref mut child) = self.settings_process {
            match child.try_wait() {
                Ok(None) => {
                    log::info!("Окно настроек уже открыто");
                    return;
                }
                _ => {} // процесс завершился, можно запустить новый
            }
        }
        // Запускаем отдельное приложение настроек
        let settings_exe = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("voxmim-settings.exe")))
            .unwrap_or_else(|| std::path::PathBuf::from("voxmim-settings.exe"));
        if settings_exe.exists() {
            match std::process::Command::new(&settings_exe).spawn() {
                Ok(child) => {
                    self.settings_process = Some(child);
                    log::info!("Окно настроек (отдельное приложение)");
                }
                Err(e) => log::error!("Не удалось запустить настройки: {e}"),
            }
        }
    }

    fn on_edit_hall_dict(&self) {
        let path = crate::config::dicts_path().join("hallucinations.txt");
        if !path.exists() {
            if let Err(e) = std::fs::write(&path, "субтитры создавал\nпродолжение следует\n") {
                log::error!("Не удалось создать hallucinations.txt: {e}");
                return;
            }
        }
        match std::process::Command::new("notepad.exe").arg(&path).spawn() {
            Ok(_) => log::info!("Открыт hallucinations.txt в блокноте"),
            Err(e) => log::error!("Не удалось открыть блокнот: {e}"),
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
        if self.state == AppState::Recording {
            // VAD tap-режим: повторный Insert = принудительная остановка
            if self.vad_enabled.load(Ordering::SeqCst) {
                log::info!("▶ Принудительная остановка");
                self.force_stop();
            }
            return;
        }
        if self.state != AppState::Idle { return; }
        self.state = AppState::Recording;
        self.recording.store(true, Ordering::SeqCst);
        let _ = self.audio_buf.lock().unwrap().clear();
        log::info!("▶ Запись началась");
    }

    fn force_stop(&mut self) {
        crate::input::hotkeys::reset_recording_state();
        self.state = AppState::Processing;
        self.recording.store(false, Ordering::SeqCst);
        let samples = {
            let mut buf = self.audio_buf.lock().unwrap();
            std::mem::take(&mut *buf)
        };
        if samples.len() >= 16000 {
            let _ = self.whisper_tx.send(samples);
        } else {
            self.state = AppState::Idle;
        }
    }

    fn on_stop(&mut self) {
        crate::input::hotkeys::reset_recording_state();
        if self.state != AppState::Recording { return; }
        self.state = AppState::Processing;
        self.recording.store(false, Ordering::SeqCst);

        let samples = {
            let mut buf = self.audio_buf.lock().unwrap();
            std::mem::take(&mut *buf)
        };

        log::info!("⏹ Записано ({} сэмплов)", samples.len());
        if samples.len() < 16000 {
            log::warn!("Слишком короткая запись");
            self.state = AppState::Idle;
            return;
        }

        let _ = self.whisper_tx.send(samples);
    }

    fn on_result(&mut self, text: &str) {
        // Игнорируем устаревший результат, если уже началась новая запись
        if self.state == AppState::Recording { return; }
        let text = text.trim();
        if text.is_empty() {
            self.state = AppState::Idle;
            return;
        }

        let fixed = fix_text(text, &self.config.text_fix, &self.user_dict);
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
                if *e != self.config.math_mode {
                    let _ = self.cmd_tx.send(AppCommand::ToggleMathMode);
                }
            }
            CommandAction::None => {}
        }
    }

    fn reload_config(&mut self) {
        let new_cfg = Config::load();
        let old = self.config.clone();
        self.config = new_cfg;

        if self.config.vad.enabled != old.vad.enabled {
            self.vad_enabled.store(self.config.vad.enabled, Ordering::SeqCst);
            crate::input::hotkeys::set_vad_enabled(self.config.vad.enabled);
        }
        if self.config.language != old.language {
            crate::lang::load_locale(&self.config.language);
        }

        let mut changes = Vec::new();
        if old.engine_mode != self.config.engine_mode {
            crate::stt::engine::set_engine_mode_server(self.config.engine_mode == "server");
            changes.push(format!("engine_mode={}→{}", old.engine_mode, self.config.engine_mode));
        }
        if old.detector_mode != self.config.detector_mode {
            changes.push(format!("detector_mode={}→{}", old.detector_mode, self.config.detector_mode));
        }
        if old.use_gpu != self.config.use_gpu {
            changes.push(format!("use_gpu={}→{}", old.use_gpu, self.config.use_gpu));
        }
        if old.wake_mode != self.config.wake_mode {
            changes.push(format!("wake_mode={}→{}", old.wake_mode, self.config.wake_mode));
        }
        if old.vad.enabled != self.config.vad.enabled {
            changes.push(format!("vad.enabled={}→{}", old.vad.enabled, self.config.vad.enabled));
        }
        if old.vad.aggressiveness != self.config.vad.aggressiveness {
            changes.push(format!("vad.aggressiveness={}→{}", old.vad.aggressiveness, self.config.vad.aggressiveness));
        }
        if old.vad.silence_duration_secs != self.config.vad.silence_duration_secs {
            changes.push(format!("vad.timeout={:.1}→{:.1}", old.vad.silence_duration_secs, self.config.vad.silence_duration_secs));
        }
        if old.text_fix.trailing_space != self.config.text_fix.trailing_space {
            changes.push(format!("trail={}→{}", old.text_fix.trailing_space, self.config.text_fix.trailing_space));
        }
        if old.text_fix.fix_hallucinations != self.config.text_fix.fix_hallucinations {
            changes.push(format!("fix_hallucinations={}→{}", old.text_fix.fix_hallucinations, self.config.text_fix.fix_hallucinations));
        }
        if old.text_fix.fix_user_dict != self.config.text_fix.fix_user_dict {
            changes.push(format!("fix_user_dict={}→{}", old.text_fix.fix_user_dict, self.config.text_fix.fix_user_dict));
        }
        if old.text_fix.fix_repetitions != self.config.text_fix.fix_repetitions {
            changes.push(format!("fix_repetitions={}→{}", old.text_fix.fix_repetitions, self.config.text_fix.fix_repetitions));
        }
        if old.text_fix.fix_punctuation != self.config.text_fix.fix_punctuation {
            changes.push(format!("fix_punctuation={}→{}", old.text_fix.fix_punctuation, self.config.text_fix.fix_punctuation));
        }
        if old.math_mode != self.config.math_mode {
            changes.push(format!("math_mode={}→{}", old.math_mode, self.config.math_mode));
        }
        if old.noise_filter_enabled != self.config.noise_filter_enabled {
            changes.push(format!("noise_filter={}→{}", old.noise_filter_enabled, self.config.noise_filter_enabled));
        }
        if old.warmup_on_start != self.config.warmup_on_start {
            changes.push(format!("warmup={}→{}", old.warmup_on_start, self.config.warmup_on_start));
        }
        if old.show_result != self.config.show_result {
            changes.push(format!("show_result={}→{}", old.show_result, self.config.show_result));
        }
        if old.log_enabled != self.config.log_enabled {
            changes.push(format!("log_enabled={}→{}", old.log_enabled, self.config.log_enabled));
        }
        if old.dark_mode != self.config.dark_mode {
            changes.push(format!("dark_mode={}→{}", old.dark_mode, self.config.dark_mode));
        }
        if old.language != self.config.language {
            changes.push(format!("language={}→{}", old.language, self.config.language));
        }
        if old.command_max_words != self.config.command_max_words {
            changes.push(format!("cmd_max_words={}→{}", old.command_max_words, self.config.command_max_words));
        }
        if old.model_path != self.config.model_path {
            changes.push(format!("model_path={}→{}", old.model_path.display(), self.config.model_path.display()));
        }
        if old.detector_model != self.config.detector_model {
            changes.push(format!("detector_model={}→{}", old.detector_model.display(), self.config.detector_model.display()));
        }
        if old.keep_wav != self.config.keep_wav {
            crate::stt::engine::set_keep_wav_global(self.config.keep_wav);
            changes.push(format!("keep_wav={}→{}", old.keep_wav, self.config.keep_wav));
        }

        if changes.is_empty() {
            log::info!("Настройки перезагружены (без изменений)");
        } else {
            log::info!("Изменено: {}. Перезагружаем", changes.join(", "));
        }
    }
}
