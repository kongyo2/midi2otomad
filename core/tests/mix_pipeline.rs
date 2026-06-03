use std::collections::HashMap;

use midi2otomad_core::audio::{build_waveform_peaks, mix_project, MixOptions, PcmAudio};
use midi2otomad_core::media::encode_wav;
use midi2otomad_core::schema::{parse_project, Project};
use serde_json::json;

fn sine_pcm(rate: f64, frames: usize, freq: f64, amp: f32) -> PcmAudio {
    let ch: Vec<f32> = (0..frames)
        .map(|i| ((2.0 * std::f64::consts::PI * freq * i as f64) / rate).sin() as f32 * amp)
        .collect();
    PcmAudio {
        sample_rate: rate,
        channels: vec![ch],
        frames,
    }
}

fn const_pcm(rate: f64, frames: usize, value: f32) -> PcmAudio {
    PcmAudio {
        sample_rate: rate,
        channels: vec![vec![value; frames]],
        frames,
    }
}

fn one_note_project(rate: i32) -> Project {
    parse_project(json!({
        "version": 1, "name": "pipeline", "sampleRate": rate, "masterGain": 1.0,
        "samples": [{
            "id": "s1", "name": "tone", "basePitch": 60, "gain": 1.0, "durationSec": 1.0,
            "envelope": { "attackMs": 5, "releaseMs": 20 }
        }],
        "tracks": [{
            "id": "t1", "name": "lead", "defaultSampleId": "s1",
            "notes": [{ "pitch": 60, "startSec": 0.0, "durationSec": 0.5, "velocity": 110 }]
        }]
    }))
    .unwrap()
}

fn bank(id: &str, pcm: PcmAudio) -> HashMap<String, PcmAudio> {
    let mut b = HashMap::new();
    b.insert(id.to_string(), pcm);
    b
}

#[test]
fn mix_produces_audible_stereo() {
    let project = one_note_project(48000);
    let b = bank("s1", sine_pcm(48000.0, 48000, 440.0, 0.8));
    let mix = mix_project(&project, &b, &MixOptions::default());
    assert_eq!(mix.sample_rate, 48000.0);
    assert_eq!(mix.left.len(), mix.frames);
    assert_eq!(mix.right.len(), mix.frames);
    assert!(mix.peak > 0.0);
    assert!(mix.left.iter().all(|v| v.is_finite()));
    assert!(mix.frames > 24000);
}

#[cfg(feature = "decode")]
#[test]
fn mix_survives_wav_roundtrip_at_each_bit_depth() {
    use midi2otomad_core::media::decode_audio;

    let project = one_note_project(48000);
    let b = bank("s1", sine_pcm(48000.0, 48000, 440.0, 0.7));
    let mix = mix_project(&project, &b, &MixOptions::default());

    for depth in [16u16, 24, 32] {
        let wav = encode_wav(
            mix.sample_rate as u32,
            &mix.left,
            &mix.right,
            mix.frames,
            depth,
        );
        let decoded = decode_audio(&wav).expect("decode mix wav");
        assert_eq!(decoded.sample_rate, 48000.0);
        assert!(decoded.channels.len() >= 2);
        assert!((decoded.frames as i64 - mix.frames as i64).abs() <= 2);
        let energy: f64 = decoded.channels[0]
            .iter()
            .map(|&v| (v as f64) * (v as f64))
            .sum();
        assert!(energy > 0.0, "decoded {depth}bit mix was silent");
    }
}

#[test]
fn mix_is_deterministic() {
    let project = one_note_project(48000);
    let b = bank("s1", sine_pcm(48000.0, 48000, 330.0, 0.6));
    let a = mix_project(&project, &b, &MixOptions::default());
    let c = mix_project(&project, &b, &MixOptions::default());
    assert_eq!(a.frames, c.frames);
    assert_eq!(a.left, c.left);
    assert_eq!(a.right, c.right);
    assert_eq!(a.peak, c.peak);
}

#[test]
fn empty_project_mixes_to_silence() {
    let project =
        parse_project(json!({ "version": 1, "name": "empty", "sampleRate": 48000 })).unwrap();
    let mix = mix_project(&project, &HashMap::new(), &MixOptions::default());
    assert_eq!(mix.peak, 0.0);
    assert!(mix.left.iter().all(|&v| v == 0.0));
    assert!(mix.frames >= 1);
}

#[test]
fn reverb_send_extends_the_tail() {
    let dry = parse_project(json!({
        "version": 1, "name": "dry", "sampleRate": 48000,
        "reverb": { "enabled": false },
        "samples": [{ "id": "s1", "name": "t", "envelope": { "attackMs": 0, "releaseMs": 0 } }],
        "tracks": [{ "id": "t1", "name": "l", "defaultSampleId": "s1", "reverbSend": 0.0,
            "notes": [{ "pitch": 60, "startSec": 0, "durationSec": 0.05, "velocity": 127 }] }]
    }))
    .unwrap();
    let wet = parse_project(json!({
        "version": 1, "name": "wet", "sampleRate": 48000,
        "reverb": { "enabled": true, "roomSize": 0.9, "wet": 1.0, "dry": 1.0 },
        "samples": [{ "id": "s1", "name": "t", "envelope": { "attackMs": 0, "releaseMs": 0 } }],
        "tracks": [{ "id": "t1", "name": "l", "defaultSampleId": "s1", "reverbSend": 1.0,
            "notes": [{ "pitch": 60, "startSec": 0, "durationSec": 0.05, "velocity": 127 }] }]
    }))
    .unwrap();
    let b = bank("s1", const_pcm(48000.0, 4800, 1.0));
    let dry_mix = mix_project(&dry, &b, &MixOptions::default());
    let wet_mix = mix_project(&wet, &b, &MixOptions::default());
    assert!(wet_mix.duration_sec > dry_mix.duration_sec + 1.0);
}

#[test]
fn waveform_peaks_track_amplitude_envelope() {
    let mut ch = vec![0.05f32; 1000];
    for v in ch.iter_mut().skip(500) {
        *v = 0.9;
    }
    let pcm = PcmAudio {
        sample_rate: 1000.0,
        channels: vec![ch],
        frames: 1000,
    };
    let peaks = build_waveform_peaks(&pcm, 10);
    assert_eq!(peaks.len(), 10);
    assert!(peaks[0] < 0.1);
    assert!(peaks[9] > 0.8);
}

#[test]
fn limiter_option_overrides_project_setting() {
    let project = parse_project(json!({
        "version": 1, "name": "lim", "sampleRate": 1000,
        "output": { "limiter": { "enabled": true, "threshold": 0.5 } },
        "samples": [{ "id": "s1", "name": "t", "envelope": { "attackMs": 0, "releaseMs": 0 } }],
        "tracks": [{ "id": "t1", "name": "l", "defaultSampleId": "s1",
            "notes": [{ "pitch": 60, "startSec": 0, "durationSec": 0.5, "velocity": 127 }] }]
    }))
    .unwrap();
    let b = bank("s1", const_pcm(1000.0, 1000, 1.0));

    let limited = mix_project(&project, &b, &MixOptions::default());
    let bypassed = mix_project(
        &project,
        &b,
        &MixOptions {
            tail_sec: None,
            limiter: Some(false),
        },
    );
    let lim_max = limited.left.iter().fold(0.0f32, |p, &v| p.max(v.abs()));
    let byp_max = bypassed.left.iter().fold(0.0f32, |p, &v| p.max(v.abs()));
    assert!(byp_max > lim_max);
}
