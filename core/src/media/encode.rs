#[cfg(feature = "mp3")]
use crate::audio::resample::resample_channel;

pub type WavBitDepth = u16;

const MP3_SAMPLE_RATES: [u32; 9] = [8000, 11025, 12000, 16000, 22050, 24000, 32000, 44100, 48000];

pub fn mp3_compatible_rate(rate: u32) -> u32 {
    if MP3_SAMPLE_RATES.contains(&rate) {
        return rate;
    }
    for &supported in MP3_SAMPLE_RATES.iter().rev() {
        let ratio = rate as f64 / supported as f64;
        let rounded = ratio.round() as u64;
        if ratio > 1.0 && (ratio - rounded as f64).abs() < 1e-9 && rounded.is_power_of_two() {
            return supported;
        }
    }
    for &supported in MP3_SAMPLE_RATES.iter().rev() {
        if supported <= rate {
            return supported;
        }
    }
    MP3_SAMPLE_RATES[0]
}

fn clamp(x: f64) -> f64 {
    if !x.is_finite() {
        0.0
    } else {
        x.clamp(-1.0, 1.0)
    }
}

#[cfg(feature = "mp3")]
fn full_channel(channel: &[f32], frames: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; frames];
    let n = channel.len().min(frames);
    out[..n].copy_from_slice(&channel[..n]);
    out
}

struct Lcg(u64);

impl Lcg {
    fn new() -> Self {
        Self(0x2545_F491_4F6C_DD1D)
    }
    fn next_unit(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
    fn triangular(&mut self) -> f64 {
        self.next_unit() - self.next_unit()
    }
}

pub fn encode_wav(
    sample_rate: u32,
    left: &[f32],
    right: &[f32],
    frames: usize,
    bit_depth: WavBitDepth,
) -> Vec<u8> {
    let channels: u16 = 2;
    let bytes_per_sample = (bit_depth / 8) as usize;
    let block_align = channels as usize * bytes_per_sample;
    let data_size = frames * block_align;
    let is_float = bit_depth == 32;
    let fmt_size: u32 = if is_float { 18 } else { 16 };
    let fact_size: u32 = if is_float { 12 } else { 0 };
    let header_size: u32 = 12 + (8 + fmt_size) + fact_size + 8;

    let mut buf = Vec::with_capacity(header_size as usize + data_size);
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&(header_size - 8 + data_size as u32).to_le_bytes());
    buf.extend_from_slice(b"WAVE");

    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&fmt_size.to_le_bytes());
    buf.extend_from_slice(&(if is_float { 3u16 } else { 1u16 }).to_le_bytes());
    buf.extend_from_slice(&channels.to_le_bytes());
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&(sample_rate * block_align as u32).to_le_bytes());
    buf.extend_from_slice(&(block_align as u16).to_le_bytes());
    buf.extend_from_slice(&bit_depth.to_le_bytes());
    if is_float {
        buf.extend_from_slice(&0u16.to_le_bytes());
        buf.extend_from_slice(b"fact");
        buf.extend_from_slice(&4u32.to_le_bytes());
        buf.extend_from_slice(&(frames as u32).to_le_bytes());
    }

    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&(data_size as u32).to_le_bytes());

    let dither = bit_depth == 16;
    let peak_int: i64 = if bit_depth == 24 { 0x7f_ffff } else { 0x7fff };
    let mut rng = Lcg::new();
    for i in 0..frames {
        for c in 0..2 {
            let src = if c == 0 { left } else { right };
            let x = clamp(*src.get(i).unwrap_or(&0.0) as f64);
            if is_float {
                buf.extend_from_slice(&(x as f32).to_le_bytes());
            } else {
                let mut scaled = x * peak_int as f64;
                if dither {
                    scaled += rng.triangular();
                }
                let mut v = (scaled.round() as i64).clamp(-peak_int - 1, peak_int);
                if bit_depth == 24 {
                    if v < 0 {
                        v += 0x100_0000;
                    }
                    let vv = (v & 0xff_ffff) as u32;
                    buf.push((vv & 0xff) as u8);
                    buf.push(((vv >> 8) & 0xff) as u8);
                    buf.push(((vv >> 16) & 0xff) as u8);
                } else {
                    buf.extend_from_slice(&(v as i16).to_le_bytes());
                }
            }
        }
    }
    buf
}

#[cfg(feature = "mp3")]
fn bitrate_from_kbps(kbps: u32) -> mp3lame_encoder::Bitrate {
    use mp3lame_encoder::Bitrate;
    match kbps {
        0..=160 => Bitrate::Kbps160,
        161..=192 => Bitrate::Kbps192,
        193..=224 => Bitrate::Kbps224,
        225..=256 => Bitrate::Kbps256,
        _ => Bitrate::Kbps320,
    }
}

