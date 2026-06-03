use midi2otomad_core::media::encode::encode_wav;

#[cfg(feature = "decode")]
#[test]
fn wav_float_roundtrips_through_decoder() {
    use midi2otomad_core::media::decode_audio;

    let frames = 1024usize;
    let left: Vec<f32> = (0..frames)
        .map(|i| ((i as f64) * 0.05).sin() as f32 * 0.8)
        .collect();
    let right: Vec<f32> = (0..frames)
        .map(|i| ((i as f64) * 0.07).cos() as f32 * 0.5)
        .collect();

    let wav = encode_wav(48000, &left, &right, frames, 32);
    let pcm = decode_audio(&wav).expect("decode wav");

    assert_eq!(pcm.sample_rate, 48000.0);
    assert!(pcm.channels.len() >= 2, "stereo expected");
    assert!(
        (pcm.frames as i64 - frames as i64).abs() <= 2,
        "frame count {} should match {}",
        pcm.frames,
        frames
    );
    let mid = frames / 2;
    assert!((pcm.channels[0][mid] - left[mid]).abs() < 1e-5);
    assert!((pcm.channels[1][mid] - right[mid]).abs() < 1e-5);
}

#[cfg(feature = "mp3")]
#[test]
fn mp3_encodes_to_valid_stream() {
    use midi2otomad_core::media::encode_mp3;

    let frames = 48000usize;
    let left: Vec<f32> = (0..frames)
        .map(|i| ((i as f64) * 2.0 * std::f64::consts::PI * 440.0 / 48000.0).sin() as f32 * 0.5)
        .collect();
    let right = left.clone();

    let mp3 = encode_mp3(48000, &left, &right, frames, 320).expect("encode mp3");
    assert!(
        mp3.len() > 1000,
        "mp3 should have content, got {} bytes",
        mp3.len()
    );

    let starts_with_id3 = mp3.starts_with(b"ID3");
    let starts_with_sync = mp3.len() >= 2 && mp3[0] == 0xFF && (mp3[1] & 0xE0) == 0xE0;
    assert!(
        starts_with_id3 || starts_with_sync,
        "not a recognizable MP3 stream"
    );
}

#[cfg(all(feature = "decode", feature = "mp3"))]
#[test]
fn mp3_decodes_back_to_similar_length() {
    use midi2otomad_core::media::{decode_audio, encode_mp3};

    let frames = 24000usize;
    let left: Vec<f32> = (0..frames)
        .map(|i| ((i as f64) * 2.0 * std::f64::consts::PI * 220.0 / 48000.0).sin() as f32 * 0.5)
        .collect();
    let right = left.clone();

    let mp3 = encode_mp3(48000, &left, &right, frames, 256).expect("encode mp3");
    let decoded = decode_audio(&mp3).expect("decode mp3");
    assert_eq!(decoded.sample_rate, 48000.0);
    assert!(decoded.frames > frames / 2);
}
