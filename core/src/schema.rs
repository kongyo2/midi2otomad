//! プロジェクトスキーマ。TypeScript の Zod 定義から移植した、シリアライズ可能な
//! データモデルとデフォルト値・範囲バリデーション。`parse_project` は欠落フィールドを
//! 既定値で補完し、範囲外や未知の列挙子を拒否する（Zod の `.parse()` と同じ挙動）。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const DEFAULT_BASE_PITCH: i32 = 60;
pub const DEFAULT_SAMPLE_RATE: i32 = 48000;
pub const DEFAULT_PROJECT_NAME: &str = "Untitled 音MAD";

// --- デフォルト値ヘルパー -------------------------------------------------

fn one_f64() -> f64 {
    1.0
}
fn t_bool() -> bool {
    true
}
fn attack_ms_default() -> f64 {
    4.0
}
fn release_ms_default() -> f64 {
    90.0
}
fn cutoff_default() -> f64 {
    20000.0
}
fn q_default() -> f64 {
    0.707
}
fn five_f64() -> f64 {
    5.0
}
fn half_f64() -> f64 {
    0.5
}
fn wet_default() -> f64 {
    0.25
}
fn tail_default() -> f64 {
    0.25
}
fn threshold_default() -> f64 {
    0.8
}
fn velocity_default() -> i32 {
    100
}
fn base_pitch_default() -> i32 {
    DEFAULT_BASE_PITCH
}
fn bpm_default() -> f64 {
    140.0
}
fn ppq_default() -> i32 {
    480
}
fn sample_rate_default() -> i32 {
    DEFAULT_SAMPLE_RATE
}
fn color_default() -> String {
    "#7c5cff".to_string()
}

