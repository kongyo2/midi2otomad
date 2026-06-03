use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use midi2otomad_core::audio::resample::resample_channel;

struct Shared {
    samples: Mutex<Vec<f32>>,
    pos: AtomicUsize,
    playing: AtomicBool,
    level: AtomicU32,
}

impl Shared {
    fn frame_len(&self) -> usize {
        self.samples.lock().map(|s| s.len() / 2).unwrap_or(0)
    }
}

pub struct Player {
    shared: Arc<Shared>,
    engine_rate: u32,
}

#[derive(serde::Serialize, Clone, Copy)]
pub struct PlayerStatus {
    pub playing: bool,
    pub position: f64,
    pub duration: f64,
    pub level: f32,
}

impl Player {
    pub fn new() -> Result<Self, String> {
        let shared = Arc::new(Shared {
            samples: Mutex::new(Vec::new()),
            pos: AtomicUsize::new(0),
            playing: AtomicBool::new(false),
            level: AtomicU32::new(0),
        });
        let shared_for_thread = shared.clone();
        let (tx, rx) = std::sync::mpsc::channel::<Result<u32, String>>();

        std::thread::spawn(move || match build_stream(shared_for_thread) {
            Ok((stream, rate)) => {
                if let Err(e) = stream.play() {
                    let _ = tx.send(Err(e.to_string()));
                    return;
                }
                let _ = tx.send(Ok(rate));
                loop {
                    std::thread::park();
                }
            }
            Err(e) => {
                let _ = tx.send(Err(e));
            }
        });

        let engine_rate = rx
            .recv()
            .map_err(|_| "オーディオスレッドの起動に失敗しました".to_string())??;
        Ok(Self {
            shared,
            engine_rate,
        })
    }

    pub fn set_mix(&self, left: &[f32], right: &[f32], mix_rate: f64) {
        let interleaved = if (mix_rate - self.engine_rate as f64).abs() < 1e-6 {
            interleave(left, right)
        } else {
            let l = resample_channel(left, mix_rate, self.engine_rate as f64);
            let r = resample_channel(right, mix_rate, self.engine_rate as f64);
            interleave(&l, &r)
        };
        let resume = self
            .shared
            .pos
            .load(Ordering::Acquire)
            .min(interleaved.len() / 2);
        if let Ok(mut buf) = self.shared.samples.lock() {
            *buf = interleaved;
        }
        self.shared.pos.store(resume, Ordering::Release);
    }

    pub fn play(&self, from_sec: Option<f64>) {
        if let Some(sec) = from_sec {
            self.seek(sec);
        }
        let len = self.shared.frame_len();
        if self.shared.pos.load(Ordering::Acquire) >= len {
            self.shared.pos.store(0, Ordering::Release);
        }
        self.shared.playing.store(true, Ordering::Release);
    }

    pub fn pause(&self) {
        self.shared.playing.store(false, Ordering::Release);
    }

    pub fn stop(&self) {
        self.shared.playing.store(false, Ordering::Release);
        self.shared.pos.store(0, Ordering::Release);
    }

    pub fn seek(&self, sec: f64) {
        let frame = (sec.max(0.0) * self.engine_rate as f64).round() as usize;
        let len = self.shared.frame_len();
        self.shared.pos.store(frame.min(len), Ordering::Release);
    }

    pub fn status(&self) -> PlayerStatus {
        let len = self.shared.frame_len();
        let pos = self.shared.pos.load(Ordering::Acquire);
        let rate = self.engine_rate as f64;
        PlayerStatus {
            playing: self.shared.playing.load(Ordering::Acquire),
            position: pos as f64 / rate,
            duration: len as f64 / rate,
            level: f32::from_bits(self.shared.level.load(Ordering::Relaxed)),
        }
    }
}

