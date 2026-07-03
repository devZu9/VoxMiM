use crate::app::AppCommand;
use crate::vad::detector::{VadDetector, VadEvent};
use crossbeam_channel::Sender;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};

pub struct AudioProcessor;

impl AudioProcessor {
    pub fn spawn(
        audio_rx: mpsc::Receiver<Vec<f32>>,
        cmd_tx: Sender<AppCommand>,
        whisper_tx: Sender<Vec<f32>>,
        recording: Arc<AtomicBool>,
        audio_buf: Arc<Mutex<Vec<f32>>>,
        vad_enabled: Arc<AtomicBool>,
        sample_rate: u32,
        vad_aggressiveness: u32,
        vad_silence_secs: f32,
        start_timeout_secs: f32,
    ) {
        std::thread::Builder::new()
            .name("audio-accum".into())
            .spawn(move || {
                let mut vad = VadDetector::new(vad_aggressiveness, vad_silence_secs, sample_rate);
                let ring_max = sample_rate as usize;
                let start_timeout_frames = (start_timeout_secs * sample_rate as f32) as usize;
                let mut ring_buf: Vec<f32> = Vec::new();
                let mut was_ptt_stop = false;
                let mut speech_detected = false;
                let mut total_frames = 0usize;

                while let Ok(chunk) = audio_rx.recv() {
                    ring_buf.extend_from_slice(&chunk);
                    if ring_buf.len() > ring_max {
                        ring_buf.drain(..chunk.len());
                    }

                    let is_recording = recording.load(Ordering::SeqCst);
                    let is_vad = vad_enabled.load(Ordering::SeqCst);

                    if is_recording {
                        if was_ptt_stop && !is_vad {
                            was_ptt_stop = false;
                        }

                        if let Ok(mut b) = audio_buf.lock() {
                            b.extend_from_slice(&chunk);
                        }

                        if is_vad {
                            match vad.process_chunk(&chunk) {
                                VadEvent::SpeechStart => {
                                    speech_detected = true;
                                }
                                VadEvent::Speech => {
                                    if !speech_detected {
                                        speech_detected = true;
                                    }
                                }
                                VadEvent::Silence => {
                                    if speech_detected {
                                        // Пост-речь: тишина превысила silence_duration_secs → автостоп
                                        recording.store(false, Ordering::SeqCst);
                                        let samples = {
                                            let mut b = audio_buf.lock().unwrap();
                                            std::mem::take(&mut *b)
                                        };
                                        if samples.len() >= 16000 {
                                            let _ = whisper_tx.send(samples);
                                        }
                                        speech_detected = false;
                                        total_frames = 0;
                                        let _ = cmd_tx.send(AppCommand::StopRecording);
                                        log::info!("Автостоп: запись остановлена по тишине");
                                    } else {
                                        // Пре-речь: ждём начала речи
                                        total_frames += chunk.len();
                                        if total_frames >= start_timeout_frames {
                                            recording.store(false, Ordering::SeqCst);
                                            let _ = audio_buf.lock().unwrap().clear();
                                            total_frames = 0;
                                            let _ = cmd_tx.send(AppCommand::StopRecording);
                                            log::info!("Таймаут ожидания речи: запись отменена");
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        was_ptt_stop = false;
                        if speech_detected {
                            speech_detected = false;
                            total_frames = 0;
                        }
                    }
                }
            })
            .ok();
    }
}
