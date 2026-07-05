#![allow(dead_code)]

#[derive(Debug, Clone, PartialEq)]
pub enum VadEvent {
    Silence,
    SpeechStart,
    Speech,
}

pub struct VadDetector {
    threshold: f32,
    silence_duration_frames: usize,
    silence_frames: usize,
    in_speech: bool,
    sample_rate: u32,
}

impl VadDetector {
    pub fn new(threshold: f32, silence_duration_secs: f32, sample_rate: u32) -> Self {
        let silence_duration_frames = (silence_duration_secs * sample_rate as f32) as usize;
        Self {
            threshold,
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

        let rms = (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt();

        if rms > self.threshold {
            if !self.in_speech {
                self.in_speech = true;
                self.silence_frames = 0;
                log::info!("[VAD] SpeechStart (rms={rms:.4})");
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
                    log::info!("[VAD] Silence (rms={rms:.4}, frame_count={})", self.silence_duration_frames);
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

    pub fn set_threshold(&mut self, threshold: f32) {
        self.threshold = threshold.clamp(0.002, 0.05);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_silence() {
        let mut vad = VadDetector::new(0.008, 0.5, 16000);
        let silent = vec![0.0f32; 1600];
        assert_eq!(vad.process_chunk(&silent), VadEvent::Silence);
    }

    #[test]
    fn test_speech_start() {
        let mut vad = VadDetector::new(0.008, 0.5, 16000);
        let loud = vec![0.1f32; 1600]; // RMS=0.1 > threshold 0.008 → SpeechStart
        assert_eq!(vad.process_chunk(&loud), VadEvent::SpeechStart);
    }
}
