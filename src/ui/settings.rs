use crate::app::AppCommand;
use crate::config::Config;
use crate::dlog;
use crossbeam_channel::Sender;
use std::sync::Mutex;

slint::include_modules!();

static SETTINGS_WEAK: Mutex<Option<slint::Weak<SettingsWindow>>> = Mutex::new(None);

pub fn init(config: Config, cmd_tx: Sender<AppCommand>) {
    std::thread::Builder::new()
        .name("settings".into())
        .spawn(move || {
            let ui = SettingsWindow::new().unwrap();
            set_config(&ui, &config);

            let ui_weak = ui.as_weak();
            ui.on_apply(move || {
                let ui = ui_weak.unwrap();
                let mut cfg = Config::load();
                cfg.use_gpu = ui.get_use_gpu();
                cfg.engine_mode = if ui.get_engine_server() { "server" } else { "one-shot" }.to_string();
                cfg.detector_mode = if ui.get_det_server() { "server" } else { "one-shot" }.to_string();
                cfg.wake_mode = ui.get_wake_enable();
                cfg.vad.enabled = ui.get_vad_enable();
                cfg.vad.aggressiveness = ui.get_vad_aggr() as u32;
                if let Ok(secs) = ui.get_vad_timeout().trim().parse::<f32>() {
                    cfg.vad.silence_duration_secs = secs;
                }
                cfg.text_fix.fix_hallucinations = ui.get_fix_hallucinations();
                cfg.text_fix.fix_user_dict = ui.get_fix_user_dict();
                cfg.text_fix.fix_repetitions = ui.get_fix_repetitions();
                cfg.text_fix.fix_punctuation = ui.get_fix_punctuation();
                cfg.text_fix.trailing_space = ui.get_trailing_space();
                cfg.math_mode = ui.get_math_mode();
                cfg.noise_filter_enabled = ui.get_noise_filter();
                cfg.warmup_on_start = ui.get_warmup();
                cfg.show_result = ui.get_show_result();
                cfg.log_enabled = ui.get_log_enable();
                cfg.wake_words = ui.get_wake_words().split('\n')
                    .map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                cfg.language = if ui.get_cur_lang() == 1 { "en" } else { "ru" }.to_string();
                if let Ok(n) = ui.get_cmd_max_words().trim().parse::<u32>() {
                    cfg.command_max_words = n;
                }
                if let Err(e) = cfg.save() {
                    dlog!("Settings: save error {e}");
                } else {
                    dlog!("Settings: trailing_space={}, engine={}, lang={}, dark={}, gpu={}",
                        cfg.text_fix.trailing_space, cfg.engine_mode, cfg.language, cfg.dark_mode, cfg.use_gpu);
                }
                let _ = cmd_tx.send(AppCommand::ApplyConfig(Box::new(cfg)));
            });

            // Hide instead of close
            ui.window().on_close_requested(|| {
                slint::CloseRequestResponse::HideWindow
            });

            *SETTINGS_WEAK.lock().unwrap() = Some(ui.as_weak());
            slint::run_event_loop().unwrap();
        })
        .ok();
}

pub fn show() {
    if let Some(weak) = SETTINGS_WEAK.lock().unwrap().as_ref() {
        let weak = weak.clone();
        match slint::invoke_from_event_loop(move || {
            if let Some(ui) = weak.upgrade() {
                ui.window().show().unwrap();
            }
        }) {
            Err(e) => dlog!("Settings: show failed — event loop not running? {e:?}"),
            _ => {}
        }
    }
}

pub fn request_quit() {
    if let Some(weak) = SETTINGS_WEAK.lock().unwrap().take() {
        if let Some(_ui) = weak.upgrade() {
            if let Err(e) = slint::quit_event_loop() { dlog!("Settings: quit error {e}"); }
        }
    }
}

fn set_config(ui: &SettingsWindow, config: &Config) {
    ui.set_engine_server(config.engine_mode == "server");
    ui.set_det_server(config.detector_mode == "server");
    ui.set_use_gpu(config.use_gpu);
    ui.set_wake_enable(config.wake_mode);
    ui.set_vad_enable(config.vad.enabled);
    ui.set_vad_aggr(config.vad.aggressiveness as i32);
    ui.set_vad_timeout(format!("{:.1}", config.vad.silence_duration_secs).into());
    ui.set_fix_hallucinations(config.text_fix.fix_hallucinations);
    ui.set_fix_user_dict(config.text_fix.fix_user_dict);
    ui.set_fix_repetitions(config.text_fix.fix_repetitions);
    ui.set_fix_punctuation(config.text_fix.fix_punctuation);
    ui.set_trailing_space(config.text_fix.trailing_space);
    ui.set_cmd_max_words(config.command_max_words.to_string().into());
    ui.set_math_mode(config.math_mode);
    ui.set_noise_filter(config.noise_filter_enabled);
    ui.set_warmup(config.warmup_on_start);
    ui.set_show_result(config.show_result);
    ui.set_log_enable(config.log_enabled);
    ui.set_wake_words(config.wake_words.join("\n").into());
    ui.set_cur_lang(if config.language == "en" { 1 } else { 0 });
    ui.set_dark_mode(config.dark_mode);
}
