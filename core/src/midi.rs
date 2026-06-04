use crate::schema::{
    AutomationPoint, Note, Polyphony, Project, Sample, Tempo, Track, TrackDynamics,
};
use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};
use std::collections::HashMap;

const TRACK_PALETTE: [&str; 10] = [
    "#ff8a3d", "#c8f24e", "#57cfd6", "#ff6b8a", "#ffc24a", "#8ad36b", "#7aa2ff", "#e879c0",
    "#ff9e64", "#6ee0b0",
];

const DEFAULT_TEMPO_US: u32 = 500_000;
const GM_DRUM_CHANNEL: u8 = 9;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImportMode {
    Normal,
    Drum,
    #[default]
    Auto,
}

impl ImportMode {
    pub fn from_str_or_auto(value: &str) -> Self {
        match value {
            "normal" => ImportMode::Normal,
            "drum" => ImportMode::Drum,
            _ => ImportMode::Auto,
        }
    }
}

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

struct TimeMap {
    ppq: f64,
    segments: Vec<(u64, f64, f64)>,
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
    is_drum: bool,
}

struct RawTrack {
    midi_index: usize,
    name: String,
    notes: Vec<RawNote>,
    volume: Vec<(u64, f64)>,
    expression: Vec<(u64, f64)>,
    all_drum: bool,
}

type ActiveNotes = HashMap<(u8, u8), Vec<(u64, u8, bool)>>;

struct TrackSpec {
    notes: Vec<RawNote>,
    name: String,
    midi_index: usize,
    drum_mode: bool,
    volume: Vec<(u64, f64)>,
    expression: Vec<(u64, f64)>,
}

struct PreservedTrack {
    reverb_send: f64,
    polyphony: Polyphony,
    dynamics_depth: f64,
}

pub fn midi_to_project(
    bytes: &[u8],
    file_name: &str,
    previous: Option<&Project>,
) -> Result<MidiImportResult, String> {
    midi_to_project_with_mode(bytes, file_name, previous, ImportMode::Auto)
}