fn interleave(left: &[f32], right: &[f32]) -> Vec<f32> {
    let n = left.len().max(right.len());
    let mut out = Vec::with_capacity(n * 2);
    if left.len() == right.len() {
        for (&l, &r) in left.iter().zip(right.iter()) {
            out.push(l);
            out.push(r);
        }
    } else {
        for i in 0..n {
            out.push(left.get(i).copied().unwrap_or(0.0));
            out.push(right.get(i).copied().unwrap_or(0.0));
        }
    }
    out
}

fn build_stream(shared: Arc<Shared>) -> Result<(cpal::Stream, u32), String> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| "出力デバイスが見つかりません".to_string())?;
    let supported = device.default_output_config().map_err(|e| e.to_string())?;
    let sample_format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let rate = config.sample_rate.0;
    let channels = config.channels as usize;

    let stream = match sample_format {
        cpal::SampleFormat::F32 => run::<f32>(&device, &config, shared, channels),
        cpal::SampleFormat::I16 => run::<i16>(&device, &config, shared, channels),
        cpal::SampleFormat::U16 => run::<u16>(&device, &config, shared, channels),
        other => return Err(format!("未対応のサンプル形式です: {other:?}")),
    }?;
    Ok((stream, rate))
}

fn run<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    shared: Arc<Shared>,
    channels: usize,
) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    let err_fn = |e| eprintln!("オーディオストリームエラー: {e}");
    device
        .build_output_stream(
            config,
            move |out: &mut [T], _: &cpal::OutputCallbackInfo| {
                let silence = T::from_sample(0.0f32);
                let guard = shared.samples.try_lock();
                let samples = match guard {
                    Ok(ref s) => s.as_slice(),
                    Err(_) => {
                        for s in out.iter_mut() {
                            *s = silence;
                        }
                        return;
                    }
                };
                let frame_len = samples.len() / 2;
                let mut peak = 0.0f32;
                for frame in out.chunks_mut(channels.max(1)) {
                    if shared.playing.load(Ordering::Acquire) {
                        let pos = shared.pos.load(Ordering::Acquire);
                        if pos < frame_len {
                            let l = samples[pos * 2];
                            let r = samples[pos * 2 + 1];
                            peak = peak.max(l.abs()).max(r.abs());
                            for (i, s) in frame.iter_mut().enumerate() {
                                let v = if channels == 1 {
                                    (l + r) * 0.5
                                } else if i == 0 {
                                    l
                                } else if i == 1 {
                                    r
                                } else {
                                    0.0
                                };
                                *s = T::from_sample(v);
                            }
                            let _ = shared.pos.compare_exchange(
                                pos,
                                pos + 1,
                                Ordering::Release,
                                Ordering::Relaxed,
                            );
                        } else {
                            shared.playing.store(false, Ordering::Release);
                            for s in frame.iter_mut() {
                                *s = silence;
                            }
                        }
                    } else {
                        for s in frame.iter_mut() {
                            *s = silence;
                        }
                    }
                }
                let prev = f32::from_bits(shared.level.load(Ordering::Relaxed));
                let level = peak.max(prev * 0.85);
                shared.level.store(level.to_bits(), Ordering::Relaxed);
            },
            err_fn,
            None,
        )
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::interleave;

    #[test]
    fn interleaves_equal_length_channels() {
        assert_eq!(
            interleave(&[1.0, 2.0, 3.0], &[4.0, 5.0, 6.0]),
            vec![1.0, 4.0, 2.0, 5.0, 3.0, 6.0]
        );
    }

    #[test]
    fn pads_shorter_channel_with_zero() {
        assert_eq!(interleave(&[1.0, 2.0], &[3.0]), vec![1.0, 3.0, 2.0, 0.0]);
        assert_eq!(interleave(&[1.0], &[3.0, 4.0]), vec![1.0, 3.0, 0.0, 4.0]);
    }

    #[test]
    fn empty_channels_yield_empty() {
        assert!(interleave(&[], &[]).is_empty());
    }
}