#[cfg(feature = "mp3")]
pub fn encode_mp3(
    sample_rate: u32,
    left: &[f32],
    right: &[f32],
    frames: usize,
    kbps: u32,
) -> Result<Vec<u8>, String> {
    use mp3lame_encoder::{Builder, DualPcm, FlushNoGap};

    let target = mp3_compatible_rate(sample_rate);
    let (l, r, rate) = if target == sample_rate {
        (
            full_channel(left, frames),
            full_channel(right, frames),
            sample_rate,
        )
    } else {
        let l = resample_channel(
            &full_channel(left, frames),
            sample_rate as f64,
            target as f64,
        );
        let r = resample_channel(
            &full_channel(right, frames),
            sample_rate as f64,
            target as f64,
        );
        (l, r, target)
    };
    let n = l.len();

    let mut builder = Builder::new().ok_or("LAME ビルダーの初期化に失敗しました")?;
    builder.set_num_channels(2).map_err(|e| e.to_string())?;
    builder.set_sample_rate(rate).map_err(|e| e.to_string())?;
    builder
        .set_brate(bitrate_from_kbps(kbps))
        .map_err(|e| e.to_string())?;
    builder
        .set_quality(mp3lame_encoder::Quality::Best)
        .map_err(|e| e.to_string())?;
    let mut encoder = builder.build().map_err(|e| e.to_string())?;

    let mut out: Vec<u8> = Vec::with_capacity(mp3lame_encoder::max_required_buffer_size(n));
    let input = DualPcm {
        left: l.as_slice(),
        right: r.as_slice(),
    };
    let encoded = encoder
        .encode(input, out.spare_capacity_mut())
        .map_err(|e| e.to_string())?;
    unsafe { out.set_len(out.len() + encoded) };
    let flushed = encoder
        .flush::<FlushNoGap>(out.spare_capacity_mut())
        .map_err(|e| e.to_string())?;
    unsafe { out.set_len(out.len() + flushed) };
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_u32(buf: &[u8], at: usize) -> u32 {
        u32::from_le_bytes([buf[at], buf[at + 1], buf[at + 2], buf[at + 3]])
    }

    #[test]
    fn mp3_rate_mapping() {
        assert_eq!(mp3_compatible_rate(48000), 48000);
        assert_eq!(mp3_compatible_rate(44100), 44100);
        assert_eq!(mp3_compatible_rate(96000), 48000);
        assert_eq!(mp3_compatible_rate(88200), 44100);
        assert_eq!(mp3_compatible_rate(50000), 48000);
        assert_eq!(mp3_compatible_rate(5000), 8000);
    }

    #[test]
    fn wav_24bit_header() {
        let left = vec![0.5f32, -0.5, 1.0, -1.0];
        let right = vec![0.0f32, 0.25, -0.25, 0.1];
        let wav = encode_wav(48000, &left, &right, 4, 24);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[12..16], b"fmt ");
        assert_eq!(read_u32(&wav, 16), 16);
        assert_eq!(u16::from_le_bytes([wav[20], wav[21]]), 1);
        assert_eq!(u16::from_le_bytes([wav[22], wav[23]]), 2);
        assert_eq!(read_u32(&wav, 24), 48000);
        assert_eq!(&wav[36..40], b"data");
        let data_size = read_u32(&wav, 40) as usize;
        assert_eq!(data_size, 4 * 2 * 3);
        assert_eq!(wav.len(), 44 + data_size);
    }

    #[test]
    fn wav_32bit_float_roundtrip() {
        let left = vec![0.5f32, -0.5, 0.123, -0.999];
        let right = vec![0.0f32, 0.25, -0.25, 0.1];
        let wav = encode_wav(44100, &left, &right, 4, 32);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(read_u32(&wav, 16), 18);
        assert_eq!(u16::from_le_bytes([wav[20], wav[21]]), 3);
        assert_eq!(&wav[38..42], b"fact");
        let data_start = 58;
        let first = f32::from_le_bytes([
            wav[data_start],
            wav[data_start + 1],
            wav[data_start + 2],
            wav[data_start + 3],
        ]);
        assert!((first - 0.5).abs() < 1e-6);
    }

    #[test]
    fn wav_clamps_out_of_range() {
        let left = vec![2.0f32, f32::NAN];
        let right = vec![-3.0f32, f32::INFINITY];
        let wav = encode_wav(48000, &left, &right, 2, 32);
        let data_start = 58;
        let first = f32::from_le_bytes([
            wav[data_start],
            wav[data_start + 1],
            wav[data_start + 2],
            wav[data_start + 3],
        ]);
        assert_eq!(first, 1.0);
    }

    fn read_i16(buf: &[u8], at: usize) -> i16 {
        i16::from_le_bytes([buf[at], buf[at + 1]])
    }

    #[test]
    fn mp3_rate_mapping_extended() {
        assert_eq!(mp3_compatible_rate(192000), 48000);
        assert_eq!(mp3_compatible_rate(176400), 44100);
        assert_eq!(mp3_compatible_rate(64000), 32000);
        assert_eq!(mp3_compatible_rate(384000), 48000);
        assert_eq!(mp3_compatible_rate(144000), 48000);
        assert_eq!(mp3_compatible_rate(132300), 48000);
        assert_eq!(mp3_compatible_rate(4000), 8000);
        assert_eq!(mp3_compatible_rate(1), 8000);
        for r in [8000, 11025, 12000, 16000, 22050, 24000, 32000, 44100, 48000] {
            assert_eq!(mp3_compatible_rate(r), r);
        }
    }

    #[test]
    fn wav_16bit_header_has_no_fact_chunk() {
        let wav = encode_wav(48000, &[0.0; 4], &[0.0; 4], 4, 16);
        assert_eq!(read_u32(&wav, 16), 16);
        assert_eq!(u16::from_le_bytes([wav[20], wav[21]]), 1);
        assert_eq!(u16::from_le_bytes([wav[34], wav[35]]), 16);
        assert_eq!(&wav[36..40], b"data");
        let data_size = read_u32(&wav, 40) as usize;
        assert_eq!(data_size, 4 * 2 * 2);
        assert_eq!(wav.len(), 44 + data_size);
    }

    #[test]
    fn wav_16bit_encodes_expected_levels() {
        let wav = encode_wav(48000, &[1.0, -1.0], &[1.0, -1.0], 2, 16);
        let data_start = 44;
        let l0 = read_i16(&wav, data_start);
        let r0 = read_i16(&wav, data_start + 2);
        assert!(l0 >= 0x7ffe);
        assert!(r0 >= 0x7ffe);
        let l1 = read_i16(&wav, data_start + 4);
        assert!(l1 <= -0x7ffe);
    }

    #[test]
    fn wav_16bit_is_deterministic() {
        let left: Vec<f32> = (0..64)
            .map(|i| (i as f64 * 0.13).sin() as f32 * 0.6)
            .collect();
        let right: Vec<f32> = (0..64)
            .map(|i| (i as f64 * 0.17).cos() as f32 * 0.6)
            .collect();
        let a = encode_wav(48000, &left, &right, 64, 16);
        let b = encode_wav(48000, &left, &right, 64, 16);
        assert_eq!(a, b);
    }

    #[test]
    fn wav_24bit_encodes_signed_values() {
        let wav = encode_wav(48000, &[0.5, -0.5], &[0.0, 0.0], 2, 24);
        let data_start = 44;
        let pos = (wav[data_start] as i32)
            | ((wav[data_start + 1] as i32) << 8)
            | ((wav[data_start + 2] as i32) << 16);
        assert!((pos - 0x3f_ffff).abs() <= 1);

        let neg_at = data_start + 6;
        assert!(wav[neg_at + 2] & 0x80 != 0);
    }

    #[test]
    fn wav_interleaves_left_and_right() {
        let wav = encode_wav(48000, &[0.25, 0.5], &[-0.25, -0.5], 2, 32);
        let read_f32 =
            |at: usize| f32::from_le_bytes([wav[at], wav[at + 1], wav[at + 2], wav[at + 3]]);
        let data_start = 58;
        assert!((read_f32(data_start) - 0.25).abs() < 1e-6);
        assert!((read_f32(data_start + 4) - (-0.25)).abs() < 1e-6);
        assert!((read_f32(data_start + 8) - 0.5).abs() < 1e-6);
        assert!((read_f32(data_start + 12) - (-0.5)).abs() < 1e-6);
    }

    #[test]
    fn wav_pads_short_channel_with_silence() {
        let wav = encode_wav(48000, &[0.5, 0.5, 0.5], &[0.5], 3, 32);
        let read_f32 =
            |at: usize| f32::from_le_bytes([wav[at], wav[at + 1], wav[at + 2], wav[at + 3]]);
        let data_start = 58;
        assert_eq!(read_f32(data_start + 12), 0.0);
    }

    #[cfg(feature = "mp3")]
    #[test]
    fn mp3_resamples_unsupported_rate() {
        let frames = 5000usize;
        let tone: Vec<f32> = (0..frames)
            .map(|i| (i as f64 * 2.0 * std::f64::consts::PI * 440.0 / 50000.0).sin() as f32 * 0.4)
            .collect();
        let mp3 = encode_mp3(50000, &tone, &tone, frames, 192).expect("encode mp3");
        assert!(mp3.len() > 500);
    }

    #[cfg(feature = "mp3")]
    #[test]
    fn mp3_handles_supported_rate_without_resample() {
        let frames = 4000usize;
        let tone: Vec<f32> = (0..frames)
            .map(|i| (i as f64 * 2.0 * std::f64::consts::PI * 330.0 / 44100.0).sin() as f32 * 0.4)
            .collect();
        let mp3 = encode_mp3(44100, &tone, &tone, frames, 128).expect("encode mp3");
        assert!(mp3.len() > 500);
    }
}