pub fn midi_to_project_with_mode(
    bytes: &[u8],
    file_name: &str,
    previous: Option<&Project>,
    mode: ImportMode,
) -> Result<MidiImportResult, String> {
    let smf = Smf::parse(bytes).map_err(|e| format!("MIDI の解析に失敗しました: {e}"))?;

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

    let mut raw_tracks: Vec<RawTrack> = Vec::new();
    for (midi_index, track) in smf.tracks.iter().enumerate() {
        let mut tick: u64 = 0;
        let mut name = String::new();
        let mut notes: Vec<RawNote> = Vec::new();
        let mut volume: Vec<(u64, f64)> = Vec::new();
        let mut expression: Vec<(u64, f64)> = Vec::new();
        let mut active: ActiveNotes = HashMap::new();
        let mut bank_msb: HashMap<u8, u8> = HashMap::new();

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
                            let ch = channel.as_int();
                            let is_drum = ch == GM_DRUM_CHANNEL || bank_msb.get(&ch) == Some(&127);
                            active.entry((ch, key.as_int())).or_default().push((
                                tick,
                                vel.as_int(),
                                is_drum,
                            ));
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
                        0 => {
                            bank_msb.insert(channel.as_int(), value.as_int());
                        }
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
            let all_drum = notes.iter().all(|n| n.is_drum);
            raw_tracks.push(RawTrack {
                midi_index,
                name: name.trim().to_string(),
                notes,
                volume,
                expression,
                all_drum,
            });
        }
    }

    let previous_samples: Vec<Sample> = previous.map(|p| p.samples.clone()).unwrap_or_default();
    let fallback_sample_id = previous_samples.first().map(|s| s.id.clone());
    let mut previous_tracks: HashMap<(usize, bool), PreservedTrack> = HashMap::new();
    if let Some(prev) = previous {
        for track in &prev.tracks {
            if let Some(idx) = track.midi_index {
                previous_tracks.insert(
                    (idx as usize, track.drum_mode),
                    PreservedTrack {
                        reverb_send: track.reverb_send,
                        polyphony: track.polyphony.clone(),
                        dynamics_depth: track.dynamics_depth,
                    },
                );
            }
        }
    }

    let mut specs: Vec<TrackSpec> = Vec::new();
    for raw in raw_tracks {
        let RawTrack {
            midi_index,
            name,
            notes,
            volume,
            expression,
            all_drum,
        } = raw;
        let mixed = mode == ImportMode::Auto
            && notes.iter().any(|n| n.is_drum)
            && notes.iter().any(|n| !n.is_drum);
        if mixed {
            let (drum_notes, melodic_notes): (Vec<RawNote>, Vec<RawNote>) =
                notes.into_iter().partition(|n| n.is_drum);
            let drum_name = if name.is_empty() {
                "ドラム".to_string()
            } else {
                format!("{name} (ドラム)")
            };
            specs.push(TrackSpec {
                notes: melodic_notes,
                name,
                midi_index,
                drum_mode: false,
                volume: volume.clone(),
                expression: expression.clone(),
            });
            specs.push(TrackSpec {
                notes: drum_notes,
                name: drum_name,
                midi_index,
                drum_mode: true,
                volume,
                expression,
            });
        } else {
            let drum_mode = match mode {
                ImportMode::Normal => false,
                ImportMode::Drum => true,
                ImportMode::Auto => all_drum,
            };
            specs.push(TrackSpec {
                notes,
                name,
                midi_index,
                drum_mode,
                volume,
                expression,
            });
        }
    }

    let mut note_count = 0;
    let tracks: Vec<Track> = specs
        .into_iter()
        .enumerate()
        .map(|(index, spec)| {
            let notes: Vec<Note> = spec
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
            let name = if spec.name.is_empty() {
                format!("Track {}", index + 1)
            } else {
                spec.name.clone()
            };
            let preserved = previous_tracks.get(&(spec.midi_index, spec.drum_mode));
            Track {
                id: crate::id::make_id("track"),
                name,
                midi_index: Some(spec.midi_index as i32),
                color: TRACK_PALETTE[index % TRACK_PALETTE.len()].to_string(),
                muted: false,
                solo: false,
                gain: 1.0,
                pan: 0.0,
                default_sample_id: fallback_sample_id.clone(),
                drum_mode: spec.drum_mode,
                note_sample_map: HashMap::new(),
                notes,
                dynamics: TrackDynamics {
                    volume: to_points(&spec.volume),
                    expression: to_points(&spec.expression),
                },
                dynamics_depth: preserved.map(|p| p.dynamics_depth).unwrap_or(1.0),
                reverb_send: preserved.map(|p| p.reverb_send).unwrap_or(0.0),
                polyphony: preserved.map(|p| p.polyphony.clone()).unwrap_or_default(),
            }
        })
        .collect();

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
        output: previous.map(|p| p.output.clone()).unwrap_or_default(),
    };
    project.validate()?;

    Ok(MidiImportResult {
        project,
        track_count,
        note_count,
    })
}

