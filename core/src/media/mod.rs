//! 音声の入出力。WAV エンコードは常に利用可能。任意の音声デコード (symphonia) と
//! MP3 エンコード (libmp3lame) はそれぞれ feature でゲートする。

pub mod encode;

#[cfg(feature = "decode")]
pub mod decode;

pub use encode::{encode_wav, mp3_compatible_rate, WavBitDepth};

#[cfg(feature = "mp3")]
pub use encode::encode_mp3;

#[cfg(feature = "decode")]
pub use decode::decode_audio;
