//! 任意の音声バイト列 (wav/mp3/ogg/flac/…) を symphonia でチャンネル PCM にデコードする。

use crate::audio::PcmAudio;
use std::io::Cursor;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// 音声バイト列をデコードしてチャンネルごとの f32 PCM にする。
pub fn decode_audio(bytes: &[u8]) -> Result<PcmAudio, String> {
    let mss = MediaSourceStream::new(Box::new(Cursor::new(bytes.to_vec())), Default::default());
    let probed = symphonia::default::get_probe()
        .format(
            &Hint::new(),
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| format!("音声フォーマットを判別できませんでした: {e}"))?;
    let mut format = probed.format;
    let track = format
        .default_track()
        .ok_or("音声トラックが見つかりません")?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("デコーダーを作成できませんでした: {e}"))?;

    let mut sample_rate = track.codec_params.sample_rate.unwrap_or(48000) as f64;
    let mut channels: Vec<Vec<f32>> = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymphoniaError::IoError(_)) => break, // ストリーム終端
            Err(e) => return Err(format!("読み取りエラー: {e}")),
        };
        if packet.track_id() != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(audio_buf) => {
                let spec = *audio_buf.spec();
                sample_rate = spec.rate as f64;
                let num_ch = spec.channels.count();
                if channels.len() < num_ch {
                    channels.resize(num_ch, Vec::new());
                }
                let mut sb = SampleBuffer::<f32>::new(audio_buf.capacity() as u64, spec);
                sb.copy_interleaved_ref(audio_buf);
                let samples = sb.samples();
                let frames = if num_ch > 0 {
                    samples.len() / num_ch
                } else {
                    0
                };
                for f in 0..frames {
                    for (c, channel) in channels.iter_mut().enumerate().take(num_ch) {
                        channel.push(samples[f * num_ch + c]);
                    }
                }
            }
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(_) => break,
        }
    }

    if channels.is_empty() {
        channels.push(Vec::new());
    }
    let frames = channels[0].len();
    Ok(PcmAudio {
        sample_rate,
        channels,
        frames,
    })
}
