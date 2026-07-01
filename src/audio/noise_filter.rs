#![allow(dead_code)]
pub enum NoiseLevel {
    Off,
    Low,
    Moderate,
    High,
    VeryHigh,
}

pub struct NoiseFilter {
    level: NoiseLevel,
    enabled: bool,
}

impl NoiseFilter {
    pub fn new(level: NoiseLevel) -> Self {
        Self {
            enabled: !matches!(level, NoiseLevel::Off),
            level,
        }
    }

    pub fn process(&mut self, samples: &[f32]) -> Vec<f32> {
        if !self.enabled {
            return samples.to_vec();
        }

        let threshold = match self.level {
            NoiseLevel::Low => 0.01,
            NoiseLevel::Moderate => 0.005,
            NoiseLevel::High => 0.002,
            NoiseLevel::VeryHigh => 0.001,
            NoiseLevel::Off => 0.0,
        };

        samples
            .iter()
            .map(|&s| {
                if s.abs() < threshold {
                    0.0
                } else {
                    s
                }
            })
            .collect()
    }

    pub fn set_level(&mut self, level: NoiseLevel) {
        self.enabled = !matches!(level, NoiseLevel::Off);
        self.level = level;
    }

    pub fn calibrate(&mut self, silence_samples: &[f32]) {
        let noise_floor = silence_samples
            .iter()
            .map(|s| s.abs())
            .fold(0.0f32, f32::max);
        log::info!("Калибровка шумоподавления: noise_floor={noise_floor:.6}");
    }
}
