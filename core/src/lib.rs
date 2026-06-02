//! 純Rust の MIDI 音MAD 合成コア。
//!
//! Electron/TypeScript 実装から移植した、GUI 非依存のドメインロジック一式:
//! プロジェクトスキーマ、音楽理論、オフラインミキサーを含む音声 DSP、MIDI 取り込み、
//! 音声のデコード/エンコード。Tauri バックエンドからも WASM フロントエンドからも利用できる。

pub mod audio;
pub mod id;
pub mod media;
pub mod midi;
pub mod music;
pub mod schema;

pub use schema::{
    create_empty_project, create_sample, parse_project, Project, Sample, Track, DEFAULT_BASE_PITCH,
    DEFAULT_SAMPLE_RATE,
};
