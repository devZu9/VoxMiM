//! Микрофоны для окна настроек: список устройств ввода и проверка уровня.
//!
//! cpal 0.15 на Windows (WASAPI) перечисляет ВСЕ активные endpoint'ы —
//! включая динамики и мониторы интерфейса (loopback), которые дают тишину.
//! Поэтому список фильтруется по наличию входного формата.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::StreamConfig;
use std::sync::{Arc, Mutex};

/// Только устройства ввода, без повторов.
pub fn list_inputs() -> Vec<String> {
    let host = cpal::default_host();
    let mut names: Vec<String> = Vec::new();
    if let Ok(devices) = host.input_devices() {
        for device in devices {
            if device.default_input_config().is_err() {
                continue;
            }
            if let Ok(name) = device.name() {
                if !names.contains(&name) {
                    names.push(name);
                }
            }
        }
    }
    names
}

/// Пиковый уровень в дБFS (0 дБ = максимум, −60 и ниже — тишина).
fn peak_dbfs(peak: f32) -> f32 {
    20.0 * peak.max(1e-6).log10()
}

/// Записать `millis` мс с выбранного устройства (`None` — системный по умолчанию)
/// и вернуть пиковый уровень в дБFS.
pub fn test_input(name: Option<&str>, millis: u64) -> Result<f32, String> {
    let host = cpal::default_host();
    let device = match name {
        Some(wanted) => host
            .input_devices()
            .map_err(|e| format!("Нет устройств ввода: {e}"))?
            .find(|d| d.name().ok().as_deref() == Some(wanted) && d.default_input_config().is_ok()),
        None => host.default_input_device(),
    }
    .ok_or_else(|| "Микрофон не найден".to_string())?;

    let cfg = device
        .default_input_config()
        .map_err(|e| format!("Нет входного формата: {e}"))?;
    let stream_cfg: StreamConfig = cfg.config();

    let samples: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let err_text: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    let s = samples.clone();
    let e = err_text.clone();
    let stream = device
        .build_input_stream(
            &stream_cfg,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if let Ok(mut buf) = s.lock() {
                    buf.extend_from_slice(data);
                }
            },
            move |err| {
                if let Ok(mut slot) = e.lock() {
                    *slot = Some(err.to_string());
                }
            },
            None,
        )
        .map_err(|e| format!("Захват не запустился: {e}"))?;

    stream.play().map_err(|e| format!("Захват не запустился: {e}"))?;
    std::thread::sleep(std::time::Duration::from_millis(millis));
    drop(stream);

    if let Some(text) = err_text.lock().ok().and_then(|slot| slot.clone()) {
        return Err(text);
    }
    let peak = samples
        .lock()
        .map(|buf| buf.iter().fold(0.0f32, |acc, v| acc.max(v.abs())))
        .unwrap_or(0.0);
    Ok(peak_dbfs(peak))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peak_dbfs() {
        // Полная шкала — 0 дБ, вдвое тише — примерно −6 дБ
        assert!((peak_dbfs(1.0) - 0.0).abs() < 0.01);
        assert!((peak_dbfs(0.5) + 6.02).abs() < 0.05);
        // Ноль не превращается в минус бесконечность
        assert!(peak_dbfs(0.0) < -100.0);
    }
}
