use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, InputCallbackInfo, Stream, StreamConfig};
use std::sync::mpsc;

pub struct AudioCapture {
    device: Option<Device>,
    stream: Option<Stream>,
    pub sample_rate: u32,
    selected_name: Option<String>,
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

    pub fn list_devices() -> Vec<(String, usize)> {
        let host = cpal::default_host();
        let mut devices = Vec::new();
        if let Ok(input_devices) = host.input_devices() {
            for (i, device) in input_devices.enumerate() {
                if let Ok(name) = device.name() {
                    devices.push((name, i));
                }
            }
        }
        devices
    }

    pub fn select_device(&mut self, name: &str, index: usize) -> Result<(), String> {
        let host = cpal::default_host();
        let devices: Vec<Device> = host
            .input_devices()
            .map_err(|e| format!("Нет устройств ввода: {e}"))?
            .collect();

        // Сначала ищем по имени
        let device = devices.iter().find(|d| d.name().ok().as_deref() == Some(name))
            .cloned()
            .or_else(|| devices.get(index).cloned())
            .ok_or_else(|| {
                format!("Устройство '{name}' не найдено. Доступны: {}",
                    devices.iter().filter_map(|d| d.name().ok()).collect::<Vec<_>>().join(", "))
            })?;

        if let Ok(d_name) = device.name() {
            if d_name != name {
                log::warn!("Микрофон по индексу {index}: '{d_name}' (ожидался '{name}')");
            }
        }

        if let Ok(config) = device.default_input_config() {
            self.sample_rate = config.sample_rate().0;
        }
        self.device = Some(device);
        self.selected_name = Some(name.to_string());
        log::info!("Микрофон: {name} ({} Гц)", self.sample_rate);
        Ok(())
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
