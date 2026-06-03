//! 音声 DSP 一式。各モジュールは TypeScript 実装の純関数を 1:1 で移植したもの。
//! 数値は JS の `number` に合わせて f64 で計算し、PCM バッファ（JS の `Float32Array`）
//! のみ f32 で保持して丸め挙動を一致させる。

pub mod curve;
pub mod envelope;
pub mod filter;
pub mod interpolation;
pub mod lfo;
pub mod mixer;
pub mod oneshot;
pub mod pitch;
pub mod pitchmod;
pub mod polyphony;
pub mod resample;
pub mod reverb;
pub mod timestretch;

pub use mixer::{
    build_waveform_peaks, mix_project, velocity_to_gain, AudioBank, MapBank, MixOptions, MixResult,
    PcmAudio,
};
pub use pitch::{detect_base_pitch, detect_pitch_hz, DetectedPitch};
pub use timestretch::time_stretch;
