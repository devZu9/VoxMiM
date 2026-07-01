#![allow(dead_code)]

#[derive(Debug, Clone, PartialEq)]
pub enum VadEvent {
    Silence,
    SpeechStart,
    Speech,
}

pub struct VadDetector {
    aggressiveness: u32,
    silence_duration_frames: usize,
    silence_frames: usize,
    in_speech: bool,
    sample_rate: u32,
}

impl VadDetector {
    pub fn new(aggressiveness: u32, silence_duration_secs: f32, sample_rate: u32) -> Self {
        let silence_duration_frames = (silence_duration_secs * sample_rate as f32) as usize;
        Self {
            aggressiveness,
            silence_duration_frames,
            silence_frames: 0,
            in_speech: false,
            sample_rate,
        }
    }

    pub fn process_chunk(&mut self, samples: &[f32]) -> VadEvent {
        if samples.is_empty() {
            return VadEvent::Silence;
        }

        let energy = samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32;

        let threshold = match self.aggressiveness {
            0 => 0.01,
            1 => 0.005,
            2 => 0.002,
            _ => 0.001,
        };

        if energy > threshold {
            if !self.in_speech {
                self.in_speech = true;
                self.silence_frames = 0;
                return VadEvent::SpeechStart;
            }
            self.silence_frames = 0;
            VadEvent::Speech
        } else {
            if self.in_speech {
                self.silence_frames += samples.len();
                if self.silence_frames >= self.silence_duration_frames {
                    self.in_speech = false;
                    self.silence_frames = 0;
                    return VadEvent::Silence;
                }
                VadEvent::Speech
            } else {
                VadEvent::Silence
            }
        }
    }

    pub fn reset(&mut self) {
        self.in_speech = false;
        self.silence_frames = 0;
    }

    pub fn set_aggressiveness(&mut self, level: u32) {
        self.aggressiveness = level.clamp(0, 3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_silence() {
        let mut vad = VadDetector::new(1, 0.5, 16000);
        let silent = vec![0.0f32; 1600];
        assert_eq!(vad.process_chunk(&silent), VadEvent::Silence);
    }

    #[test]
    fn test_speech_start() {
        let mut vad = VadDetector::new(1, 0.5, 16000);
        let loud = vec![0.1f32; 1600];
        assert_eq!(vad.process_chunk(&loud), VadEvent::SpeechStart);
    }
}
