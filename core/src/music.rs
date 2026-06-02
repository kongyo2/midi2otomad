//! 音高変換のヘルパー。MIDI ノート番号・再生速度比・音名の相互変換。

const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

/// 半音差を再生速度比に変換する。1 オクターブ (12 半音) で 2 倍。
pub fn semitones_to_ratio(semitones: f64) -> f64 {
    if semitones == 0.0 {
        return 1.0;
    }
    2.0_f64.powf(semitones / 12.0)
}

/// `base_pitch` で録音された素材を MIDI ノート `note_pitch` で鳴らすときの再生速度比。
/// 基準より 1 オクターブ上のノートは 2 倍速で再生される。
pub fn pitch_ratio(note_pitch: f64, base_pitch: f64, tune_cents: f64) -> f64 {
    semitones_to_ratio(note_pitch - base_pitch + tune_cents / 100.0)
}

/// MIDI ノート番号を音名 (例: 60 → "C4") に変換する。範囲外はクランプし、小数は丸める。
pub fn midi_to_note_name(midi: f64) -> String {
    let clamped = midi.round().clamp(0.0, 127.0) as i32;
    let name = NOTE_NAMES[(clamped % 12) as usize];
    let octave = clamped / 12 - 1;
    format!("{name}{octave}")
}

/// 音名 (例: "C4", "Db4", "g3") を MIDI ノート番号へ。解釈できなければ `None`。
pub fn note_name_to_midi(name: &str) -> Option<i32> {
    let trimmed = name.trim();
    let mut chars = trimmed.chars().peekable();

    let letter = chars.next()?.to_ascii_lowercase();
    let mut semitone = match letter {
        'c' => 0,
        'd' => 2,
        'e' => 4,
        'f' => 5,
        'g' => 7,
        'a' => 9,
        'b' => 11,
        _ => return None,
    };

    match chars.peek() {
        Some('#') => {
            semitone += 1;
            chars.next();
        }
        Some('b') => {
            semitone -= 1;
            chars.next();
        }
        _ => {}
    }

    let octave_str: String = chars.collect();
    if octave_str.is_empty() {
        return None;
    }
    let octave: i32 = octave_str.parse().ok()?;
    Some((octave + 1) * 12 + semitone)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, prec: i32) -> bool {
        (a - b).abs() < 10f64.powi(-prec) / 2.0
    }

    #[test]
    fn semitones_to_ratio_octave() {
        assert!(close(semitones_to_ratio(12.0), 2.0, 9));
        assert_eq!(semitones_to_ratio(0.0), 1.0);
        assert!(close(semitones_to_ratio(-12.0), 0.5, 9));
    }

    #[test]
    fn pitch_ratio_basics() {
        assert!(close(pitch_ratio(72.0, 60.0, 0.0), 2.0, 9));
        assert!(close(pitch_ratio(60.0, 60.0, 0.0), 1.0, 9));
        assert!(close(pitch_ratio(60.0, 60.0, 1200.0), 2.0, 9));
        assert!(close(
            pitch_ratio(63.0, 60.0, 0.0),
            semitones_to_ratio(3.0),
            9
        ));
    }

    #[test]
    fn midi_to_note_name_cases() {
        assert_eq!(midi_to_note_name(60.0), "C4");
        assert_eq!(midi_to_note_name(61.0), "C#4");
        assert_eq!(midi_to_note_name(69.0), "A4");
        assert_eq!(midi_to_note_name(-5.0), "C-1");
        assert_eq!(midi_to_note_name(200.0), "G9");
        assert_eq!(midi_to_note_name(60.4), "C4");
        assert_eq!(midi_to_note_name(60.6), "C#4");
    }

    #[test]
    fn note_name_to_midi_cases() {
        assert_eq!(note_name_to_midi("C4"), Some(60));
        assert_eq!(note_name_to_midi("A4"), Some(69));
        assert_eq!(note_name_to_midi("C#4"), Some(61));
        assert_eq!(note_name_to_midi("Db4"), Some(61));
        assert_eq!(note_name_to_midi("Cb4"), Some(59));
        assert_eq!(note_name_to_midi("g3"), Some(55));
        assert_eq!(note_name_to_midi("  C4 "), Some(60));
        assert_eq!(note_name_to_midi("C-1"), Some(0));
        assert_eq!(note_name_to_midi("H4"), None);
        assert_eq!(note_name_to_midi("C"), None);
        assert_eq!(note_name_to_midi(""), None);
    }

    #[test]
    fn round_trips() {
        for midi in 12..=120 {
            assert_eq!(
                note_name_to_midi(&midi_to_note_name(midi as f64)),
                Some(midi)
            );
        }
    }
}
