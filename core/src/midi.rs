//! MIDI 取り込み。`midly` で SMF を解析し、トラック・ノート・テンポ・コントロールチェンジ
//! (CC7 ボリューム / CC11 エクスプレッション) をプロジェクトへ変換する。既存プロジェクトを
//! 渡すと素材ライブラリ・マスター設定・リバーブ送り・ポリフォニーを引き継ぐ。

use crate::schema::{
    AutomationPoint, Note, Polyphony, Project, Sample, Tempo, Track, TrackDynamics,
};
use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};
use std::collections::HashMap;

const TRACK_PALETTE: [&str; 10] = [
    "#7c5cff", "#36d399", "#f87272", "#fbbd23", "#3abff8", "#e879f9", "#f97316", "#22d3ee",
    "#a3e635", "#fb7185",
];

const DEFAULT_TEMPO_US: u32 = 500_000; // 120 BPM

pub struct MidiImportResult {
    pub project: Project,
    pub track_count: usize,
    pub note_count: usize,
}

fn clamp01(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

fn strip_extension(file_name: &str) -> String {
    let lower = file_name.to_ascii_lowercase();
    if lower.ends_with(".midi") {
        file_name[..file_name.len() - 5].to_string()
    } else if lower.ends_with(".mid") {
        file_name[..file_name.len() - 4].to_string()
    } else {
        file_name.to_string()
    }
}

/// 絶対 tick を秒へ変換するテンポマップ。
struct TimeMap {
    ppq: f64,
    /// (tick, 累積秒, このセグメントの µs/四分音符) を tick 昇順で。
    segments: Vec<(u64, f64, f64)>,
    /// タイムコード時の 1 秒あたり tick 数（Metrical のときは None）。
    ticks_per_second: Option<f64>,
}

impl TimeMap {
    fn new(timing: &Timing, tempo_changes: &[(u64, u32)]) -> Self {
        match timing {
            Timing::Metrical(ppq) => {
                let ppq = ppq.as_int() as f64;
                let mut changes: Vec<(u64, u32)> = tempo_changes.to_vec();
                changes.sort_by_key(|(tick, _)| *tick);
                let mut segments: Vec<(u64, f64, f64)> = vec![(0, 0.0, DEFAULT_TEMPO_US as f64)];
                for &(tick, us) in &changes {
                    let (last_tick, last_sec, last_us) = *segments.last().unwrap();
                    let sec = if tick <= last_tick {
                        // 同一 tick での再設定はテンポのみ更新。
                        segments.last_mut().unwrap().2 = us as f64;
                        continue;
                    } else {
                        last_sec + (tick - last_tick) as f64 / ppq * (last_us / 1_000_000.0)
                    };
                    segments.push((tick, sec, us as f64));
                }
                Self {
                    ppq,
                    segments,
                    ticks_per_second: None,
                }
            }
            Timing::Timecode(fps, subframe) => Self {
                ppq: 480.0,
                segments: vec![(0, 0.0, DEFAULT_TEMPO_US as f64)],
                ticks_per_second: Some(fps.as_f32() as f64 * *subframe as f64),
            },
        }
    }

    fn seconds_at(&self, tick: u64) -> f64 {
        if let Some(tps) = self.ticks_per_second {
            return tick as f64 / tps;
        }
        let mut seg = &self.segments[0];
        for s in &self.segments {
            if s.0 <= tick {
                seg = s;
            } else {
                break;
            }
        }
        seg.1 + (tick - seg.0) as f64 / self.ppq * (seg.2 / 1_000_000.0)
    }
}

struct RawNote {
    pitch: i32,
    start_tick: u64,
    end_tick: u64,
    velocity: i32,
}

struct RawTrack {
    midi_index: usize,
    name: String,
    notes: Vec<RawNote>,
    volume: Vec<(u64, f64)>,
    expression: Vec<(u64, f64)>,
}

pub fn midi_to_project(
    bytes: &[u8],
    file_name: &str,
    previous: Option<&Project>,
) -> Result<MidiImportResult, String> {
    let smf = Smf::parse(bytes).map_err(|e| format!("MIDI の解析に失敗しました: {e}"))?;

    // 1. テンポ変化を全トラックから絶対 tick で集める。
    let mut tempo_changes: Vec<(u64, u32)> = Vec::new();
    for track in &smf.tracks {
        let mut tick: u64 = 0;
        for event in track {
            tick += event.delta.as_int() as u64;
            if let TrackEventKind::Meta(MetaMessage::Tempo(us)) = event.kind {
                tempo_changes.push((tick, us.as_int()));
            }
        }
    }
    let time_map = TimeMap::new(&smf.header.timing, &tempo_changes);

    // 2. 各トラックからノートと CC を抽出。
    let mut raw_tracks: Vec<RawTrack> = Vec::new();
    for (midi_index, track) in smf.tracks.iter().enumerate() {
        let mut tick: u64 = 0;
        let mut name = String::new();
        let mut notes: Vec<RawNote> = Vec::new();
        let mut volume: Vec<(u64, f64)> = Vec::new();
        let mut expression: Vec<(u64, f64)> = Vec::new();
        let mut active: HashMap<(u8, u8), Vec<(u64, u8)>> = HashMap::new();

        for event in track {
            tick += event.delta.as_int() as u64;
            match event.kind {
                TrackEventKind::Meta(MetaMessage::TrackName(raw)) => {
                    if name.is_empty() {
                        name = String::from_utf8_lossy(raw).to_string();
                    }
                }
                TrackEventKind::Midi { channel, message } => match message {
                    MidiMessage::NoteOn { key, vel } => {
                        if vel.as_int() > 0 {
                            active
                                .entry((channel.as_int(), key.as_int()))
                                .or_default()
                                .push((tick, vel.as_int()));
                        } else {
                            close_note(
                                &mut active,
                                &mut notes,
                                channel.as_int(),
                                key.as_int(),
                                tick,
                            );
                        }
                    }
                    MidiMessage::NoteOff { key, .. } => {
                        close_note(
                            &mut active,
                            &mut notes,
                            channel.as_int(),
                            key.as_int(),
                            tick,
                        );
                    }
                    MidiMessage::Controller { controller, value } => match controller.as_int() {
                        7 => volume.push((tick, value.as_int() as f64 / 127.0)),
                        11 => expression.push((tick, value.as_int() as f64 / 127.0)),
                        _ => {}
                    },
                    _ => {}
                },
                _ => {}
            }
        }

        if !notes.is_empty() {
            raw_tracks.push(RawTrack {
                midi_index,
                name: name.trim().to_string(),
                notes,
                volume,
                expression,
            });
        }
    }

    // 3. 前プロジェクトからの引き継ぎ。
    let previous_samples: Vec<Sample> = previous.map(|p| p.samples.clone()).unwrap_or_default();
    let fallback_sample_id = previous_samples.first().map(|s| s.id.clone());
    let mut previous_sends: HashMap<usize, f64> = HashMap::new();
    let mut previous_polyphony: HashMap<usize, Polyphony> = HashMap::new();
    if let Some(prev) = previous {
        for track in &prev.tracks {
            if let Some(idx) = track.midi_index {
                previous_sends.insert(idx as usize, track.reverb_send);
                previous_polyphony.insert(idx as usize, track.polyphony.clone());
            }
        }
    }

    // 4. プロジェクトのトラックを構築。
    let mut note_count = 0;
    let tracks: Vec<Track> = raw_tracks
        .into_iter()
        .enumerate()
        .map(|(index, raw)| {
            let notes: Vec<Note> = raw
                .notes
                .iter()
                .map(|n| {
                    note_count += 1;
                    Note {
                        pitch: n.pitch.clamp(0, 127),
                        start_sec: time_map.seconds_at(n.start_tick).max(0.0),
                        duration_sec: (time_map.seconds_at(n.end_tick)
                            - time_map.seconds_at(n.start_tick))
                        .max(0.02),
                        velocity: n.velocity.clamp(1, 127),
                    }
                })
                .collect();
            let to_points = |events: &[(u64, f64)]| -> Vec<AutomationPoint> {
                events
                    .iter()
                    .map(|&(tick, v)| AutomationPoint {
                        t: time_map.seconds_at(tick).max(0.0),
                        v: clamp01(v),
                    })
                    .collect()
            };
            let name = if raw.name.is_empty() {
                format!("Track {}", index + 1)
            } else {
                raw.name.clone()
            };
            Track {
                id: crate::id::make_id("track"),
                name,
                midi_index: Some(raw.midi_index as i32),
                color: TRACK_PALETTE[index % TRACK_PALETTE.len()].to_string(),
                muted: false,
                solo: false,
                gain: 1.0,
                pan: 0.0,
                default_sample_id: fallback_sample_id.clone(),
                note_sample_map: HashMap::new(),
                notes,
                dynamics: TrackDynamics {
                    volume: to_points(&raw.volume),
                    expression: to_points(&raw.expression),
                },
                reverb_send: previous_sends.get(&raw.midi_index).copied().unwrap_or(0.0),
                polyphony: previous_polyphony
                    .get(&raw.midi_index)
                    .cloned()
                    .unwrap_or_default(),
            }
        })
        .collect();

    // 5. テンポ配列。
    let tempos: Vec<Tempo> = tempo_changes
        .iter()
        .map(|&(tick, us)| Tempo {
            time_sec: time_map.seconds_at(tick),
            bpm: 60_000_000.0 / us as f64,
        })
        .collect();
    let first_bpm = tempo_changes
        .iter()
        .min_by_key(|(tick, _)| *tick)
        .map(|&(_, us)| 60_000_000.0 / us as f64);

    let track_count = tracks.len();
    let name = {
        let stripped = strip_extension(file_name);
        if stripped.is_empty() {
            "音MAD".to_string()
        } else {
            stripped
        }
    };

    let project = Project {
        version: 1,
        name,
        bpm: first_bpm.or(previous.map(|p| p.bpm)).unwrap_or(140.0),
        ppq: time_map.ppq as i32,
        sample_rate: previous.map(|p| p.sample_rate).unwrap_or(48000),
        master_gain: previous.map(|p| p.master_gain).unwrap_or(1.0),
        tempos,
        samples: previous_samples,
        tracks,
        reverb: previous.map(|p| p.reverb.clone()).unwrap_or_default(),
        output: previous
            .map(|p| p.output.clone())
            .unwrap_or_default(),
    };
    project.validate()?;

    Ok(MidiImportResult {
        project,
        track_count,
        note_count,
    })
}

fn close_note(
    active: &mut HashMap<(u8, u8), Vec<(u64, u8)>>,
    notes: &mut Vec<RawNote>,
    channel: u8,
    key: u8,
    end_tick: u64,
) {
    if let Some(stack) = active.get_mut(&(channel, key)) {
        if !stack.is_empty() {
            let (start_tick, velocity) = stack.remove(0);
            notes.push(RawNote {
                pitch: key as i32,
                start_tick,
                end_tick,
                velocity: velocity as i32,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use midly::num::{u15, u24, u28, u7};
    use midly::{Format, Header, MidiMessage, TrackEvent, TrackEventKind};

    fn write_smf(events: Vec<TrackEvent>) -> Vec<u8> {
        let header = Header::new(Format::SingleTrack, Timing::Metrical(u15::new(480)));
        let smf = Smf {
            header,
            tracks: vec![events],
        };
        let mut bytes = Vec::new();
        smf.write(&mut bytes).unwrap();
        bytes
    }

    fn delta(d: u32) -> u28 {
        u28::new(d)
    }

    #[test]
    fn imports_notes_and_tempo() {
        // 120 BPM (500000 µs/beat), 480 ppq → 1 beat = 0.5s. ノート: tick0→480 (0.5s) と tick480→960.
        let events = vec![
            TrackEvent {
                delta: delta(0),
                kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::new(500_000))),
            },
            TrackEvent {
                delta: delta(0),
                kind: TrackEventKind::Meta(MetaMessage::TrackName(b"Lead")),
            },
            TrackEvent {
                delta: delta(0),
                kind: TrackEventKind::Midi {
                    channel: 0.into(),
                    message: MidiMessage::NoteOn {
                        key: u7::new(60),
                        vel: u7::new(100),
                    },
                },
            },
            TrackEvent {
                delta: delta(480),
                kind: TrackEventKind::Midi {
                    channel: 0.into(),
                    message: MidiMessage::NoteOff {
                        key: u7::new(60),
                        vel: u7::new(0),
                    },
                },
            },
            TrackEvent {
                delta: delta(0),
                kind: TrackEventKind::Midi {
                    channel: 0.into(),
                    message: MidiMessage::NoteOn {
                        key: u7::new(64),
                        vel: u7::new(80),
                    },
                },
            },
            TrackEvent {
                delta: delta(480),
                kind: TrackEventKind::Midi {
                    channel: 0.into(),
                    message: MidiMessage::NoteOff {
                        key: u7::new(64),
                        vel: u7::new(0),
                    },
                },
            },
            TrackEvent {
                delta: delta(0),
                kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
            },
        ];
        let bytes = write_smf(events);
        let result = midi_to_project(&bytes, "song.mid", None).unwrap();
        assert_eq!(result.track_count, 1);
        assert_eq!(result.note_count, 2);
        let track = &result.project.tracks[0];
        assert_eq!(track.name, "Lead");
        assert_eq!(track.notes.len(), 2);
        assert_eq!(track.notes[0].pitch, 60);
        assert!((track.notes[0].start_sec - 0.0).abs() < 1e-6);
        assert!((track.notes[0].duration_sec - 0.5).abs() < 1e-3);
        assert!((track.notes[1].start_sec - 0.5).abs() < 1e-3);
        assert_eq!(track.notes[1].velocity, 80);
        assert_eq!(result.project.ppq, 480);
        assert!((result.project.bpm - 120.0).abs() < 1e-6);
        assert_eq!(result.project.name, "song");
    }

    #[test]
    fn drops_empty_tracks_and_strips_extension() {
        let events = vec![TrackEvent {
            delta: delta(0),
            kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
        }];
        let bytes = write_smf(events);
        let result = midi_to_project(&bytes, "empty.midi", None).unwrap();
        assert_eq!(result.track_count, 0);
        assert_eq!(result.note_count, 0);
        assert_eq!(result.project.name, "empty");
    }

    #[test]
    fn preserves_samples_from_previous() {
        let previous = crate::schema::parse_project(serde_json::json!({
            "version": 1, "name": "prev", "sampleRate": 44100, "masterGain": 0.8,
            "samples": [{ "id": "kept", "name": "kick" }]
        }))
        .unwrap();
        let events = vec![
            TrackEvent {
                delta: delta(0),
                kind: TrackEventKind::Midi {
                    channel: 0.into(),
                    message: MidiMessage::NoteOn {
                        key: u7::new(60),
                        vel: u7::new(100),
                    },
                },
            },
            TrackEvent {
                delta: delta(240),
                kind: TrackEventKind::Midi {
                    channel: 0.into(),
                    message: MidiMessage::NoteOff {
                        key: u7::new(60),
                        vel: u7::new(0),
                    },
                },
            },
            TrackEvent {
                delta: delta(0),
                kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
            },
        ];
        let bytes = write_smf(events);
        let result = midi_to_project(&bytes, "x.mid", Some(&previous)).unwrap();
        assert_eq!(result.project.samples.len(), 1);
        assert_eq!(result.project.samples[0].id, "kept");
        assert_eq!(result.project.sample_rate, 44100);
        assert_eq!(
            result.project.tracks[0].default_sample_id.as_deref(),
            Some("kept")
        );
    }
}
