use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, InputCallbackInfo, Stream, StreamConfig};
use std::sync::mpsc;

pub struct AudioCapture {
    device: Option<Device>,
    stream: Option<Stream>,
    pub sample_rate: u32,
    selected_name: Option<String>,
}

// AudioCapture must be Send on macOS for the tray-on-main-thread pattern.
// cpal::Stream is technically !Send on all platforms, but on macOS
// CoreAudio AudioUnit is thread-safe, making this safe.
#[cfg(target_os = "macos")]
unsafe impl Send for AudioCapture {}

/// Микрофон ли это. cpal 0.15 на Windows (WASAPI) перечисляет ВСЕ активные
/// endpoint'ы — включая динамики и мониторы интерфейса (loopback), которые
/// дают тишину. У устройства ввода есть входной формат, у воспроизведения — нет.
fn is_input(device: &Device) -> bool {
    device.default_input_config().is_ok()
}

impl AudioCapture {
    pub fn new() -> Self {
        Self {
            device: None,
            stream: None,
            sample_rate: 16000,
            selected_name: None,
        }
    }

    /// Только устройства ввода. Индекс — позиция в этом списке.
    pub fn list_devices() -> Vec<(String, usize)> {
        let host = cpal::default_host();
        let mut devices = Vec::new();
        if let Ok(input_devices) = host.input_devices() {
            for device in input_devices {
                if !is_input(&device) {
                    continue;
                }
                if let Ok(name) = device.name() {
                    let idx = devices.len();
                    devices.push((name, idx));
                }
            }
        }
        devices
    }

    /// Имя системного микрофона по умолчанию (в настройках Windows).
    pub fn default_device_name() -> Option<String> {
        let host = cpal::default_host();
        let device = host.default_input_device()?;
        if !is_input(&device) {
            return None;
        }
        device.name().ok()
    }

    /// Выбрать системный микрофон по умолчанию.
    pub fn select_default(&mut self) -> Result<(), String> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or("Системный микрофон не найден")?;
        if !is_input(&device) {
            return Err("Системное устройство ввода не найдено".to_string());
        }
        self.apply_device(device)
    }

    pub fn select_device(&mut self, name: &str, index: usize) -> Result<(), String> {
        let host = cpal::default_host();
        let devices: Vec<Device> = host
            .input_devices()
            .map_err(|e| format!("Нет устройств ввода: {e}"))?
            .filter(is_input)
            .collect();

        // Сначала ищем по имени, затем по индексу; не нашли — системный по умолчанию
        let device = devices
            .iter()
            .find(|d| d.name().ok().as_deref() == Some(name))
            .cloned()
            .or_else(|| devices.get(index).cloned());

        let device = match device {
            Some(d) => d,
            None => {
                log::warn!("Микрофон '{name}' не найден — беру системный по умолчанию");
                return self.select_default();
            }
        };

        self.apply_device(device)
    }

    fn apply_device(&mut self, device: Device) -> Result<(), String> {
        let name = device.name().unwrap_or_default();
        if let Ok(config) = device.default_input_config() {
            self.sample_rate = config.sample_rate().0;
        }
        self.device = Some(device);
        self.selected_name = Some(name.clone());
        log::info!("Микрофон: {name} ({} Гц)", self.sample_rate);
        Ok(())
    }

    /// Остановить захват — поток закрывается вместе со `stream`.
    pub fn stop(&mut self) {
        self.stream = None;
    }

    /// Запуск захвата — шлёт чанки аудио в mpsc-канал.
    /// `preferred` — если Some, пробует эту частоту первой (из сохранённой настройки).
    /// Возвращает частоту, на которой удалось запустить захват.
    #[allow(dead_code)]
    pub fn start_capture(&mut self, tx: mpsc::Sender<Vec<f32>>, preferred: Option<u32>) -> Result<u32, String> {
        let txs = vec![tx];
        self.start_capture_multi(txs, preferred)
    }

    /// Запуск захвата с раздачей аудио в несколько каналов (fan-out).
    /// Все получатели получают одни и те же чанки.
    pub fn start_capture_multi(&mut self, txs: Vec<mpsc::Sender<Vec<f32>>>, preferred: Option<u32>) -> Result<u32, String> {
        let device = self.device.as_ref().ok_or("Устройство не выбрано")?;

        let default_cfg = device
            .default_input_config()
            .map_err(|e| format!("Нет конфига: {e}"))?;

        let native_sr = default_cfg.sample_rate().0;
        let channels = default_cfg.channels();

        log::info!("Формат устройства: {} Гц, {} кнл", native_sr, channels);

        // Цепочка частот: preferred → 48000 → 32000 → 24000 → 16000 → 8000
        // Первая успешная — используется
        let fallback = [48000u32, 32000, 24000, 16000, 8000];
        let mut rates = Vec::new();
        if let Some(p) = preferred {
            rates.push(p);
        }
        for &r in &fallback {
            if !rates.contains(&r) {
                rates.push(r);
            }
        }

        for &rate in &rates {
            let cfg = StreamConfig {
                channels: 1,
                sample_rate: cpal::SampleRate(rate),
                buffer_size: cpal::BufferSize::Default,
            };

            let err_fn = |err| log::error!("Ошибка потока: {err}");
            let txs = txs.clone();
            match device.build_input_stream(
                &cfg,
                move |data: &[f32], _: &InputCallbackInfo| {
                    let v = data.to_vec();
                    for tx in &txs {
                        let _ = tx.send(v.clone());
                    }
                },
                err_fn,
                None,
            ) {
                Ok(s) => {
                    if let Err(e) = s.play() {
                        log::warn!("{rate}Hz play: {e}");
                        continue;
                    }
                    self.sample_rate = rate;
                    self.stream = Some(s);
                    log::info!("Аудио-захват: {rate}Hz mono");
                    return Ok(rate);
                }
                Err(e) => {
                    log::warn!("{rate}Hz: {e}");
                }
            }
        }

        Err("Аудио-захват не запущен".to_string())
    }
}
