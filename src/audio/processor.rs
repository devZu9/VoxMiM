use crate::app::AppCommand;
use crate::stt::engine::{write_wav, KEEP_WAV};
use crate::vad::detector::{VadDetector, VadEvent};
use chrono::Local;
use crossbeam_channel::Sender;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};

pub struct AudioProcessor;

impl AudioProcessor {
    pub fn spawn(
        audio_rx: mpsc::Receiver<Vec<f32>>,
        cmd_tx: Sender<AppCommand>,
        recording: Arc<AtomicBool>,
        audio_buf: Arc<Mutex<Vec<f32>>>,
        vad_enabled: Arc<AtomicBool>,
        sample_rate: u32,
        vad_threshold: f32,
        vad_silence_secs: f32,
        start_timeout_secs: f32,
    ) {
        std::thread::Builder::new()
            .name("audio-accum".into())
            .spawn(move || {
                let mut vad = VadDetector::new(vad_threshold, vad_silence_secs, sample_rate);
                let ring_max = sample_rate as usize;
                let start_timeout_frames = (start_timeout_secs * sample_rate as f32) as usize;
                let mut ring_buf: Vec<f32> = Vec::new();
                let mut was_ptt_stop = false;
                let mut speech_detected = false;
                let mut total_frames = 0usize;
                let mut vad_accum: Vec<f32> = Vec::new();
                let vad_accum_target = (sample_rate as usize) / 10; // 100ms

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
                            // Копим чанки до ~100мс, потом отдаём VAD
                            vad_accum.extend_from_slice(&chunk);
                            if vad_accum.len() >= vad_accum_target {
                                match vad.process_chunk(&vad_accum) {
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
                                            speech_detected = false;
                                            total_frames = 0;
                                            let _ = cmd_tx.send(AppCommand::StopRecording);
                                            log::info!("Автостоп: запись остановлена по тишине");
                                        } else {
                                            // Пре-речь: ждём начала речи
                                            total_frames += vad_accum.len();
                                            if total_frames >= start_timeout_frames {
                                                recording.store(false, Ordering::SeqCst);
                                                // Если keep_wav включён — сохраняем VAD-буфер для диагностики
                                                if KEEP_WAV.load(Ordering::SeqCst) {
                                                    let samples = audio_buf.lock().unwrap().clone();
                                                    if !samples.is_empty() {
                                                        let ts = Local::now().format("%Y-%m-%d_%H-%M-%S%.3f");
                                                        let dir = std::env::current_exe()
                                                            .ok().and_then(|p| p.parent().map(|p| p.join("wavs")))
                                                            .unwrap_or_else(|| PathBuf::from("wavs"));
                                                        let _ = std::fs::create_dir_all(&dir);
                                                        let path = dir.join(format!("vad_debug_{ts}.wav"));
                                                        if let Err(e) = write_wav(&path, &samples, sample_rate) {
                                                            log::warn!("VAD debug WAV: {e}");
                                                        } else {
                                                            log::info!("VAD debug: сохранён {}", path.display());
                                                        }
                                                    }
                                                }
                                                let _ = audio_buf.lock().unwrap().clear();
                                                total_frames = 0;
                                                let _ = cmd_tx.send(AppCommand::StopRecording);
                                                let rms = (vad_accum.iter().map(|s| s * s).sum::<f32>() / vad_accum.len() as f32).sqrt();
                                                log::info!("Таймаут ожидания речи: {:.1}с тишины без речи (rms={rms:.4})", start_timeout_secs);
                                            }
                                        }
                                    }
                                }
                                vad_accum.clear();
                            }
                        }
                    } else {
                        was_ptt_stop = false;
                        if speech_detected {
                            speech_detected = false;
                            total_frames = 0;
                        }
                        vad_accum.clear();
                    }
                }
            })
            .ok();
    }
}
