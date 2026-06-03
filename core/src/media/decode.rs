use crate::audio::PcmAudio;
use std::io::Cursor;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

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
            Err(SymphoniaError::IoError(_)) => break,
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
                let frames = samples.len().checked_div(num_ch).unwrap_or(0);
                for channel in channels.iter_mut().take(num_ch) {
                    channel.reserve(frames);
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::encode::encode_wav;

    fn mono_wav_16(sample_rate: u32, samples: &[i16]) -> Vec<u8> {
        let data_size = samples.len() * 2;
        let mut buf = Vec::new();
        buf.extend_from_slice(b"RIFF");
        buf.extend_from_slice(&((36 + data_size) as u32).to_le_bytes());
        buf.extend_from_slice(b"WAVE");
        buf.extend_from_slice(b"fmt ");
        buf.extend_from_slice(&16u32.to_le_bytes());
        buf.extend_from_slice(&1u16.to_le_bytes());
        buf.extend_from_slice(&1u16.to_le_bytes());
        buf.extend_from_slice(&sample_rate.to_le_bytes());
        buf.extend_from_slice(&(sample_rate * 2).to_le_bytes());
        buf.extend_from_slice(&2u16.to_le_bytes());
        buf.extend_from_slice(&16u16.to_le_bytes());
        buf.extend_from_slice(b"data");
        buf.extend_from_slice(&(data_size as u32).to_le_bytes());
        for &s in samples {
            buf.extend_from_slice(&s.to_le_bytes());
        }
        buf
    }

    fn stereo_tone(frames: usize) -> (Vec<f32>, Vec<f32>) {
        let left: Vec<f32> = (0..frames)
            .map(|i| (i as f64 * 0.05).sin() as f32 * 0.8)
            .collect();
        let right: Vec<f32> = (0..frames)
            .map(|i| (i as f64 * 0.07).cos() as f32 * 0.5)
            .collect();
        (left, right)
    }

    #[test]
    fn rejects_garbage_bytes() {
        assert!(decode_audio(&[1, 2, 3, 4, 5, 6, 7, 8]).is_err());
    }

    #[test]
    fn rejects_empty_input() {
        assert!(decode_audio(&[]).is_err());
    }

    #[test]
    fn rejects_truncated_riff_header() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&4u32.to_le_bytes());
        bytes.extend_from_slice(b"WAVE");
        assert!(decode_audio(&bytes).is_err());
    }

    #[test]
    fn decodes_16bit_stereo_roundtrip() {
        let frames = 512usize;
        let (left, right) = stereo_tone(frames);
        let wav = encode_wav(44100, &left, &right, frames, 16);
        let pcm = decode_audio(&wav).expect("decode 16bit wav");

        assert_eq!(pcm.sample_rate, 44100.0);
        assert!(pcm.channels.len() >= 2);
        assert!((pcm.frames as i64 - frames as i64).abs() <= 2);
        let mid = frames / 2;
        assert!((pcm.channels[0][mid] - left[mid]).abs() < 2e-4);
        assert!((pcm.channels[1][mid] - right[mid]).abs() < 2e-4);
    }

    #[test]
    fn decodes_24bit_stereo_roundtrip() {
        let frames = 512usize;
        let (left, right) = stereo_tone(frames);
        let wav = encode_wav(48000, &left, &right, frames, 24);
        let pcm = decode_audio(&wav).expect("decode 24bit wav");

        assert_eq!(pcm.sample_rate, 48000.0);
        assert!(pcm.channels.len() >= 2);
        let mid = frames / 2;
        assert!((pcm.channels[0][mid] - left[mid]).abs() < 1e-4);
        assert!((pcm.channels[1][mid] - right[mid]).abs() < 1e-4);
    }

    #[test]
    fn decodes_mono_into_single_channel() {
        let samples: Vec<i16> = (0..256)
            .map(|i| ((i as f64 * 0.1).sin() * 16000.0) as i16)
            .collect();
        let wav = mono_wav_16(22050, &samples);
        let pcm = decode_audio(&wav).expect("decode mono wav");

        assert_eq!(pcm.sample_rate, 22050.0);
        assert_eq!(pcm.channels.len(), 1);
        assert!((pcm.frames as i64 - samples.len() as i64).abs() <= 2);
        assert!(pcm.channels[0].iter().all(|v| v.is_finite()));
    }

    #[test]
    fn reports_duration_from_decoded_frames() {
        let frames = 480usize;
        let (left, right) = stereo_tone(frames);
        let wav = encode_wav(48000, &left, &right, frames, 16);
        let pcm = decode_audio(&wav).expect("decode");
        assert!((pcm.duration_sec() - 0.01).abs() < 1e-3);
    }
}