fn close_note(
    active: &mut ActiveNotes,
    notes: &mut Vec<RawNote>,
    channel: u8,
    key: u8,
    end_tick: u64,
) {
    if let Some(stack) = active.get_mut(&(channel, key)) {
        if !stack.is_empty() {
            let (start_tick, velocity, is_drum) = stack.remove(0);
            notes.push(RawNote {
                pitch: key as i32,
                start_tick,
                end_tick,
                velocity: velocity as i32,
                is_drum,
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

    fn midi_event(tick_delta: u32, message: MidiMessage) -> TrackEvent<'static> {
        TrackEvent {
            delta: delta(tick_delta),
            kind: TrackEventKind::Midi {
                channel: 0.into(),
                message,
            },
        }
    }

    fn note_on(tick_delta: u32, key: u8, vel: u8) -> TrackEvent<'static> {
        midi_event(
            tick_delta,
            MidiMessage::NoteOn {
                key: u7::new(key),
                vel: u7::new(vel),
            },
        )
    }

    fn note_off(tick_delta: u32, key: u8) -> TrackEvent<'static> {
        midi_event(
            tick_delta,
            MidiMessage::NoteOff {
                key: u7::new(key),
                vel: u7::new(0),
            },
        )
    }

    fn end_of_track() -> TrackEvent<'static> {
        TrackEvent {
            delta: delta(0),
            kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
        }
    }

    #[test]
    fn velocity_zero_note_on_acts_as_note_off() {
        let events = vec![note_on(0, 60, 100), note_on(480, 60, 0), end_of_track()];
        let result = midi_to_project(&write_smf(events), "x.mid", None).unwrap();
        assert_eq!(result.note_count, 1);
        let note = &result.project.tracks[0].notes[0];
        assert_eq!(note.pitch, 60);
        assert!((note.duration_sec - 0.5).abs() < 1e-3);
    }

    #[test]
    fn imports_control_changes_as_dynamics() {
        let cc = |tick: u32, controller: u8, value: u8| TrackEvent {
            delta: delta(tick),
            kind: TrackEventKind::Midi {
                channel: 0.into(),
                message: MidiMessage::Controller {
                    controller: u7::new(controller),
                    value: u7::new(value),
                },
            },
        };
        let events = vec![
            cc(0, 7, 127),
            cc(0, 11, 64),
            note_on(0, 60, 100),
            note_off(480, 60),
            cc(0, 7, 0),
            end_of_track(),
        ];
        let result = midi_to_project(&write_smf(events), "x.mid", None).unwrap();
        let dyn_ = &result.project.tracks[0].dynamics;
        assert_eq!(dyn_.volume.len(), 2);
        assert!((dyn_.volume[0].v - 1.0).abs() < 1e-9);
        assert!((dyn_.volume[1].v - 0.0).abs() < 1e-9);
        assert_eq!(dyn_.expression.len(), 1);
        assert!((dyn_.expression[0].v - 64.0 / 127.0).abs() < 1e-9);
    }

    #[test]
    fn tempo_change_shifts_later_notes() {
        let events = vec![
            TrackEvent {
                delta: delta(0),
                kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::new(500_000))),
            },
            note_on(0, 60, 100),
            note_off(480, 60),
            TrackEvent {
                delta: delta(0),
                kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::new(250_000))),
            },
            note_on(0, 64, 100),
            note_off(480, 64),
            end_of_track(),
        ];
        let result = midi_to_project(&write_smf(events), "x.mid", None).unwrap();
        let notes = &result.project.tracks[0].notes;
        assert_eq!(notes.len(), 2);
        assert!((notes[0].start_sec - 0.0).abs() < 1e-6);
        assert!((notes[0].duration_sec - 0.5).abs() < 1e-3);
        assert!((notes[1].start_sec - 0.5).abs() < 1e-3);
        assert!((notes[1].duration_sec - 0.25).abs() < 1e-3);
        assert_eq!(result.project.tempos.len(), 2);
        assert!((result.project.bpm - 120.0).abs() < 1e-6);
    }

    #[test]
    fn unclosed_notes_are_dropped() {
        let events = vec![note_on(0, 60, 100), end_of_track()];
        let result = midi_to_project(&write_smf(events), "x.mid", None).unwrap();
        assert_eq!(result.track_count, 0);
        assert_eq!(result.note_count, 0);
    }

    #[test]
    fn clamps_very_short_notes_to_minimum_duration() {
        let events = vec![note_on(0, 60, 100), note_off(1, 60), end_of_track()];
        let result = midi_to_project(&write_smf(events), "x.mid", None).unwrap();
        assert!((result.project.tracks[0].notes[0].duration_sec - 0.02).abs() < 1e-9);
    }

    #[test]
    fn names_unnamed_tracks_by_index() {
        let events = vec![note_on(0, 60, 100), note_off(240, 60), end_of_track()];
        let result = midi_to_project(&write_smf(events), "x.mid", None).unwrap();
        assert_eq!(result.project.tracks[0].name, "Track 1");
    }

    #[test]
    fn empty_filename_falls_back_to_default_name() {
        let events = vec![note_on(0, 60, 100), note_off(240, 60), end_of_track()];
        let result = midi_to_project(&write_smf(events), ".mid", None).unwrap();
        assert_eq!(result.project.name, "音MAD");
    }

    #[test]
    fn carries_polyphony_and_reverb_send_from_previous() {
        let previous = crate::schema::parse_project(serde_json::json!({
            "version": 1, "name": "prev",
            "samples": [{ "id": "kept", "name": "k" }],
            "tracks": [{
                "id": "old", "name": "old", "midiIndex": 0,
                "reverbSend": 0.5,
                "polyphony": { "maxVoices": 4, "priority": "oldest", "stopMode": "pitch" }
            }]
        }))
        .unwrap();
        let events = vec![note_on(0, 60, 100), note_off(240, 60), end_of_track()];
        let result = midi_to_project(&write_smf(events), "x.mid", Some(&previous)).unwrap();
        let track = &result.project.tracks[0];
        assert_eq!(track.midi_index, Some(0));
        assert!((track.reverb_send - 0.5).abs() < 1e-9);
        assert_eq!(track.polyphony.max_voices, 4);
        assert_eq!(
            track.polyphony.priority,
            crate::schema::VoicePriority::Oldest
        );
        assert_eq!(track.polyphony.stop_mode, crate::schema::StopMode::Pitch);
    }

    #[test]
    fn preserves_split_track_dynamics_depth_independently() {
        let previous = crate::schema::parse_project(serde_json::json!({
            "version": 1, "name": "prev",
            "tracks": [
                { "id": "mel", "name": "m", "midiIndex": 0, "drumMode": false, "dynamicsDepth": 0.25 },
                { "id": "drm", "name": "d", "midiIndex": 0, "drumMode": true, "dynamicsDepth": 0.75 }
            ]
        }))
        .unwrap();
        let events = vec![
            note_on_ch(0, 0, 60, 100),
            note_off_ch(240, 0, 60),
            note_on_ch(0, GM_DRUM_CHANNEL, 38, 100),
            note_off_ch(240, GM_DRUM_CHANNEL, 38),
            end_of_track(),
        ];
        let result = midi_to_project(&write_smf(events), "x.mid", Some(&previous)).unwrap();
        let melodic = result.project.tracks.iter().find(|t| !t.drum_mode).unwrap();
        let drum = result.project.tracks.iter().find(|t| t.drum_mode).unwrap();
        assert!((melodic.dynamics_depth - 0.25).abs() < 1e-9);
        assert!((drum.dynamics_depth - 0.75).abs() < 1e-9);
    }

    #[test]
    fn rejects_invalid_midi_bytes() {
        assert!(midi_to_project(&[0, 1, 2, 3], "x.mid", None).is_err());
        assert!(midi_to_project(&[], "x.mid", None).is_err());
    }

    #[test]
    fn assigns_distinct_track_colors_from_palette() {
        let result = midi_to_project(
            &write_smf(vec![note_on(0, 60, 100), note_off(240, 60), end_of_track()]),
            "x.mid",
            None,
        )
        .unwrap();
        assert_eq!(result.project.tracks[0].color, TRACK_PALETTE[0]);
    }

    fn note_on_ch(tick_delta: u32, channel: u8, key: u8, vel: u8) -> TrackEvent<'static> {
        TrackEvent {
            delta: delta(tick_delta),
            kind: TrackEventKind::Midi {
                channel: channel.into(),
                message: MidiMessage::NoteOn {
                    key: u7::new(key),
                    vel: u7::new(vel),
                },
            },
        }
    }

    fn note_off_ch(tick_delta: u32, channel: u8, key: u8) -> TrackEvent<'static> {
        TrackEvent {
            delta: delta(tick_delta),
            kind: TrackEventKind::Midi {
                channel: channel.into(),
                message: MidiMessage::NoteOff {
                    key: u7::new(key),
                    vel: u7::new(0),
                },
            },
        }
    }

    fn controller(tick_delta: u32, channel: u8, num: u8, value: u8) -> TrackEvent<'static> {
        TrackEvent {
            delta: delta(tick_delta),
            kind: TrackEventKind::Midi {
                channel: channel.into(),
                message: MidiMessage::Controller {
                    controller: u7::new(num),
                    value: u7::new(value),
                },
            },
        }
    }

    #[test]
    fn auto_mode_flags_gm_drum_channel() {
        let drum = vec![
            note_on_ch(0, 9, 38, 100),
            note_off_ch(240, 9, 38),
            end_of_track(),
        ];
        let r = midi_to_project(&write_smf(drum), "d.mid", None).unwrap();
        assert!(r.project.tracks[0].drum_mode);

        let melodic = vec![note_on(0, 60, 100), note_off(240, 60), end_of_track()];
        let r = midi_to_project(&write_smf(melodic), "m.mid", None).unwrap();
        assert!(!r.project.tracks[0].drum_mode);
    }

    #[test]
    fn auto_mode_flags_bank_msb_127() {
        let events = vec![
            controller(0, 3, 0, 127),
            note_on_ch(0, 3, 60, 100),
            note_off_ch(240, 3, 60),
            end_of_track(),
        ];
        let r = midi_to_project(&write_smf(events), "x.mid", None).unwrap();
        assert!(r.project.tracks[0].drum_mode);
    }

    #[test]
    fn auto_mode_uses_bank_state_at_note_onset() {
        let events = vec![
            note_on_ch(0, 3, 60, 100),
            note_off_ch(240, 3, 60),
            controller(0, 3, 0, 127),
            note_on_ch(0, 3, 38, 100),
            note_off_ch(240, 3, 38),
            end_of_track(),
        ];
        let r = midi_to_project(&write_smf(events), "x.mid", None).unwrap();
        assert!(!r.project.tracks[0].drum_mode);
    }

    #[test]
    fn auto_mode_splits_mixed_melodic_and_drum_tracks() {
        let mixed = || {
            vec![
                note_on_ch(0, 0, 60, 100),
                note_off_ch(240, 0, 60),
                note_on_ch(0, 9, 38, 100),
                note_off_ch(240, 9, 38),
                end_of_track(),
            ]
        };
        let r = midi_to_project(&write_smf(mixed()), "x.mid", None).unwrap();
        assert_eq!(r.track_count, 2);
        assert_eq!(r.note_count, 2);
        let drum = r.project.tracks.iter().find(|t| t.drum_mode).unwrap();
        let melodic = r.project.tracks.iter().find(|t| !t.drum_mode).unwrap();
        assert_eq!(drum.notes.len(), 1);
        assert_eq!(drum.notes[0].pitch, 38);
        assert_eq!(melodic.notes.len(), 1);
        assert_eq!(melodic.notes[0].pitch, 60);

        let normal =
            midi_to_project_with_mode(&write_smf(mixed()), "x.mid", None, ImportMode::Normal)
                .unwrap();
        assert_eq!(normal.track_count, 1);
        assert!(!normal.project.tracks[0].drum_mode);
    }

    #[test]
    fn import_modes_force_drum_or_normal() {
        let melodic = write_smf(vec![note_on(0, 60, 100), note_off(240, 60), end_of_track()]);
        let normal =
            midi_to_project_with_mode(&melodic, "x.mid", None, ImportMode::Normal).unwrap();
        assert!(!normal.project.tracks[0].drum_mode);
        let drum = midi_to_project_with_mode(&melodic, "x.mid", None, ImportMode::Drum).unwrap();
        assert!(drum.project.tracks[0].drum_mode);

        let gm_drum = write_smf(vec![
            note_on_ch(0, 9, 38, 100),
            note_off_ch(240, 9, 38),
            end_of_track(),
        ]);
        let forced_normal =
            midi_to_project_with_mode(&gm_drum, "x.mid", None, ImportMode::Normal).unwrap();
        assert!(!forced_normal.project.tracks[0].drum_mode);
    }

    #[test]
    fn import_mode_from_str() {
        assert_eq!(ImportMode::from_str_or_auto("normal"), ImportMode::Normal);
        assert_eq!(ImportMode::from_str_or_auto("drum"), ImportMode::Drum);
        assert_eq!(ImportMode::from_str_or_auto("auto"), ImportMode::Auto);
        assert_eq!(ImportMode::from_str_or_auto("???"), ImportMode::Auto);
    }
}
