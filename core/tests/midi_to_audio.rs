//! MIDI 取り込みから音声出力までの縦断テスト。midly で組んだ SMF バイト列を
//! `midi_to_project` で取り込み、素材を割り当てて `mix_project` でレンダリングし、
//! WAV へエンコードする現実的なフローを検証する。

use std::collections::HashMap;

use midi2otomad_core::audio::{mix_project, MixOptions, PcmAudio};
use midi2otomad_core::media::encode_wav;
use midi2otomad_core::midi::midi_to_project;
use midi2otomad_core::schema::parse_project;

use midly::num::{u15, u24, u28, u7};
use midly::{Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};

fn delta(d: u32) -> u28 {
    u28::new(d)
}

fn midi_event(d: u32, message: MidiMessage) -> TrackEvent<'static> {
    TrackEvent {
        delta: delta(d),
        kind: TrackEventKind::Midi {
            channel: 0.into(),
            message,
        },
    }
}

fn note_on(d: u32, key: u8, vel: u8) -> TrackEvent<'static> {
    midi_event(
        d,
        MidiMessage::NoteOn {
            key: u7::new(key),
            vel: u7::new(vel),
        },
    )
}

fn note_off(d: u32, key: u8) -> TrackEvent<'static> {
    midi_event(
        d,
        MidiMessage::NoteOff {
            key: u7::new(key),
            vel: u7::new(0),
        },
    )
}

fn eot() -> TrackEvent<'static> {
    TrackEvent {
        delta: delta(0),
        kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
    }
}

fn write_smf(format: Format, tracks: Vec<Vec<TrackEvent>>) -> Vec<u8> {
    let smf = Smf {
        header: Header::new(format, Timing::Metrical(u15::new(480))),
        tracks,
    };
    let mut bytes = Vec::new();
    smf.write(&mut bytes).unwrap();
    bytes
}

fn const_pcm(rate: f64, frames: usize, value: f32) -> PcmAudio {
    PcmAudio {
        sample_rate: rate,
        channels: vec![vec![value; frames]],
        frames,
    }
}

#[test]
fn imports_then_renders_audible_mix() {
    // 120 BPM, 480 ppq の 3 音メロディ。
    let track = vec![
        TrackEvent {
            delta: delta(0),
            kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::new(500_000))),
        },
        note_on(0, 60, 100),
        note_off(480, 60),
        note_on(0, 64, 100),
        note_off(480, 64),
        note_on(0, 67, 100),
        note_off(480, 67),
        eot(),
    ];
    let bytes = write_smf(Format::SingleTrack, vec![track]);

    // 既存プロジェクト（素材付き）を previous に渡し、取り込みトラックへ素材を継承させる。
    let previous = parse_project(serde_json::json!({
        "version": 1, "name": "prev", "sampleRate": 48000,
        "samples": [{ "id": "kit", "name": "kit", "basePitch": 60,
            "envelope": { "attackMs": 1, "releaseMs": 10 } }]
    }))
    .unwrap();

    let imported = midi_to_project(&bytes, "melody.mid", Some(&previous)).unwrap();
    assert_eq!(imported.track_count, 1);
    assert_eq!(imported.note_count, 3);
    let project = imported.project;
    assert_eq!(project.tracks[0].default_sample_id.as_deref(), Some("kit"));

    let mut bank = HashMap::new();
    bank.insert("kit".to_string(), const_pcm(48000.0, 48000, 0.6));

    let mix = mix_project(&project, &bank, &MixOptions::default());
    assert!(mix.peak > 0.0);
    assert!(mix.left.iter().all(|v| v.is_finite()));
    // メロディは 3 拍 = 1.5 秒 @120BPM。フレーム数はそれ以上。
    assert!(mix.frames as f64 / mix.sample_rate >= 1.5);

    let wav = encode_wav(
        mix.sample_rate as u32,
        &mix.left,
        &mix.right,
        mix.frames,
        24,
    );
    assert_eq!(&wav[0..4], b"RIFF");
    assert_eq!(&wav[8..12], b"WAVE");
}

#[test]
fn parallel_tracks_become_separate_project_tracks() {
    let lead = vec![note_on(0, 72, 100), note_off(480, 72), eot()];
    let bass = vec![note_on(0, 36, 100), note_off(960, 36), eot()];
    let bytes = write_smf(Format::Parallel, vec![lead, bass]);

    let imported = midi_to_project(&bytes, "duet.mid", None).unwrap();
    assert_eq!(imported.track_count, 2);
    assert_eq!(imported.note_count, 2);
    // 別々の midi_index を持つ。
    let idxs: Vec<_> = imported
        .project
        .tracks
        .iter()
        .filter_map(|t| t.midi_index)
        .collect();
    assert_eq!(idxs.len(), 2);
    assert_ne!(idxs[0], idxs[1]);
}

#[test]
fn imported_project_passes_validation() {
    let track = vec![note_on(0, 60, 90), note_off(240, 60), eot()];
    let bytes = write_smf(Format::SingleTrack, vec![track]);
    let imported = midi_to_project(&bytes, "v.mid", None).unwrap();
    // 取り込み結果は常にスキーマ検証を通過する（midi_to_project 内で検証済み）。
    imported.project.validate().expect("imported project valid");
}

#[test]
fn unassigned_samples_render_silently_but_safely() {
    // previous 無しで取り込むと素材が割り当たらず、ミックスは無音だが安全。
    let track = vec![note_on(0, 60, 100), note_off(480, 60), eot()];
    let bytes = write_smf(Format::SingleTrack, vec![track]);
    let project = midi_to_project(&bytes, "x.mid", None).unwrap().project;
    let mix = mix_project(&project, &HashMap::new(), &MixOptions::default());
    assert_eq!(mix.peak, 0.0);
    assert!(mix.left.iter().all(|&v| v == 0.0));
}