// --- 列挙型 ---------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum FilterType {
    #[default]
    Lowpass,
    Highpass,
    Bandpass,
    Notch,
    Peaking,
    Lowshelf,
    Highshelf,
    Allpass,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum LfoShape {
    #[default]
    Sine,
    Triangle,
    Square,
    Saw,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum InterpolationMode {
    Linear,
    #[default]
    Hermite,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum VoicePriority {
    #[default]
    Newest,
    Oldest,
    Highest,
    Lowest,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum StopMode {
    #[default]
    None,
    Pitch,
    Sample,
    Track,
}


// --- ネストした設定構造体 -------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Envelope {
    #[serde(default)]
    pub delay_ms: f64,
    #[serde(default = "attack_ms_default")]
    pub attack_ms: f64,
    #[serde(default)]
    pub attack_curve: f64,
    #[serde(default)]
    pub hold_ms: f64,
    #[serde(default)]
    pub decay_ms: f64,
    #[serde(default)]
    pub decay_curve: f64,
    #[serde(default = "one_f64")]
    pub sustain: f64,
    #[serde(default = "release_ms_default")]
    pub release_ms: f64,
    #[serde(default)]
    pub release_curve: f64,
}

impl Default for Envelope {
    fn default() -> Self {
        Self {
            delay_ms: 0.0,
            attack_ms: 4.0,
            attack_curve: 0.0,
            hold_ms: 0.0,
            decay_ms: 0.0,
            decay_curve: 0.0,
            sustain: 1.0,
            release_ms: 90.0,
            release_curve: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Filter {
    #[serde(default)]
    pub enabled: bool,
    #[serde(rename = "type", default)]
    pub kind: FilterType,
    #[serde(default = "cutoff_default")]
    pub cutoff_hz: f64,
    #[serde(default = "q_default")]
    pub q: f64,
    #[serde(default)]
    pub gain_db: f64,
    #[serde(default)]
    pub env_amount: f64,
    #[serde(default = "five_f64")]
    pub lfo_hz: f64,
    #[serde(default)]
    pub lfo_depth: f64,
    #[serde(default)]
    pub lfo_shape: LfoShape,
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            enabled: false,
            kind: FilterType::Lowpass,
            cutoff_hz: 20000.0,
            q: 0.707,
            gain_db: 0.0,
            env_amount: 0.0,
            lfo_hz: 5.0,
            lfo_depth: 0.0,
            lfo_shape: LfoShape::Sine,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PitchMod {
    #[serde(default)]
    pub glide_semitones: f64,
    #[serde(default)]
    pub glide_ms: f64,
    #[serde(default)]
    pub glide_curve: f64,
    #[serde(default)]
    pub vibrato_cents: f64,
    #[serde(default = "five_f64")]
    pub vibrato_hz: f64,
    #[serde(default)]
    pub vibrato_delay_ms: f64,
    #[serde(default)]
    pub vibrato_fade_ms: f64,
    #[serde(default)]
    pub vibrato_shape: LfoShape,
}

impl Default for PitchMod {
    fn default() -> Self {
        Self {
            glide_semitones: 0.0,
            glide_ms: 0.0,
            glide_curve: 0.0,
            vibrato_cents: 0.0,
            vibrato_hz: 5.0,
            vibrato_delay_ms: 0.0,
            vibrato_fade_ms: 0.0,
            vibrato_shape: LfoShape::Sine,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Loop {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub start_sec: f64,
    #[serde(default)]
    pub end_sec: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub file_name: String,
    #[serde(default = "base_pitch_default")]
    pub base_pitch: i32,
    #[serde(default)]
    pub tune_cents: f64,
    #[serde(default = "one_f64")]
    pub gain: f64,
    #[serde(default)]
    pub duration_sec: f64,
    #[serde(default)]
    pub interpolation: InterpolationMode,
    #[serde(rename = "loop", default)]
    pub loop_region: Loop,
    #[serde(default)]
    pub envelope: Envelope,
    #[serde(default)]
    pub filter: Filter,
    #[serde(default)]
    pub pitch_mod: PitchMod,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reverb {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "half_f64")]
    pub room_size: f64,
    #[serde(default = "half_f64")]
    pub damping: f64,
    #[serde(default = "one_f64")]
    pub width: f64,
    #[serde(default = "wet_default")]
    pub wet: f64,
    #[serde(default = "one_f64")]
    pub dry: f64,
    #[serde(default)]
    pub pre_delay_ms: f64,
}

impl Default for Reverb {
    fn default() -> Self {
        Self {
            enabled: false,
            room_size: 0.5,
            damping: 0.5,
            width: 1.0,
            wet: 0.25,
            dry: 1.0,
            pre_delay_ms: 0.0,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Polyphony {
    #[serde(default)]
    pub max_voices: i32,
    #[serde(default)]
    pub priority: VoicePriority,
    #[serde(default)]
    pub stop_mode: StopMode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Limiter {
    #[serde(default = "t_bool")]
    pub enabled: bool,
    #[serde(default = "threshold_default")]
    pub threshold: f64,
}

impl Default for Limiter {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold: 0.8,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    #[serde(default = "tail_default")]
    pub tail_sec: f64,
    #[serde(default)]
    pub limiter: Limiter,
}

impl Default for Output {
    fn default() -> Self {
        Self {
            tail_sec: 0.25,
            limiter: Limiter::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub pitch: i32,
    pub start_sec: f64,
    pub duration_sec: f64,
    #[serde(default = "velocity_default")]
    pub velocity: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AutomationPoint {
    pub t: f64,
    pub v: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackDynamics {
    #[serde(default)]
    pub volume: Vec<AutomationPoint>,
    #[serde(default)]
    pub expression: Vec<AutomationPoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub midi_index: Option<i32>,
    #[serde(default = "color_default")]
    pub color: String,
    #[serde(default)]
    pub muted: bool,
    #[serde(default)]
    pub solo: bool,
    #[serde(default = "one_f64")]
    pub gain: f64,
    #[serde(default)]
    pub pan: f64,
    #[serde(default)]
    pub default_sample_id: Option<String>,
    #[serde(default)]
    pub note_sample_map: HashMap<String, String>,
    #[serde(default)]
    pub notes: Vec<Note>,
    #[serde(default)]
    pub dynamics: TrackDynamics,
    #[serde(default)]
    pub reverb_send: f64,
    #[serde(default)]
    pub polyphony: Polyphony,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tempo {
    pub time_sec: f64,
    pub bpm: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub version: u32,
    pub name: String,
    #[serde(default = "bpm_default")]
    pub bpm: f64,
    #[serde(default = "ppq_default")]
    pub ppq: i32,
    #[serde(default = "sample_rate_default")]
    pub sample_rate: i32,
    #[serde(default = "one_f64")]
    pub master_gain: f64,
    #[serde(default)]
    pub tempos: Vec<Tempo>,
    #[serde(default)]
    pub samples: Vec<Sample>,
    #[serde(default)]
    pub tracks: Vec<Track>,
    #[serde(default)]
    pub reverb: Reverb,
    #[serde(default)]
    pub output: Output,
}

// --- バリデーション -------------------------------------------------------

fn range(label: &str, v: f64, lo: f64, hi: f64) -> Result<(), String> {
    if v < lo || v > hi {
        Err(format!("{label} は範囲 [{lo}, {hi}] 外です: {v}"))
    } else {
        Ok(())
    }
}

fn at_least(label: &str, v: f64, lo: f64) -> Result<(), String> {
    if v < lo {
        Err(format!("{label} は {lo} 以上である必要があります: {v}"))
    } else {
        Ok(())
    }
}

impl Envelope {
    fn validate(&self) -> Result<(), String> {
        range("delayMs", self.delay_ms, 0.0, 5000.0)?;
        range("attackMs", self.attack_ms, 0.0, 5000.0)?;
        range("attackCurve", self.attack_curve, -8.0, 8.0)?;
        range("holdMs", self.hold_ms, 0.0, 5000.0)?;
        range("decayMs", self.decay_ms, 0.0, 20000.0)?;
        range("decayCurve", self.decay_curve, -8.0, 8.0)?;
        range("sustain", self.sustain, 0.0, 1.0)?;
        range("releaseMs", self.release_ms, 0.0, 20000.0)?;
        range("releaseCurve", self.release_curve, -8.0, 8.0)
    }
}

impl Filter {
    fn validate(&self) -> Result<(), String> {
        range("cutoffHz", self.cutoff_hz, 20.0, 20000.0)?;
        range("q", self.q, 0.1, 24.0)?;
        range("gainDb", self.gain_db, -24.0, 24.0)?;
        range("envAmount", self.env_amount, -8.0, 8.0)?;
        range("lfoHz", self.lfo_hz, 0.0, 16.0)?;
        range("lfoDepth", self.lfo_depth, 0.0, 8.0)
    }
}

impl PitchMod {
    fn validate(&self) -> Result<(), String> {
        range("glideSemitones", self.glide_semitones, -48.0, 48.0)?;
        range("glideMs", self.glide_ms, 0.0, 5000.0)?;
        range("glideCurve", self.glide_curve, -8.0, 8.0)?;
        range("vibratoCents", self.vibrato_cents, 0.0, 1200.0)?;
        range("vibratoHz", self.vibrato_hz, 0.0, 20.0)?;
        range("vibratoDelayMs", self.vibrato_delay_ms, 0.0, 5000.0)?;
        range("vibratoFadeMs", self.vibrato_fade_ms, 0.0, 5000.0)
    }
}

impl Loop {
    fn validate(&self) -> Result<(), String> {
        at_least("loop.startSec", self.start_sec, 0.0)?;
        at_least("loop.endSec", self.end_sec, 0.0)
    }
}

impl Sample {
    fn validate(&self) -> Result<(), String> {
        range("basePitch", self.base_pitch as f64, 0.0, 127.0)?;
        range("tuneCents", self.tune_cents, -2400.0, 2400.0)?;
        range("gain", self.gain, 0.0, 4.0)?;
        at_least("durationSec", self.duration_sec, 0.0)?;
        self.loop_region.validate()?;
        self.envelope.validate()?;
        self.filter.validate()?;
        self.pitch_mod.validate()
    }
}

impl Reverb {
    fn validate(&self) -> Result<(), String> {
        range("roomSize", self.room_size, 0.0, 1.0)?;
        range("damping", self.damping, 0.0, 1.0)?;
        range("width", self.width, 0.0, 1.0)?;
        range("wet", self.wet, 0.0, 1.0)?;
        range("dry", self.dry, 0.0, 1.0)?;
        range("preDelayMs", self.pre_delay_ms, 0.0, 500.0)
    }
}

impl Polyphony {
    fn validate(&self) -> Result<(), String> {
        range("maxVoices", self.max_voices as f64, 0.0, 64.0)
    }
}

impl Limiter {
    fn validate(&self) -> Result<(), String> {
        range("limiter.threshold", self.threshold, 0.1, 1.0)
    }
}

impl Output {
    fn validate(&self) -> Result<(), String> {
        range("tailSec", self.tail_sec, 0.0, 10.0)?;
        self.limiter.validate()
    }
}

impl Note {
    fn validate(&self) -> Result<(), String> {
        range("pitch", self.pitch as f64, 0.0, 127.0)?;
        at_least("startSec", self.start_sec, 0.0)?;
        if self.duration_sec <= 0.0 {
            return Err(format!(
                "durationSec は正の値である必要があります: {}",
                self.duration_sec
            ));
        }
        range("velocity", self.velocity as f64, 0.0, 127.0)
    }
}

impl AutomationPoint {
    fn validate(&self) -> Result<(), String> {
        at_least("automation.t", self.t, 0.0)?;
        range("automation.v", self.v, 0.0, 1.0)
    }
}

impl Tempo {
    fn validate(&self) -> Result<(), String> {
        at_least("tempo.timeSec", self.time_sec, 0.0)?;
        if self.bpm <= 0.0 {
            return Err(format!(
                "tempo.bpm は正の値である必要があります: {}",
                self.bpm
            ));
        }
        Ok(())
    }
}

impl Track {
    fn validate(&self) -> Result<(), String> {
        range("track.gain", self.gain, 0.0, 4.0)?;
        range("track.pan", self.pan, -1.0, 1.0)?;
        range("reverbSend", self.reverb_send, 0.0, 1.0)?;
        if let Some(idx) = self.midi_index {
            at_least("midiIndex", idx as f64, 0.0)?;
        }
        for note in &self.notes {
            note.validate()?;
        }
        for point in self.dynamics.volume.iter().chain(&self.dynamics.expression) {
            point.validate()?;
        }
        self.polyphony.validate()
    }
}

impl Project {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err(format!("サポート外のバージョンです: {}", self.version));
        }
        if self.name.is_empty() {
            return Err("name は空にできません".to_string());
        }
        if self.bpm <= 0.0 {
            return Err(format!("bpm は正の値である必要があります: {}", self.bpm));
        }
        if self.ppq <= 0 {
            return Err(format!("ppq は正の整数である必要があります: {}", self.ppq));
        }
        if self.sample_rate <= 0 {
            return Err(format!(
                "sampleRate は正の整数である必要があります: {}",
                self.sample_rate
            ));
        }
        range("masterGain", self.master_gain, 0.0, 4.0)?;
        for tempo in &self.tempos {
            tempo.validate()?;
        }
        for sample in &self.samples {
            sample.validate()?;
        }
        for track in &self.tracks {
            track.validate()?;
        }
        self.reverb.validate()?;
        self.output.validate()
    }
}

// --- 公開 API -------------------------------------------------------------

/// 任意の JSON 値をプロジェクトとしてデシリアライズし、欠落値を補完してから
/// 範囲・列挙バリデーションを行う。Zod の `parseProject` 相当。
pub fn parse_project(raw: serde_json::Value) -> Result<Project, String> {
    let project: Project = serde_json::from_value(raw).map_err(|e| e.to_string())?;
    project.validate()?;
    Ok(project)
}

/// 既定値で初期化した空のプロジェクトを作る。
pub fn create_empty_project(name: &str) -> Project {
    Project {
        version: 1,
        name: name.to_string(),
        bpm: 140.0,
        ppq: 480,
        sample_rate: DEFAULT_SAMPLE_RATE,
        master_gain: 1.0,
        tempos: Vec::new(),
        samples: Vec::new(),
        tracks: Vec::new(),
        reverb: Reverb::default(),
        output: Output::default(),
    }
}

/// 既定値で初期化した素材を作る。
pub fn create_sample(id: &str, name: &str) -> Sample {
    Sample {
        id: id.to_string(),
        name: name.to_string(),
        file_name: String::new(),
        base_pitch: DEFAULT_BASE_PITCH,
        tune_cents: 0.0,
        gain: 1.0,
        duration_sec: 0.0,
        interpolation: InterpolationMode::Hermite,
        loop_region: Loop::default(),
        envelope: Envelope::default(),
        filter: Filter::default(),
        pitch_mod: PitchMod::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn exposes_constants() {
        assert_eq!(DEFAULT_BASE_PITCH, 60);
        assert_eq!(DEFAULT_SAMPLE_RATE, 48000);
    }

    #[test]
    fn applies_defaults_for_minimal_project() {
        let project = parse_project(json!({ "version": 1, "name": "Minimal" })).unwrap();
        assert_eq!(project.bpm, 140.0);
        assert_eq!(project.ppq, 480);
        assert_eq!(project.sample_rate, DEFAULT_SAMPLE_RATE);
        assert_eq!(project.master_gain, 1.0);
        assert!(project.tempos.is_empty());
        assert!(project.samples.is_empty());
        assert!(project.tracks.is_empty());
    }

    #[test]
    fn fills_sample_defaults() {
        let project = parse_project(
            json!({ "version": 1, "name": "S", "samples": [{ "id": "s1", "name": "kick" }] }),
        )
        .unwrap();
        let s = &project.samples[0];
        assert_eq!(s.file_name, "");
        assert_eq!(s.base_pitch, DEFAULT_BASE_PITCH);
        assert_eq!(s.tune_cents, 0.0);
        assert_eq!(s.gain, 1.0);
        assert_eq!(s.interpolation, InterpolationMode::Hermite);
        assert_eq!(s.loop_region, Loop::default());
        assert_eq!(s.envelope, Envelope::default());
        assert_eq!(s.envelope.attack_ms, 4.0);
        assert_eq!(s.envelope.release_ms, 90.0);
        assert_eq!(s.filter, Filter::default());
        assert_eq!(s.pitch_mod.vibrato_hz, 5.0);
    }

    #[test]
    fn fills_track_and_note_defaults() {
        let project = parse_project(json!({
            "version": 1, "name": "T",
            "tracks": [{ "id": "t1", "name": "lead", "notes": [{ "pitch": 60, "startSec": 0, "durationSec": 1 }] }]
        }))
        .unwrap();
        let track = &project.tracks[0];
        assert_eq!(track.color, "#7c5cff");
        assert!(!track.muted);
        assert!(!track.solo);
        assert_eq!(track.gain, 1.0);
        assert_eq!(track.pan, 0.0);
        assert_eq!(track.default_sample_id, None);
        assert!(track.note_sample_map.is_empty());
        assert_eq!(track.notes[0].velocity, 100);
    }

    #[test]
    fn partial_envelope_and_filter_fill() {
        let project = parse_project(json!({
            "version": 1, "name": "Synth",
            "samples": [{
                "id": "s1", "name": "voice", "interpolation": "linear",
                "envelope": { "attackMs": 5, "releaseMs": 120 },
                "filter": { "enabled": true, "type": "bandpass", "cutoffHz": 800, "q": 4, "gainDb": 6 }
            }]
        }))
        .unwrap();
        let s = &project.samples[0];
        assert_eq!(s.interpolation, InterpolationMode::Linear);
        assert_eq!(s.envelope.attack_ms, 5.0);
        assert_eq!(s.envelope.release_ms, 120.0);
        assert_eq!(s.envelope.sustain, 1.0);
        assert_eq!(s.filter.kind, FilterType::Bandpass);
        assert_eq!(s.filter.env_amount, 0.0);
        assert_eq!(s.filter.lfo_hz, 5.0);
        assert_eq!(s.filter.lfo_shape, LfoShape::Sine);
    }

    #[test]
    fn reverb_and_output_defaults() {
        let project = parse_project(json!({ "version": 1, "name": "R" })).unwrap();
        assert_eq!(project.reverb, Reverb::default());
        assert_eq!(project.reverb.wet, 0.25);
        assert_eq!(project.output.tail_sec, 0.25);
        assert!(project.output.limiter.enabled);
        assert_eq!(project.output.limiter.threshold, 0.8);
        assert_eq!(project.tracks.len(), 0);
    }

    #[test]
    fn polyphony_defaults_and_overrides() {
        let project = parse_project(
            json!({ "version": 1, "name": "P", "tracks": [{ "id": "t1", "name": "t" }] }),
        )
        .unwrap();
        assert_eq!(project.tracks[0].polyphony, Polyphony::default());

        let project = parse_project(json!({
            "version": 1, "name": "P",
            "tracks": [{ "id": "t1", "name": "t", "polyphony": { "maxVoices": 4, "priority": "oldest", "stopMode": "pitch" } }]
        }))
        .unwrap();
        let p = &project.tracks[0].polyphony;
        assert_eq!(p.max_voices, 4);
        assert_eq!(p.priority, VoicePriority::Oldest);
        assert_eq!(p.stop_mode, StopMode::Pitch);
    }

    #[test]
    fn rejects_invalid_projects() {
        assert!(parse_project(json!({ "name": "x" })).is_err());
        assert!(parse_project(json!({ "version": 2, "name": "x" })).is_err());
        assert!(parse_project(json!({ "version": 1, "name": "" })).is_err());
        assert!(parse_project(json!({
            "version": 1, "name": "x",
            "tracks": [{ "id": "t1", "name": "t", "notes": [{ "pitch": 60, "startSec": 0, "durationSec": 0 }] }]
        }))
        .is_err());
        assert!(parse_project(json!({
            "version": 1, "name": "x",
            "tracks": [{ "id": "t1", "name": "t", "notes": [{ "pitch": 200, "startSec": 0, "durationSec": 1 }] }]
        }))
        .is_err());
        assert!(parse_project(json!({
            "version": 1, "name": "x",
            "samples": [{ "id": "s1", "name": "s", "envelope": { "sustain": 1.5 } }]
        }))
        .is_err());
        assert!(parse_project(json!({
            "version": 1, "name": "x",
            "samples": [{ "id": "s1", "name": "s", "filter": { "type": "comb" } }]
        }))
        .is_err());
        assert!(parse_project(json!({
            "version": 1, "name": "x", "tracks": [{ "id": "t1", "name": "t", "reverbSend": 2 }]
        }))
        .is_err());
        assert!(parse_project(json!({
            "version": 1, "name": "x", "tracks": [{ "id": "t1", "name": "t", "polyphony": { "priority": "loudest" } }]
        }))
        .is_err());
        assert!(parse_project(json!({
            "version": 1, "name": "x", "tracks": [{ "id": "t1", "name": "t", "polyphony": { "stopMode": "channel" } }]
        }))
        .is_err());
        assert!(parse_project(json!({
            "version": 1, "name": "x", "tracks": [{ "id": "t1", "name": "t", "polyphony": { "maxVoices": -1 } }]
        }))
        .is_err());
        assert!(parse_project(
            json!({ "version": 1, "name": "x", "output": { "limiter": { "threshold": 0.05 } } })
        )
        .is_err());
        assert!(parse_project(
            json!({ "version": 1, "name": "x", "output": { "limiter": { "threshold": 1.5 } } })
        )
        .is_err());
        assert!(
            parse_project(json!({ "version": 1, "name": "x", "output": { "tailSec": -1 } }))
                .is_err()
        );
        assert!(
            parse_project(json!({ "version": 1, "name": "x", "output": { "tailSec": 11 } }))
                .is_err()
        );
    }

    #[test]
    fn create_empty_project_named() {
        let project = create_empty_project(DEFAULT_PROJECT_NAME);
        assert_eq!(project.name, "Untitled 音MAD");
        assert_eq!(project.version, 1);
        assert!(project.tracks.is_empty());
        assert!(project.samples.is_empty());
        project.validate().unwrap();
        assert_eq!(create_empty_project("My Song").name, "My Song");
    }
}
