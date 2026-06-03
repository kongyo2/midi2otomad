pub mod encode;

#[cfg(feature = "decode")]
pub mod decode;

pub use encode::{encode_wav, mp3_compatible_rate, WavBitDepth};

#[cfg(feature = "mp3")]
pub use encode::encode_mp3;

#[cfg(feature = "decode")]
pub use decode::decode_audio;
