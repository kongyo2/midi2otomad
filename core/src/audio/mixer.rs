use super::envelope::envelope_level;
use super::filter::{create_biquad_state, design_biquad, process_biquad_sample, BiquadCoeffs};
use super::granular::{GrainCloud, DEFAULT_OVERLAP};
use super::lfo::lfo_value;
use super::pitchmod::pitch_offset_semitones;
use super::polyphony::{allocate_voices, VoiceRequest};
use super::reverb::{create_reverb, reverb_decay_seconds, ReverbParams};
use crate::music::{pitch_ratio, semitones_to_ratio};
use crate::schema::{
    AutomationPoint, Filter, InterpolationMode, Note, PitchMode, Project, Sample, Track,
};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct PcmAudio {
    pub sample_rate: f64,
    pub channels: Vec<Vec<f32>>,
    pub frames: usize,
}

impl PcmAudio {
    pub fn duration_sec(&self) -> f64 {
        if self.sample_rate > 0.0 {
            self.frames as f64 / self.sample_rate
        } else {
            0.0
        }
    }
}

pub fn build_waveform_peaks(pcm: &PcmAudio, buckets: usize) -> Vec<f32> {
    let mut peaks = vec![0.0f32; buckets];
    let channel = match pcm.channels.first() {
        Some(c) if pcm.frames > 0 => c,
        _ => return peaks,
    };
    let step = pcm.frames as f64 / buckets as f64;
    for (b, slot) in peaks.iter_mut().enumerate() {
        let start = (b as f64 * step).floor() as usize;
        let end = (((b + 1) as f64 * step).floor() as usize).min(pcm.frames);
        let mut max = 0.0f32;
        for &v in &channel[start..end] {
            let a = v.abs();
            if a > max {
                max = a;
            }
        }
        *slot = max;
    }
    peaks
}

pub trait AudioBank {
    fn get(&self, id: &str) -> Option<&PcmAudio>;
}

impl AudioBank for HashMap<String, PcmAudio> {
    fn get(&self, id: &str) -> Option<&PcmAudio> {
        HashMap::get(self, id)
    }
}

pub struct MapBank(pub HashMap<String, PcmAudio>);

impl AudioBank for MapBank {
    fn get(&self, id: &str) -> Option<&PcmAudio> {
        self.0.get(id)
    }
}

#[derive(Debug, Clone)]
pub struct MixResult {
    pub sample_rate: f64,
    pub left: Vec<f32>,
    pub right: Vec<f32>,
    pub frames: usize,
    pub duration_sec: f64,
    pub peak: f64,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MixOptions {
    pub tail_sec: Option<f64>,
    pub limiter: Option<bool>,
}

const MIN_FRAMES: usize = 1;

const CHOKE_RELEASE_MS: f64 = 5.0;

pub fn velocity_to_gain(velocity: f64) -> f64 {
    let v = velocity.clamp(0.0, 127.0) / 127.0;
    v.powf(1.35)
}

fn scale_dynamics(value: f64, depth: f64) -> f64 {
    1.0 + depth * (value - 1.0)
}

fn eval_automation(points: &[AutomationPoint], t: f64, cursor: &mut usize) -> f64 {
    let len = points.len();
    if len == 0 || t < points[0].t {
        return 1.0;
    }
    let mut k = (*cursor).max(1);
    while k < len && points[k].t <= t {
        k += 1;
    }
    *cursor = k;
    if k >= len {
        points[len - 1].v
    } else {
        let prev = &points[k - 1];
        let next = &points[k];
        let span = next.t - prev.t;
        prev.v + (next.v - prev.v) * ((t - prev.t) / span)
    }
}

#[derive(Debug, Clone, Copy)]
struct LoopRegion {
    start: i64,
    end: i64,
    length: i64,
}

fn resolve_trim(sample: &Sample, src: &PcmAudio) -> (i64, i64) {
    let full = src.frames as i64;
    if !sample.trim.enabled || full < 2 {
        return (0, full);
    }
    let start = ((sample.trim.start_sec * src.sample_rate).floor().max(0.0) as i64).min(full - 1);
    let raw_end = if sample.trim.end_sec > sample.trim.start_sec {
        sample.trim.end_sec
    } else {
        full as f64 / src.sample_rate
    };
    let end = ((raw_end * src.sample_rate).floor() as i64).clamp(start + 1, full);
    (start, end)
}

fn resolve_loop(sample: &Sample, src: &PcmAudio, trim: (i64, i64)) -> Option<LoopRegion> {
    if !sample.loop_region.enabled {
        return None;
    }
    let (trim_start, trim_end) = trim;
    let start = ((sample.loop_region.start_sec * src.sample_rate)
        .floor()
        .max(0.0) as i64)
        .max(trim_start);
    let raw_end = if sample.loop_region.end_sec > sample.loop_region.start_sec {
        sample.loop_region.end_sec
    } else {
        trim_end as f64 / src.sample_rate
    };
    let end = ((raw_end * src.sample_rate).floor() as i64).min(trim_end);
    let length = end - start;
    if length < 2 {
        return None;
    }
    Some(LoopRegion { start, end, length })
}

#[inline]
fn finite(v: f32) -> f64 {
    let v = v as f64;
    if v.is_finite() {
        v
    } else {
        0.0
    }
}

fn sample_at(channel: &[f32], frames: usize, index: i64, region: Option<LoopRegion>) -> f64 {
    let idx = match region {
        Some(r) => (r.start + (((index - r.start) % r.length) + r.length) % r.length) as usize,
        None => {
            if index < 0 {
                0
            } else if index as usize >= frames {
                frames - 1
            } else {
                index as usize
            }
        }
    };
    finite(channel[idx.min(channel.len().saturating_sub(1))])
}

fn read_sample(
    channel: &[f32],
    frames: usize,
    pos: f64,
    interp: InterpolationMode,
    region: Option<LoopRegion>,
) -> f64 {
    let i0 = pos.floor() as i64;
    let frac = pos - i0 as f64;
    match interp {
        InterpolationMode::Linear => {
            let a = sample_at(channel, frames, i0, region);
            let b = sample_at(channel, frames, i0 + 1, region);
            a + (b - a) * frac
        }
        InterpolationMode::Sinc => super::interpolation::windowed_sinc(i0, frac, |idx| {
            sample_at(channel, frames, idx, region)
        }),
        InterpolationMode::Hermite => {
            if region.is_none() {
                let m = frames.min(channel.len());
                if i0 >= 1 && (i0 as usize) + 2 < m {
                    let i = i0 as usize;
                    return super::interpolation::cubic_hermite(
                        finite(channel[i - 1]),
                        finite(channel[i]),
                        finite(channel[i + 1]),
                        finite(channel[i + 2]),
                        frac,
                    );
                }
            }
            let y0 = sample_at(channel, frames, i0 - 1, region);
            let y1 = sample_at(channel, frames, i0, region);
            let y2 = sample_at(channel, frames, i0 + 1, region);
            let y3 = sample_at(channel, frames, i0 + 2, region);
            super::interpolation::cubic_hermite(y0, y1, y2, y3, frac)
        }
    }
}

fn soft_clip(x: f64, threshold: f64) -> f64 {
    let abs = x.abs();
    if abs <= threshold {
        return x;
    }
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let over = (abs - threshold) / (1.0 - threshold);
    sign * (threshold + (1.0 - threshold) * over.tanh())
}

fn pan_gains(pan: f64) -> (f64, f64) {
    let p = pan.clamp(-1.0, 1.0);
    (
        if p <= 0.0 { 1.0 } else { 1.0 - p },
        if p >= 0.0 { 1.0 } else { 1.0 + p },
    )
}

fn any_solo(tracks: &[Track]) -> bool {
    tracks.iter().any(|t| t.solo)
}

fn track_renders(track: &Track, solo: bool) -> bool {
    !track.muted && (!solo || track.solo)
}

fn build_track_dynamics(track: &Track, frames: usize, sample_rate: f64) -> Option<Vec<f32>> {
    let volume = &track.dynamics.volume;
    let expression = &track.dynamics.expression;
    if volume.is_empty() && expression.is_empty() {
        return None;
    }
    let mut out = vec![0.0f32; frames];
    let mut volume_cursor = 1usize;
    let mut expression_cursor = 1usize;
    for (i, slot) in out.iter_mut().enumerate() {
        let t = i as f64 / sample_rate;
        *slot = (eval_automation(volume, t, &mut volume_cursor)
            * eval_automation(expression, t, &mut expression_cursor)) as f32;
    }
    Some(out)
}

fn modulated_cutoff(filter: &Filter, env: f64, t_sec: f64, nyquist: f64) -> f64 {
    let octaves = filter.env_amount * env
        + filter.lfo_depth * lfo_value(filter.lfo_shape, t_sec * filter.lfo_hz);
    let cutoff = filter.cutoff_hz * 2f64.powf(octaves);
    cutoff.clamp(20.0, nyquist)
}

struct Playback {
    pitch_ratio: f64,
    speed_ratio: f64,
    granular: bool,
    reverse: bool,
    grain_ms: f64,
}

fn sample_playback(note: &Note, sample: &Sample, drum_mode: bool) -> Playback {
    let effective_pitch = if drum_mode {
        sample.base_pitch
    } else {
        note.pitch
    };
    let pitch_ratio = pitch_ratio(
        effective_pitch as f64,
        sample.base_pitch as f64,
        sample.tune_cents,
    );
    let granular = sample.pitch_mode == PitchMode::Granular;
    let speed_ratio = if granular {
        sample.speed.clamp(0.05, 16.0)
    } else {
        pitch_ratio
    };
    Playback {
        pitch_ratio,
        speed_ratio,
        granular,
        reverse: sample.reverse,
        grain_ms: sample.grain_ms,
    }
}

fn wrap_loop(pos: f64, region: Option<LoopRegion>) -> f64 {
    match region {
        Some(l) if pos >= l.end as f64 => {
            l.start as f64 + ((pos - l.start as f64) % l.length as f64)
        }
        _ => pos,
    }
}

#[allow(clippy::too_many_arguments)]
fn read_lr(
    ch0: &[f32],
    ch1: &[f32],
    mono: bool,
    frames: usize,
    pos: f64,
    interp: InterpolationMode,
    loop_region: Option<LoopRegion>,
) -> (f64, f64) {
    let region = match loop_region {
        Some(l) if pos >= l.start as f64 => Some(l),
        _ => None,
    };
    if mono {
        let s = read_sample(ch0, frames, pos, interp, region);
        (s, s)
    } else {
        (
            read_sample(ch0, frames, pos, interp, region),
            read_sample(ch1, frames, pos, interp, region),
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn render_note(
    note: &Note,
    sample: &Sample,
    src: &PcmAudio,
    track: &Track,
    track_dyn: Option<&[f32]>,
    left: &mut [f32],
    right: &mut [f32],
    mut send: Option<(&mut [f32], &mut [f32])>,
    out_rate: f64,
    master_gain: f64,
    pan: (f64, f64),
    cut_sec: Option<f64>,
) {
    let total = left.len() as i64;
    let play = sample_playback(note, sample, track.drum_mode);
    let src_rate_ratio = src.sample_rate / out_rate;
    let base_pitch_increment = src_rate_ratio * play.pitch_ratio;
    let time_increment = src_rate_ratio * play.speed_ratio;
    let dir = if play.reverse { -1.0 } else { 1.0 };

    let (trim_start, trim_end) = resolve_trim(sample, src);
    let loop_region = if play.reverse {
        None
    } else {
        resolve_loop(sample, src, (trim_start, trim_end))
    };

    let start_frame = (note.start_sec * out_rate).round() as i64;
    let release_sec = sample.envelope.release_ms / 1000.0;
    let one_shot = sample.one_shot && loop_region.is_none();
    let gate_sec = if one_shot {
        ((trim_end - trim_start) as f64 / src.sample_rate / play.speed_ratio).max(0.0)
    } else {
        note.duration_sec
    };
    let gate_frames = ((gate_sec * out_rate).round() as i64).max(1);
    let release_frames = ((release_sec * out_rate).round() as i64).max(0);
    let voice_frames = gate_frames + release_frames;

    let choke_sec = CHOKE_RELEASE_MS / 1000.0;
    let cut_end_sec = match cut_sec {
        None => f64::INFINITY,
        Some(cs) => cs + choke_sec,
    };

    let depth = track.dynamics_depth.clamp(0.0, 1.0);
    let vel_gain = velocity_to_gain(note.velocity as f64);
    let note_gain = sample.gain * track.gain * master_gain;
    let interp = sample.interpolation;
    let frames = src.frames;

    let ch0 = match src.channels.first() {
        Some(c) => c.as_slice(),
        None => return,
    };
    let ch1 = src.channels.get(1).map(|c| c.as_slice()).unwrap_or(ch0);
    let mono = src.channels.len() < 2;

    let filter = &sample.filter;
    let filter_modulated = filter.enabled && (filter.env_amount != 0.0 || filter.lfo_depth != 0.0);
    let nyquist = out_rate * 0.49;
    let static_coeffs: Option<BiquadCoeffs> = if filter.enabled && !filter_modulated {
        Some(design_biquad(
            filter.kind,
            filter.cutoff_hz.min(nyquist),
            out_rate,
            filter.q,
            filter.gain_db,
        ))
    } else {
        None
    };
    let mut state_l = create_biquad_state();
    let mut state_r = create_biquad_state();
    let reverb_send = track.reverb_send;

    let pitch_modulated =
        sample.pitch_mod.glide_semitones != 0.0 || sample.pitch_mod.vibrato_cents != 0.0;

    let start_pos = if dir > 0.0 {
        trim_start as f64
    } else {
        (trim_end - 1).max(trim_start) as f64
    };
    let mut src_pos = start_pos;
    let mut time_pos = start_pos;
    let mut cloud = if play.granular {
        Some(GrainCloud::new(
            play.grain_ms / 1000.0 * out_rate,
            DEFAULT_OVERLAP,
        ))
    } else {
        None
    };

    for i in 0..voice_frames {
        let out_idx = start_frame + i;
        let t_sec = i as f64 / out_rate;
        if t_sec >= cut_end_sec {
            break;
        }
        let pitch_increment = if pitch_modulated {
            base_pitch_increment
                * semitones_to_ratio(pitch_offset_semitones(&sample.pitch_mod, t_sec))
        } else {
            base_pitch_increment
        };

        let (raw_l, raw_r, alive) = if let Some(cloud) = cloud.as_mut() {
            time_pos = wrap_loop(time_pos, loop_region);
            let spawn = time_pos;
            let read_adv = dir * pitch_increment;
            let (l, r) = cloud.process(spawn, read_adv, |p| {
                read_lr(ch0, ch1, mono, frames, p, interp, loop_region)
            });
            time_pos += dir * time_increment;
            let alive = loop_region.is_some()
                || cloud.active()
                || if dir > 0.0 {
                    time_pos < trim_end as f64
                } else {
                    time_pos > trim_start as f64
                };
            (l, r, alive)
        } else {
            let mut pos = src_pos;
            let mut alive = true;
            if let Some(l) = loop_region {
                if pos >= l.end as f64 {
                    pos = l.start as f64 + ((pos - l.start as f64) % l.length as f64);
                }
            } else if dir > 0.0 {
                if pos >= trim_end as f64 - 1.0 {
                    alive = false;
                }
            } else if pos <= trim_start as f64 {
                alive = false;
            }
            let (l, r) = if alive {
                read_lr(ch0, ch1, mono, frames, pos, interp, loop_region)
            } else {
                (0.0, 0.0)
            };
            src_pos += dir * pitch_increment;
            (l, r, alive)
        };

        if out_idx < 0 {
            continue;
        }
        if out_idx >= total {
            break;
        }
        let out_idx = out_idx as usize;
        if !alive {
            continue;
        }

        let env = envelope_level(&sample.envelope, t_sec, gate_sec);
        let (s_l, s_r) = if filter.enabled {
            let coeffs = if filter_modulated {
                Some(design_biquad(
                    filter.kind,
                    modulated_cutoff(filter, env, t_sec, nyquist),
                    out_rate,
                    filter.q,
                    filter.gain_db,
                ))
            } else {
                static_coeffs
            };
            match coeffs {
                Some(c) if mono => {
                    let s = process_biquad_sample(&c, &mut state_l, raw_l);
                    (s, s)
                }
                Some(c) => (
                    process_biquad_sample(&c, &mut state_l, raw_l),
                    process_biquad_sample(&c, &mut state_r, raw_r),
                ),
                None => (raw_l, raw_r),
            }
        } else {
            (raw_l, raw_r)
        };

        if env > 0.0 {
            let cut_gain = match cut_sec {
                Some(cs) if t_sec > cs => 1.0 - (t_sec - cs) / choke_sec,
                _ => 1.0,
            };
            let cc_gain = match track_dyn {
                None => 1.0,
                Some(td) => td[out_idx] as f64,
            };
            let dyn_gain = scale_dynamics(vel_gain * cc_gain, depth);
            let amp = env * note_gain * dyn_gain * cut_gain;
            let out_l = s_l * amp * pan.0;
            let out_r = s_r * amp * pan.1;
            left[out_idx] = (left[out_idx] as f64 + out_l) as f32;
            right[out_idx] = (right[out_idx] as f64 + out_r) as f32;
            if reverb_send > 0.0 {
                if let Some((sl, sr)) = send.as_mut() {
                    sl[out_idx] = (sl[out_idx] as f64 + out_l * reverb_send) as f32;
                    sr[out_idx] = (sr[out_idx] as f64 + out_r * reverb_send) as f32;
                }
            }
        }
    }
}

fn sounding_duration_sec(note: &Note, sample: &Sample, src: &PcmAudio, drum_mode: bool) -> f64 {
    let release = sample.envelope.release_ms / 1000.0;
    let gate_plus_release = note.duration_sec + release;
    let play = sample_playback(note, sample, drum_mode);
    let (trim_start, trim_end) = resolve_trim(sample, src);
    let looped = !play.reverse && resolve_loop(sample, src, (trim_start, trim_end)).is_some();
    let one_shot_sec = (trim_end - trim_start) as f64 / src.sample_rate / play.speed_ratio;
    if sample.one_shot && !looped {
        return one_shot_sec + release;
    }
    let pitch_modulated =
        sample.pitch_mod.glide_semitones != 0.0 || sample.pitch_mod.vibrato_cents != 0.0;
    if looped || pitch_modulated {
        return gate_plus_release;
    }
    gate_plus_release.min(one_shot_sec)
}

struct PlannedVoice<'a> {
    note: &'a Note,
    sources: Vec<(&'a Sample, &'a PcmAudio)>,
    cut_sec: Option<f64>,
    end_sec: f64,
}

const MAX_LAYERS: usize = 8;

fn collect_sources<'a>(
    root: (&'a Sample, &'a PcmAudio),
    sample_by_id: &HashMap<&str, &'a Sample>,
    bank: &'a dyn AudioBank,
) -> Vec<(&'a Sample, &'a PcmAudio)> {
    let mut out = vec![root];
    if root.0.link_ids.is_empty() {
        return out;
    }
    let mut visited: std::collections::HashSet<&str> = std::collections::HashSet::new();
    visited.insert(root.0.id.as_str());
    let mut queue: std::collections::VecDeque<&str> =
        root.0.link_ids.iter().map(|s| s.as_str()).collect();
    while let Some(id) = queue.pop_front() {
        if out.len() >= MAX_LAYERS {
            break;
        }
        if !visited.insert(id) {
            continue;
        }
        if let (Some(&sample), Some(src)) = (sample_by_id.get(id), bank.get(id)) {
            if src.frames >= 2 {
                out.push((sample, src));
                for l in &sample.link_ids {
                    queue.push_back(l.as_str());
                }
            }
        }
    }
    out
}

struct TrackPlan<'a> {
    track: &'a Track,
    voices: Vec<PlannedVoice<'a>>,
}

fn resolve_note_source<'a>(
    track: &Track,
    note: &Note,
    sample_by_id: &HashMap<&str, &'a Sample>,
    bank: &'a dyn AudioBank,
) -> Option<(&'a Sample, &'a PcmAudio)> {
    let sample_id = track
        .note_sample_map
        .get(&note.pitch.to_string())
        .or(track.default_sample_id.as_ref())?;
    let sample = *sample_by_id.get(sample_id.as_str())?;
    let src = bank.get(sample_id)?;
    if src.frames < 2 {
        return None;
    }
    Some((sample, src))
}

fn plan_track_voices<'a>(
    track: &'a Track,
    sample_by_id: &HashMap<&str, &'a Sample>,
    bank: &'a dyn AudioBank,
) -> Vec<PlannedVoice<'a>> {
    let resolved: Vec<(&Note, Vec<(&'a Sample, &'a PcmAudio)>)> = track
        .notes
        .iter()
        .filter_map(|note| {
            resolve_note_source(track, note, sample_by_id, bank)
                .map(|root| (note, collect_sources(root, sample_by_id, bank)))
        })
        .collect();
    let requests: Vec<VoiceRequest> = resolved
        .iter()
        .map(|(note, sources)| VoiceRequest {
            pitch: note.pitch,
            start_sec: note.start_sec,
            duration_sec: sources
                .iter()
                .map(|(sample, src)| sounding_duration_sec(note, sample, src, track.drum_mode))
                .fold(0.0, f64::max),
            sample_id: sources[0].0.id.clone(),
        })
        .collect();
    allocate_voices(&requests, &track.polyphony)
        .into_iter()
        .map(|alloc| {
            let note = resolved[alloc.index].0;
            let sources = resolved[alloc.index].1.clone();
            let cut_sec = if alloc.duration_sec < requests[alloc.index].duration_sec {
                Some(alloc.duration_sec)
            } else {
                None
            };
            let max_release = sources
                .iter()
                .map(|(s, _)| s.envelope.release_ms)
                .fold(0.0, f64::max)
                / 1000.0;
            let natural = requests[alloc.index]
                .duration_sec
                .max(note.duration_sec + max_release);
            let end_sec = match cut_sec {
                None => note.start_sec + natural,
                Some(cs) => note.start_sec + cs + CHOKE_RELEASE_MS / 1000.0,
            };
            PlannedVoice {
                note,
                sources,
                cut_sec,
                end_sec,
            }
        })
        .collect()
}

fn reverb_audible(project: &Project, plans: &[TrackPlan]) -> bool {
    if !project.reverb.enabled || project.reverb.wet <= 0.0 {
        return false;
    }
    plans
        .iter()
        .any(|tp| tp.track.reverb_send > 0.0 && !tp.voices.is_empty())
}

fn reverb_tail_seconds(project: &Project) -> f64 {
    project.reverb.pre_delay_ms / 1000.0 + reverb_decay_seconds(project.reverb.room_size)
}

fn apply_reverb(
    project: &Project,
    out_rate: f64,
    send_l: &[f32],
    send_r: &[f32],
    left: &mut [f32],
    right: &mut [f32],
) {
    let r = &project.reverb;
    let mut verb = create_reverb(
        out_rate,
        ReverbParams {
            room_size: r.room_size,
            damping: r.damping,
            width: r.width,
            wet: r.wet,
            dry: 0.0,
            pre_delay_ms: r.pre_delay_ms,
        },
    );
    let wet = verb.process_block(send_l, send_r);
    for (o, w) in left.iter_mut().zip(wet.left.iter()) {
        *o = (*o as f64 + *w as f64) as f32;
    }
    for (o, w) in right.iter_mut().zip(wet.right.iter()) {
        *o = (*o as f64 + *w as f64) as f32;
    }
}

pub fn mix_project<'a>(
    project: &'a Project,
    bank: &'a dyn AudioBank,
    options: &MixOptions,
) -> MixResult {
    let out_rate = project.sample_rate as f64;
    let sample_by_id: HashMap<&str, &Sample> =
        project.samples.iter().map(|s| (s.id.as_str(), s)).collect();
    let tail_sec = options.tail_sec.unwrap_or(project.output.tail_sec);
    let solo = any_solo(&project.tracks);
    let plans: Vec<TrackPlan> = project
        .tracks
        .iter()
        .filter(|t| track_renders(t, solo))
        .map(|t| TrackPlan {
            track: t,
            voices: plan_track_voices(t, &sample_by_id, bank),
        })
        .collect();

    let mut last_end = 0.0;
    for tp in &plans {
        for v in &tp.voices {
            if v.end_sec > last_end {
                last_end = v.end_sec;
            }
        }
    }

    let audible = reverb_audible(project, &plans);
    let end = last_end
        + tail_sec
        + if audible {
            reverb_tail_seconds(project)
        } else {
            0.0
        };
    let frames = ((end * out_rate).ceil() as i64 + 1).max(MIN_FRAMES as i64) as usize;

    let mut left = vec![0.0f32; frames];
    let mut right = vec![0.0f32; frames];
    let mut send: Option<(Vec<f32>, Vec<f32>)> = if audible {
        Some((vec![0.0f32; frames], vec![0.0f32; frames]))
    } else {
        None
    };

    let master_gain = project.master_gain;

    for tp in &plans {
        let pan = pan_gains(tp.track.pan);
        let track_dyn = build_track_dynamics(tp.track, frames, out_rate);
        for voice in &tp.voices {
            for &(sample, src) in &voice.sources {
                let send_ref = send
                    .as_mut()
                    .map(|(l, r)| (l.as_mut_slice(), r.as_mut_slice()));
                render_note(
                    voice.note,
                    sample,
                    src,
                    tp.track,
                    track_dyn.as_deref(),
                    &mut left,
                    &mut right,
                    send_ref,
                    out_rate,
                    master_gain,
                    pan,
                    voice.cut_sec,
                );
            }
        }
    }

    if let Some((sl, sr)) = &send {
        apply_reverb(project, out_rate, sl, sr, &mut left, &mut right);
    }

    let mut peak = 0.0;
    for (l, r) in left.iter().zip(right.iter()) {
        let m = (*l as f64).abs().max((*r as f64).abs());
        if m > peak {
            peak = m;
        }
    }

    if options.limiter.unwrap_or(project.output.limiter.enabled) {
        let threshold = project.output.limiter.threshold;
        for s in left.iter_mut() {
            *s = soft_clip(*s as f64, threshold) as f32;
        }
        for s in right.iter_mut() {
            *s = soft_clip(*s as f64, threshold) as f32;
        }
    }

    MixResult {
        sample_rate: out_rate,
        left,
        right,
        frames,
        duration_sec: frames as f64 / out_rate,
        peak,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::parse_project;
    use serde_json::{json, Value};

    fn mono_source(frames: usize, sample_rate: f64, gen: impl Fn(usize) -> f32) -> PcmAudio {
        PcmAudio {
            sample_rate,
            channels: vec![(0..frames).map(gen).collect()],
            frames,
        }
    }

    fn const_source(value: f32, frames: usize) -> PcmAudio {
        mono_source(frames, 1000.0, |_| value)
    }

    fn ramp_source(frames: usize) -> PcmAudio {
        mono_source(frames, 1000.0, |i| (i % 100) as f32 / 100.0)
    }

    fn mono_ramp_source(frames: usize) -> PcmAudio {
        mono_source(frames, 1000.0, |i| i as f32 / frames as f32)
    }

    fn nyquist_source(frames: usize) -> PcmAudio {
        mono_source(frames, 1000.0, |i| if i % 2 == 0 { 1.0 } else { -1.0 })
    }

    fn bright_source(frames: usize, rate: f64) -> PcmAudio {
        mono_source(frames, rate, |i| if (i / 2) % 2 == 0 { 1.0 } else { -1.0 })
    }

    fn merge(mut base: Value, over: Value) -> Value {
        if let (Value::Object(b), Value::Object(o)) = (&mut base, over) {
            for (k, v) in o {
                b.insert(k, v);
            }
        }
        base
    }

    fn sample_raw(over: Value) -> Value {
        merge(
            json!({
                "id": "s1", "name": "s", "basePitch": 60, "gain": 1, "durationSec": 1,
                "loop": { "enabled": false, "startSec": 0, "endSec": 0 },
                "envelope": { "attackMs": 0, "releaseMs": 0 }
            }),
            over,
        )
    }

    fn track_raw(over: Value) -> Value {
        merge(
            json!({
                "id": "t1", "name": "t", "defaultSampleId": "s1",
                "notes": [{ "pitch": 60, "startSec": 0, "durationSec": 0.5, "velocity": 127 }],
                "dynamics": { "volume": [], "expression": [] }
            }),
            over,
        )
    }

    fn make_project(samples: Value, tracks: Value) -> Project {
        parse_project(json!({
            "version": 1, "name": "test", "sampleRate": 1000, "masterGain": 1,
            "samples": samples, "tracks": tracks
        }))
        .unwrap()
    }

    fn bank1(id: &str, pcm: PcmAudio) -> HashMap<String, PcmAudio> {
        let mut b = HashMap::new();
        b.insert(id.to_string(), pcm);
        b
    }

    fn mix(project: &Project, bank: &HashMap<String, PcmAudio>, opts: MixOptions) -> MixResult {
        mix_project(project, bank, &opts)
    }

    fn limiter_off() -> MixOptions {
        MixOptions {
            tail_sec: None,
            limiter: Some(false),
        }
    }

    fn all_finite(arr: &[f32]) -> bool {
        arr.iter().all(|v| v.is_finite())
    }

    fn max_abs(arr: &[f32]) -> f32 {
        arr.iter().fold(0.0, |p, &v| p.max(v.abs()))
    }

    fn close(a: f64, b: f64, prec: i32) -> bool {
        (a - b).abs() < 10f64.powi(-prec) / 2.0
    }

    fn tail_energy(arr: &[f32], start: usize) -> f64 {
        if start >= arr.len() {
            return 0.0;
        }
        arr[start..].iter().map(|&v| (v as f64) * (v as f64)).sum()
    }

    #[test]
    fn velocity_to_gain_curve() {
        assert!(close(velocity_to_gain(127.0), 1.0, 9));
        assert_eq!(velocity_to_gain(0.0), 0.0);
        assert_eq!(velocity_to_gain(-50.0), 0.0);
        assert!(close(velocity_to_gain(999.0), 1.0, 9));
        assert!(close(
            velocity_to_gain(64.0),
            (64.0f64 / 127.0).powf(1.35),
            9
        ));
    }

    #[test]
    fn bank_lookup() {
        let bank = bank1("s1", const_source(1.0, 4));
        assert!(AudioBank::get(&bank, "s1").is_some());
        assert!(AudioBank::get(&bank, "missing").is_none());
    }

    #[test]
    fn basics() {
        let project = make_project(
            json!([sample_raw(json!({}))]),
            json!([track_raw(json!({}))]),
        );
        let m = mix(
            &project,
            &bank1("s1", const_source(1.0, 1000)),
            MixOptions::default(),
        );
        assert_eq!(m.sample_rate, 1000.0);
        assert!(m.peak > 0.0);
        assert!(close(m.duration_sec, m.frames as f64 / 1000.0, 9));
        assert!(all_finite(&m.left) && all_finite(&m.right));

        let empty = make_project(json!([]), json!([]));
        let m = mix(&empty, &HashMap::new(), MixOptions::default());
        assert_eq!(m.frames, 251);
        assert_eq!(m.peak, 0.0);

        let with_tail = mix(
            &project,
            &bank1("s1", const_source(1.0, 1000)),
            MixOptions {
                tail_sec: Some(1.0),
                limiter: None,
            },
        );
        let no_tail = mix(
            &project,
            &bank1("s1", const_source(1.0, 1000)),
            MixOptions {
                tail_sec: Some(0.0),
                limiter: None,
            },
        );
        assert!(with_tail.frames > no_tail.frames);
    }

    #[test]
    fn voice_selection() {
        let muted = make_project(
            json!([sample_raw(json!({}))]),
            json!([track_raw(json!({ "muted": true }))]),
        );
        assert_eq!(
            mix(
                &muted,
                &bank1("s1", const_source(1.0, 1000)),
                MixOptions::default()
            )
            .peak,
            0.0
        );

        let solo = make_project(
            json!([sample_raw(json!({}))]),
            json!([
                track_raw(json!({ "id": "solo", "solo": true, "pan": -1 })),
                track_raw(json!({ "id": "muted", "solo": false, "pan": 1 }))
            ]),
        );
        let m = mix(&solo, &bank1("s1", const_source(1.0, 1000)), limiter_off());
        assert!(m.left[50].abs() > 0.0);
        assert_eq!(m.right[50], 0.0);

        for tr in [
            track_raw(json!({ "defaultSampleId": null })),
            track_raw(json!({ "defaultSampleId": "ghost" })),
        ] {
            let p = make_project(json!([sample_raw(json!({}))]), json!([tr]));
            assert_eq!(
                mix(
                    &p,
                    &bank1("s1", const_source(1.0, 1000)),
                    MixOptions::default()
                )
                .peak,
                0.0
            );
        }

        let p = make_project(
            json!([sample_raw(json!({}))]),
            json!([track_raw(json!({}))]),
        );
        assert_eq!(mix(&p, &HashMap::new(), MixOptions::default()).peak, 0.0);
        assert_eq!(
            mix(
                &p,
                &bank1("s1", const_source(1.0, 1)),
                MixOptions::default()
            )
            .peak,
            0.0
        );

        let mapped = make_project(
            json!([
                sample_raw(json!({ "id": "a" })),
                sample_raw(json!({ "id": "b" }))
            ]),
            json!([track_raw(json!({
                "defaultSampleId": "a", "noteSampleMap": { "60": "b" },
                "notes": [{ "pitch": 60, "startSec": 0, "durationSec": 0.5, "velocity": 127 }]
            }))]),
        );
        assert!(
            mix(
                &mapped,
                &bank1("b", const_source(1.0, 1000)),
                MixOptions::default()
            )
            .peak
                > 0.0
        );
    }

    fn dynamics_gain(frame: usize) -> f32 {
        let project = make_project(
            json!([sample_raw(json!({}))]),
            json!([track_raw(json!({
                "notes": [{ "pitch": 60, "startSec": 0, "durationSec": 0.5, "velocity": 127 }],
                "dynamics": { "volume": [{ "t": 0.1, "v": 0.4 }, { "t": 0.3, "v": 0.9 }], "expression": [] }
            }))]),
        );
        mix(
            &project,
            &bank1("s1", const_source(1.0, 1000)),
            limiter_off(),
        )
        .left[frame]
    }

    #[test]
    fn track_dynamics() {
        assert!(close(dynamics_gain(50) as f64, 1.0, 4));
        assert!(close(dynamics_gain(200) as f64, 0.65, 4));
        assert!(close(dynamics_gain(400) as f64, 0.9, 4));

        let project = make_project(
            json!([sample_raw(json!({}))]),
            json!([track_raw(json!({
                "notes": [{ "pitch": 60, "startSec": 0, "durationSec": 0.5, "velocity": 127 }],
                "dynamics": { "volume": [{ "t": 0, "v": 0.5 }], "expression": [{ "t": 0, "v": 0.5 }] }
            }))]),
        );
        let m = mix(
            &project,
            &bank1("s1", const_source(1.0, 1000)),
            limiter_off(),
        );
        assert!(close(m.left[100] as f64, 0.25, 4));
    }

    #[test]
    fn dynamics_depth_flattens_velocity_and_cc() {
        let make = |depth: f64, volume: Value| {
            make_project(
                json!([sample_raw(json!({}))]),
                json!([track_raw(json!({
                    "notes": [{ "pitch": 60, "startSec": 0, "durationSec": 0.5, "velocity": 64 }],
                    "dynamics": { "volume": volume, "expression": [] },
                    "dynamicsDepth": depth
                }))]),
            )
        };
        let render = |depth: f64, volume: Value| {
            mix(
                &make(depth, volume),
                &bank1("s1", const_source(1.0, 1000)),
                limiter_off(),
            )
            .left[100] as f64
        };

        assert!(close(render(1.0, json!([])), velocity_to_gain(64.0), 4));
        assert!(close(render(0.0, json!([])), 1.0, 4));

        let cc = json!([{ "t": 0.0, "v": 0.5 }]);
        assert!(close(
            render(1.0, cc.clone()),
            velocity_to_gain(64.0) * 0.5,
            4
        ));
        assert!(close(render(0.0, cc.clone()), 1.0, 4));
        let half = render(0.5, cc);
        assert!(half > render(1.0, json!([{ "t": 0.0, "v": 0.5 }])) && half < 1.0);
    }

    #[test]
    fn panning() {
        let panned = |pan: f64| {
            let p = make_project(
                json!([sample_raw(json!({}))]),
                json!([track_raw(json!({ "pan": pan }))]),
            );
            let m = mix(&p, &bank1("s1", const_source(1.0, 1000)), limiter_off());
            (m.left[50] as f64, m.right[50] as f64)
        };
        let (l, r) = panned(0.0);
        assert!(close(l, 1.0, 5) && close(r, 1.0, 5));
        let (l, r) = panned(0.5);
        assert!(close(l, 0.5, 5) && close(r, 1.0, 5));
        let (l, r) = panned(-0.5);
        assert!(close(l, 1.0, 5) && close(r, 0.5, 5));
        let (l, r) = panned(1.0);
        assert!(close(l, 0.0, 5) && close(r, 1.0, 5));
        let (l, r) = panned(-1.0);
        assert!(close(l, 1.0, 5) && close(r, 0.0, 5));
    }

    #[test]
    fn envelope_and_limiter() {
        let project = make_project(
            json!([sample_raw(
                json!({ "envelope": { "attackMs": 50, "releaseMs": 5 } })
            )]),
            json!([track_raw(json!({}))]),
        );
        let m = mix(
            &project,
            &bank1("s1", const_source(1.0, 1000)),
            limiter_off(),
        );
        assert_eq!(m.left[0], 0.0);
        assert!(m.left[25].abs() < m.left[49].abs());

        let square: Vec<f32> = (0..1000)
            .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
            .collect();
        let project = make_project(
            json!([sample_raw(
                json!({ "envelope": { "attackMs": 20, "releaseMs": 5 } })
            )]),
            json!([track_raw(json!({}))]),
        );
        let pcm = PcmAudio {
            sample_rate: 1000.0,
            channels: vec![square],
            frames: 1000,
        };
        let m = mix(&project, &bank1("s1", pcm), MixOptions::default());
        assert!(m.peak > 0.8);
        assert!(max_abs(&m.left) < 1.0);

        let project = make_project(
            json!([sample_raw(json!({}))]),
            json!([track_raw(json!({}))]),
        );
        let m = mix(
            &project,
            &bank1("s1", const_source(1.0, 1000)),
            limiter_off(),
        );
        assert!(close(m.left[100] as f64, 1.0, 5));
    }

    #[test]
    fn output_settings() {
        let hot = sample_raw(json!({ "envelope": { "attackMs": 0, "releaseMs": 0 } }));
        let make = |output: Value| {
            parse_project(json!({
                "version": 1, "name": "test", "sampleRate": 1000, "masterGain": 1,
                "output": output, "samples": [hot.clone()], "tracks": [track_raw(json!({}))]
            }))
            .unwrap()
        };
        let low = mix(
            &make(json!({ "limiter": { "threshold": 0.5 } })),
            &bank1("s1", const_source(1.0, 1000)),
            MixOptions::default(),
        );
        let high = mix(
            &make(json!({ "limiter": { "threshold": 0.95 } })),
            &bank1("s1", const_source(1.0, 1000)),
            MixOptions::default(),
        );
        assert!(max_abs(&low.left) < max_abs(&high.left));

        let m = mix(
            &make(json!({ "limiter": { "enabled": false } })),
            &bank1("s1", const_source(1.0, 1000)),
            MixOptions::default(),
        );
        assert!(close(m.left[100] as f64, 1.0, 5));

        let forced_off = mix(
            &make(json!({ "limiter": { "enabled": true } })),
            &bank1("s1", const_source(1.0, 1000)),
            limiter_off(),
        );
        assert!(close(forced_off.left[100] as f64, 1.0, 5));
        let forced_on = mix(
            &make(json!({ "limiter": { "enabled": false } })),
            &bank1("s1", const_source(1.0, 1000)),
            MixOptions {
                tail_sec: None,
                limiter: Some(true),
            },
        );
        assert!((forced_on.left[100] as f64) < 1.0);

        let long = mix(
            &make(json!({ "tailSec": 2 })),
            &bank1("s1", const_source(1.0, 1000)),
            MixOptions::default(),
        );
        let short = mix(
            &make(json!({ "tailSec": 0 })),
            &bank1("s1", const_source(1.0, 1000)),
            MixOptions::default(),
        );
        assert!(long.frames > short.frames);
        let project_tail = mix(
            &make(json!({ "tailSec": 2 })),
            &bank1("s1", const_source(1.0, 1000)),
            MixOptions::default(),
        );
        let overridden = mix(
            &make(json!({ "tailSec": 2 })),
            &bank1("s1", const_source(1.0, 1000)),
            MixOptions {
                tail_sec: Some(0.0),
                limiter: None,
            },
        );
        assert!(overridden.frames < project_tail.frames);
    }

    #[test]
    fn source_edge_cases() {
        let left = vec![0.5f32; 1000];
        let right = vec![-0.25f32; 1000];
        let project = make_project(
            json!([sample_raw(json!({}))]),
            json!([track_raw(json!({ "pan": 0 }))]),
        );
        let pcm = PcmAudio {
            sample_rate: 1000.0,
            channels: vec![left, right],
            frames: 1000,
        };
        let m = mix(&project, &bank1("s1", pcm), limiter_off());
        assert!(close(m.left[100] as f64, 0.5, 5));
        assert!(close(m.right[100] as f64, -0.25, 5));

        let project = make_project(
            json!([sample_raw(json!({}))]),
            json!([track_raw(json!({}))]),
        );
        let no_channels = PcmAudio {
            sample_rate: 1000.0,
            channels: vec![],
            frames: 1000,
        };
        assert_eq!(
            mix(&project, &bank1("s1", no_channels), MixOptions::default()).peak,
            0.0
        );

        let mut ch = vec![0.3f32; 1000];
        ch[1] = f32::NAN;
        ch[2] = f32::INFINITY;
        ch[3] = f32::NEG_INFINITY;
        let pcm = PcmAudio {
            sample_rate: 1000.0,
            channels: vec![ch],
            frames: 1000,
        };
        let m = mix(&project, &bank1("s1", pcm), limiter_off());
        assert!(all_finite(&m.left) && all_finite(&m.right));

        let m = mix(
            &project,
            &bank1("s1", const_source(1.0, 100)),
            limiter_off(),
        );
        assert!(m.left[10].abs() > 0.0);
        assert_eq!(m.left[400], 0.0);
    }

    #[test]
    fn looping() {
        for over in [
            json!({ "loop": { "enabled": true, "startSec": 0.1, "endSec": 0.3 } }),
            json!({ "loop": { "enabled": true, "startSec": 0.1, "endSec": 0.05 } }),
            json!({ "loop": { "enabled": true, "startSec": 0.9, "endSec": 0.9001 } }),
        ] {
            let project = make_project(json!([sample_raw(over)]), json!([track_raw(json!({}))]));
            let m = mix(&project, &bank1("s1", ramp_source(1000)), limiter_off());
            assert!(m.peak > 0.0);
            assert!(all_finite(&m.left));
        }
    }

    #[test]
    fn trimming() {
        let plain = make_project(
            json!([sample_raw(json!({}))]),
            json!([track_raw(json!({}))]),
        );
        let pm = mix(&plain, &bank1("s1", mono_ramp_source(1000)), limiter_off());
        assert!(pm.left[0].abs() < 0.05);
        assert!(pm.left[400].abs() > 0.0);

        let head = make_project(
            json!([sample_raw(
                json!({ "trim": { "enabled": true, "startSec": 0.5, "endSec": 0 } })
            )]),
            json!([track_raw(json!({}))]),
        );
        let hm = mix(&head, &bank1("s1", mono_ramp_source(1000)), limiter_off());
        assert!(hm.left[0] > 0.4);

        let tail = make_project(
            json!([sample_raw(
                json!({ "trim": { "enabled": true, "startSec": 0, "endSec": 0.3 } })
            )]),
            json!([track_raw(json!({}))]),
        );
        let tm = mix(&tail, &bank1("s1", mono_ramp_source(1000)), limiter_off());
        assert!(tm.left[100].abs() > 0.0);
        assert_eq!(tm.left[400], 0.0);

        let disabled = make_project(
            json!([sample_raw(
                json!({ "trim": { "enabled": false, "startSec": 0.5, "endSec": 0.6 } })
            )]),
            json!([track_raw(json!({}))]),
        );
        let dm = mix(
            &disabled,
            &bank1("s1", mono_ramp_source(1000)),
            limiter_off(),
        );
        assert!(dm.left[0].abs() < 0.05);
        assert!(dm.left[400].abs() > 0.0);

        let looped = make_project(
            json!([sample_raw(json!({
                "trim": { "enabled": true, "startSec": 0.2, "endSec": 0.6 },
                "loop": { "enabled": true, "startSec": 0.0, "endSec": 1.0 }
            }))]),
            json!([track_raw(
                json!({ "notes": [{ "pitch": 60, "startSec": 0, "durationSec": 0.8, "velocity": 127 }] })
            )]),
        );
        let lm = mix(&looped, &bank1("s1", mono_ramp_source(1000)), limiter_off());
        assert!(all_finite(&lm.left));
        assert!(lm.left.iter().all(|&v| v.abs() < 0.7));
    }

    #[test]
    fn interpolation_quality() {
        let curved: Vec<f32> = (0..200).map(|i| (i as f64 * 0.3).sin() as f32).collect();
        let src = PcmAudio {
            sample_rate: 1000.0,
            channels: vec![curved],
            frames: 200,
        };
        let base = json!({ "tuneCents": 100, "envelope": { "attackMs": 0, "releaseMs": 0 } });
        let hermite = make_project(
            json!([sample_raw(merge(
                base.clone(),
                json!({ "interpolation": "hermite" })
            ))]),
            json!([track_raw(json!({}))]),
        );
        let linear = make_project(
            json!([sample_raw(merge(
                base,
                json!({ "interpolation": "linear" })
            ))]),
            json!([track_raw(json!({}))]),
        );
        let hm = mix(&hermite, &bank1("s1", src.clone()), limiter_off());
        let lm = mix(&linear, &bank1("s1", src), limiter_off());
        let diverges = (5..150).any(|i| (hm.left[i] as f64 - lm.left[i] as f64).abs() > 1e-7);
        assert!(diverges);
        assert!(all_finite(&hm.left) && all_finite(&lm.left));
    }

    #[test]
    fn sinc_interpolation_renders_finite_and_distinct() {
        let curved: Vec<f32> = (0..400).map(|i| (i as f64 * 0.2).sin() as f32).collect();
        let src = PcmAudio {
            sample_rate: 1000.0,
            channels: vec![curved],
            frames: 400,
        };
        let base = json!({ "tuneCents": 100, "envelope": { "attackMs": 0, "releaseMs": 0 } });
        let sinc = make_project(
            json!([sample_raw(merge(
                base.clone(),
                json!({ "interpolation": "sinc" })
            ))]),
            json!([track_raw(json!({}))]),
        );
        let linear = make_project(
            json!([sample_raw(merge(
                base,
                json!({ "interpolation": "linear" })
            ))]),
            json!([track_raw(json!({}))]),
        );
        let sm = mix(&sinc, &bank1("s1", src.clone()), limiter_off());
        let lm = mix(&linear, &bank1("s1", src), limiter_off());
        assert!(all_finite(&sm.left));
        assert!(sm.peak > 0.0);
        assert!((20..150).any(|i| (sm.left[i] as f64 - lm.left[i] as f64).abs() > 1e-6));
    }

    #[test]
    fn drum_mode_ignores_note_pitch_for_speed() {
        let note = json!([{ "pitch": 72, "startSec": 0, "durationSec": 0.5, "velocity": 127 }]);
        let pitched = make_project(
            json!([sample_raw(json!({}))]),
            json!([track_raw(json!({ "notes": note.clone() }))]),
        );
        let drum = make_project(
            json!([sample_raw(json!({}))]),
            json!([track_raw(json!({ "notes": note, "drumMode": true }))]),
        );
        let pm = mix(
            &pitched,
            &bank1("s1", mono_ramp_source(1000)),
            limiter_off(),
        );
        let dm = mix(&drum, &bank1("s1", mono_ramp_source(1000)), limiter_off());
        assert!(close(pm.left[100] as f64, 0.2, 2));
        assert!(close(dm.left[100] as f64, 0.1, 2));
    }

    fn two_const_bank() -> HashMap<String, PcmAudio> {
        let mut bank = HashMap::new();
        bank.insert("s1".to_string(), const_source(1.0, 1000));
        bank.insert("s2".to_string(), const_source(1.0, 1000));
        bank
    }

    #[test]
    fn linked_samples_layer_together() {
        let solo = make_project(
            json!([sample_raw(json!({ "id": "s1" }))]),
            json!([track_raw(json!({ "defaultSampleId": "s1" }))]),
        );
        let layered = make_project(
            json!([
                sample_raw(json!({ "id": "s1", "linkIds": ["s2"] })),
                sample_raw(json!({ "id": "s2" }))
            ]),
            json!([track_raw(json!({ "defaultSampleId": "s1" }))]),
        );
        let bank = two_const_bank();
        let sm = mix(&solo, &bank, limiter_off());
        let lm = mix(&layered, &bank, limiter_off());
        assert!(close(sm.left[100] as f64, 1.0, 4));
        assert!(close(lm.left[100] as f64, 2.0, 4));
    }

    #[test]
    fn linked_samples_handle_cycles() {
        let proj = make_project(
            json!([
                sample_raw(json!({ "id": "s1", "linkIds": ["s2"] })),
                sample_raw(json!({ "id": "s2", "linkIds": ["s1"] }))
            ]),
            json!([track_raw(json!({ "defaultSampleId": "s1" }))]),
        );
        let m = mix(&proj, &two_const_bank(), limiter_off());
        assert!(all_finite(&m.left));
        assert!(close(m.left[100] as f64, 2.0, 4));
    }

    #[test]
    fn layered_voice_duration_drives_polyphony() {
        let project = make_project(
            json!([
                sample_raw(
                    json!({ "id": "s1", "linkIds": ["s2"], "envelope": { "attackMs": 0, "releaseMs": 0 } })
                ),
                sample_raw(json!({ "id": "s2", "envelope": { "attackMs": 0, "releaseMs": 800 } }))
            ]),
            json!([track_raw(json!({
                "defaultSampleId": "s1",
                "polyphony": { "maxVoices": 1, "priority": "oldest", "stopMode": "none" },
                "notes": [
                    { "pitch": 60, "startSec": 0.0, "durationSec": 0.05, "velocity": 127 },
                    { "pitch": 60, "startSec": 0.3, "durationSec": 0.05, "velocity": 127 }
                ]
            }))]),
        );
        let m = mix(&project, &two_const_bank(), limiter_off());
        assert!(m.left[20] > 1.5);
        assert!(m.left[350] < 1.2);
    }

    #[test]
    fn full_envelope() {
        let project = make_project(
            json!([sample_raw(
                json!({ "envelope": { "attackMs": 0, "decayMs": 100, "sustain": 0.5, "releaseMs": 0 } })
            )]),
            json!([track_raw(json!({}))]),
        );
        let m = mix(
            &project,
            &bank1("s1", const_source(1.0, 1000)),
            limiter_off(),
        );
        assert!(close(m.left[200] as f64, 0.5, 3));
        assert!(close(m.left[50] as f64, 0.75, 3));

        let project = make_project(
            json!([sample_raw(
                json!({ "envelope": { "delayMs": 50, "attackMs": 0, "releaseMs": 0 } })
            )]),
            json!([track_raw(json!({}))]),
        );
        let m = mix(
            &project,
            &bank1("s1", const_source(1.0, 1000)),
            limiter_off(),
        );
        assert_eq!(m.left[20], 0.0);
        assert!(close(m.left[80] as f64, 1.0, 3));
    }

    #[test]
    fn dynamic_pitch() {
        let src = mono_ramp_source(1000);
        let glided = make_project(
            json!([sample_raw(
                json!({ "envelope": { "attackMs": 0, "releaseMs": 0 }, "pitchMod": { "glideSemitones": 12, "glideMs": 1000 } })
            )]),
            json!([track_raw(json!({}))]),
        );
        let plain = make_project(
            json!([sample_raw(
                json!({ "envelope": { "attackMs": 0, "releaseMs": 0 } })
            )]),
            json!([track_raw(json!({}))]),
        );
        let gm = mix(&glided, &bank1("s1", src.clone()), limiter_off());
        let pm = mix(&plain, &bank1("s1", src.clone()), limiter_off());
        assert!(gm.left[100] > pm.left[100]);

        let vibrato = make_project(
            json!([sample_raw(
                json!({ "envelope": { "attackMs": 0, "releaseMs": 0 }, "pitchMod": { "vibratoCents": 200, "vibratoHz": 8 } })
            )]),
            json!([track_raw(json!({}))]),
        );
        let vm = mix(&vibrato, &bank1("s1", src.clone()), limiter_off());
        let pm = mix(&plain, &bank1("s1", src), limiter_off());
        let wobbles = (10..400).any(|i| (vm.left[i] as f64 - pm.left[i] as f64).abs() > 1e-6);
        assert!(wobbles);
    }

    #[test]
    fn per_sample_filter() {
        let filtered = make_project(
            json!([sample_raw(
                json!({ "envelope": { "attackMs": 0, "releaseMs": 0 }, "filter": { "enabled": true, "type": "lowpass", "cutoffHz": 50 } })
            )]),
            json!([track_raw(json!({}))]),
        );
        let open = make_project(
            json!([sample_raw(
                json!({ "envelope": { "attackMs": 0, "releaseMs": 0 } })
            )]),
            json!([track_raw(json!({}))]),
        );
        let fm = mix(&filtered, &bank1("s1", nyquist_source(1000)), limiter_off());
        let om = mix(&open, &bank1("s1", nyquist_source(1000)), limiter_off());
        assert!(fm.peak < om.peak);
        assert!(all_finite(&fm.left));

        let swept = make_project(
            json!([sample_raw(
                json!({ "envelope": { "attackMs": 0, "releaseMs": 0 }, "filter": { "enabled": true, "type": "lowpass", "cutoffHz": 80, "envAmount": 4 } })
            )]),
            json!([track_raw(json!({}))]),
        );
        let closed = make_project(
            json!([sample_raw(
                json!({ "envelope": { "attackMs": 0, "releaseMs": 0 }, "filter": { "enabled": true, "type": "lowpass", "cutoffHz": 80, "envAmount": 0 } })
            )]),
            json!([track_raw(json!({}))]),
        );
        let sm = mix(
            &swept,
            &bank1("s1", bright_source(1000, 1000.0)),
            limiter_off(),
        );
        let cm = mix(
            &closed,
            &bank1("s1", bright_source(1000, 1000.0)),
            limiter_off(),
        );
        assert!(sm.peak > cm.peak);

        let lfo = make_project(
            json!([sample_raw(
                json!({ "durationSec": 2, "envelope": { "attackMs": 0, "releaseMs": 0 }, "filter": { "enabled": true, "type": "lowpass", "cutoffHz": 200, "lfoDepth": 4, "lfoHz": 2, "lfoShape": "sine" } })
            )]),
            json!([track_raw(
                json!({ "notes": [{ "pitch": 60, "startSec": 0, "durationSec": 1.5, "velocity": 127 }] })
            )]),
        );
        let m = mix(
            &lfo,
            &bank1("s1", bright_source(2000, 1000.0)),
            limiter_off(),
        );
        let energy = |center: usize| -> f64 {
            (center - 20..center + 20)
                .map(|i| (m.left[i] as f64) * (m.left[i] as f64))
                .sum()
        };
        assert!(energy(625) > energy(875) * 2.0);

        let project = parse_project(json!({
            "version": 1, "name": "test", "sampleRate": 8000, "masterGain": 1,
            "samples": [sample_raw(json!({ "envelope": { "attackMs": 0, "releaseMs": 0 }, "filter": { "enabled": true, "type": "lowpass", "cutoffHz": 6000 } }))],
            "tracks": [track_raw(json!({}))]
        }))
        .unwrap();
        let m = mix_project(
            &project,
            &bank1("s1", bright_source(4000, 8000.0)),
            &limiter_off(),
        );
        assert!(all_finite(&m.left));
        assert!(m.peak < 8.0);
    }

    fn reverb_project(reverb: Option<Value>, reverb_send: f64) -> Project {
        let mut base = json!({
            "version": 1, "name": "r", "sampleRate": 1000,
            "samples": [sample_raw(json!({ "envelope": { "attackMs": 0, "releaseMs": 0 } }))],
            "tracks": [track_raw(json!({ "reverbSend": reverb_send, "notes": [{ "pitch": 60, "startSec": 0, "durationSec": 0.05, "velocity": 127 }] }))]
        });
        if let (Value::Object(b), Some(r)) = (&mut base, reverb) {
            b.insert("reverb".to_string(), r);
        }
        parse_project(base).unwrap()
    }

    #[test]
    fn reverb_send() {
        let p = reverb_project(
            Some(json!({ "enabled": true, "roomSize": 0.8, "wet": 1, "damping": 0.2 })),
            1.0,
        );
        let m = mix(&p, &bank1("s1", const_source(1.0, 1000)), limiter_off());
        assert!(tail_energy(&m.left, 600) > 0.0);

        let p = reverb_project(
            Some(json!({ "enabled": true, "roomSize": 0.8, "wet": 1 })),
            0.0,
        );
        assert_eq!(
            tail_energy(
                &mix(&p, &bank1("s1", const_source(1.0, 1000)), limiter_off()).left,
                600
            ),
            0.0
        );

        let p = reverb_project(None, 1.0);
        assert_eq!(
            tail_energy(
                &mix(&p, &bank1("s1", const_source(1.0, 1000)), limiter_off()).left,
                600
            ),
            0.0
        );

        let p = reverb_project(
            Some(json!({ "enabled": true, "roomSize": 1, "wet": 1 })),
            1.0,
        );
        assert!(mix(&p, &bank1("s1", const_source(1.0, 1000)), limiter_off()).duration_sec > 10.0);

        let p = reverb_project(
            Some(json!({ "enabled": true, "roomSize": 1, "wet": 1 })),
            0.0,
        );
        assert!(mix(&p, &bank1("s1", const_source(1.0, 1000)), limiter_off()).duration_sec < 2.0);

        let p = reverb_project(
            Some(json!({ "enabled": true, "roomSize": 1, "wet": 0 })),
            1.0,
        );
        let m = mix(&p, &bank1("s1", const_source(1.0, 1000)), limiter_off());
        assert!(m.duration_sec < 2.0);
        assert_eq!(tail_energy(&m.left, 600), 0.0);

        let muted = parse_project(json!({
            "version": 1, "name": "r", "sampleRate": 1000,
            "reverb": { "enabled": true, "roomSize": 1, "wet": 1 },
            "samples": [sample_raw(json!({ "envelope": { "attackMs": 0, "releaseMs": 0 } }))],
            "tracks": [track_raw(json!({ "muted": true, "reverbSend": 1, "notes": [{ "pitch": 60, "startSec": 0, "durationSec": 0.05, "velocity": 127 }] }))]
        }))
        .unwrap();
        assert!(
            mix(&muted, &bank1("s1", const_source(1.0, 1000)), limiter_off()).duration_sec < 2.0
        );

        let p = reverb_project(
            Some(json!({ "enabled": true, "roomSize": 1, "wet": 1 })),
            1.0,
        );
        let m = mix(&p, &HashMap::new(), limiter_off());
        assert!(m.duration_sec < 2.0);
        assert_eq!(tail_energy(&m.left, 600), 0.0);

        let no_sample = parse_project(json!({
            "version": 1, "name": "r", "sampleRate": 1000,
            "reverb": { "enabled": true, "roomSize": 1, "wet": 1 },
            "samples": [sample_raw(json!({ "envelope": { "attackMs": 0, "releaseMs": 0 } }))],
            "tracks": [track_raw(json!({ "defaultSampleId": null, "reverbSend": 1, "notes": [{ "pitch": 60, "startSec": 0, "durationSec": 0.05, "velocity": 127 }] }))]
        }))
        .unwrap();
        assert!(
            mix(
                &no_sample,
                &bank1("s1", const_source(1.0, 1000)),
                limiter_off()
            )
            .duration_sec
                < 2.0
        );
    }

    fn poly_mix(notes: Value, polyphony: Value) -> Vec<f32> {
        let dry = sample_raw(json!({ "envelope": { "attackMs": 0, "releaseMs": 0 } }));
        let project = make_project(
            json!([dry]),
            json!([track_raw(json!({ "notes": notes, "polyphony": polyphony }))]),
        );
        mix(
            &project,
            &bank1("s1", const_source(1.0, 1000)),
            limiter_off(),
        )
        .left
    }

    #[test]
    fn polyphony_behaviour() {
        let notes = json!([
            { "pitch": 60, "startSec": 0, "durationSec": 1, "velocity": 127 },
            { "pitch": 60, "startSec": 0.5, "durationSec": 0.05, "velocity": 127 }
        ]);
        assert!(poly_mix(notes.clone(), json!({ "stopMode": "none" }))[800].abs() > 0.0);
        assert_eq!(poly_mix(notes, json!({ "stopMode": "pitch" }))[800], 0.0);

        let chord = json!([
            { "pitch": 60, "startSec": 0, "durationSec": 1, "velocity": 127 },
            { "pitch": 64, "startSec": 0.2, "durationSec": 1, "velocity": 127 },
            { "pitch": 67, "startSec": 0.4, "durationSec": 1, "velocity": 127 }
        ]);
        assert!(close(
            poly_mix(chord.clone(), json!({ "maxVoices": 0 }))[600] as f64,
            3.0,
            5
        ));
        assert!(close(
            poly_mix(chord, json!({ "maxVoices": 2 }))[600] as f64,
            2.0,
            5
        ));

        let project = make_project(
            json!([
                sample_raw(
                    json!({ "id": "long", "envelope": { "attackMs": 0, "releaseMs": 500 } })
                ),
                sample_raw(json!({ "id": "short", "envelope": { "attackMs": 0, "releaseMs": 0 } }))
            ]),
            json!([track_raw(json!({
                "defaultSampleId": "long", "noteSampleMap": { "64": "short" },
                "notes": [
                    { "pitch": 60, "startSec": 0, "durationSec": 2, "velocity": 127 },
                    { "pitch": 64, "startSec": 0.5, "durationSec": 0.1, "velocity": 127 }
                ],
                "polyphony": { "maxVoices": 1, "priority": "newest", "stopMode": "none" }
            }))]),
        );
        let mut b = HashMap::new();
        b.insert("long".to_string(), const_source(1.0, 3000));
        b.insert("short".to_string(), const_source(1.0, 3000));
        assert_eq!(mix(&project, &b, limiter_off()).left[800], 0.0);

        let project = make_project(
            json!([sample_raw(
                json!({ "id": "hit", "envelope": { "attackMs": 0, "releaseMs": 0 } })
            )]),
            json!([track_raw(json!({
                "defaultSampleId": "hit",
                "notes": [
                    { "pitch": 60, "startSec": 0, "durationSec": 2, "velocity": 127 },
                    { "pitch": 60, "startSec": 1, "durationSec": 0.2, "velocity": 127 }
                ],
                "polyphony": { "maxVoices": 1, "priority": "oldest", "stopMode": "none" }
            }))]),
        );
        assert!(
            mix(
                &project,
                &bank1("hit", const_source(1.0, 200)),
                limiter_off()
            )
            .left[1100]
                > 0.0
        );

        let project = make_project(
            json!([
                sample_raw(
                    json!({ "id": "pad", "envelope": { "attackMs": 0, "releaseMs": 0 }, "loop": { "enabled": true, "startSec": 0, "endSec": 0.2 } })
                ),
                sample_raw(json!({ "id": "beep", "envelope": { "attackMs": 0, "releaseMs": 0 } }))
            ]),
            json!([track_raw(json!({
                "defaultSampleId": "pad", "noteSampleMap": { "72": "beep" },
                "notes": [
                    { "pitch": 60, "startSec": 0, "durationSec": 2, "velocity": 127 },
                    { "pitch": 72, "startSec": 1, "durationSec": 0.5, "velocity": 127 }
                ],
                "polyphony": { "maxVoices": 1, "priority": "oldest", "stopMode": "none" }
            }))]),
        );
        let mut b = HashMap::new();
        b.insert("pad".to_string(), const_source(1.0, 200));
        b.insert("beep".to_string(), const_source(0.5, 2000));
        assert!(close(
            mix(&project, &b, limiter_off()).left[1100] as f64,
            1.0,
            5
        ));

        let project = make_project(
            json!([
                sample_raw(json!({ "id": "rel", "envelope": { "attackMs": 0, "releaseMs": 500 } })),
                sample_raw(json!({ "id": "dry", "envelope": { "attackMs": 0, "releaseMs": 0 } }))
            ]),
            json!([track_raw(json!({
                "defaultSampleId": "rel", "noteSampleMap": { "64": "dry" },
                "notes": [
                    { "pitch": 60, "startSec": 0, "durationSec": 0.1, "velocity": 127 },
                    { "pitch": 64, "startSec": 0.3, "durationSec": 0.1, "velocity": 127 }
                ],
                "polyphony": { "maxVoices": 1, "priority": "newest", "stopMode": "none" }
            }))]),
        );
        let mut b = HashMap::new();
        b.insert("rel".to_string(), const_source(1.0, 3000));
        b.insert("dry".to_string(), const_source(1.0, 3000));
        assert_eq!(mix(&project, &b, limiter_off()).left[500], 0.0);

        let project = make_project(
            json!([sample_raw(json!({}))]),
            json!([track_raw(json!({
                "notes": [
                    { "pitch": 60, "startSec": 0, "durationSec": 5, "velocity": 127 },
                    { "pitch": 64, "startSec": 0, "durationSec": 0.1, "velocity": 127 }
                ],
                "polyphony": { "maxVoices": 1, "priority": "newest", "stopMode": "none" }
            }))]),
        );
        assert!(
            mix(
                &project,
                &bank1("s1", const_source(1.0, 6000)),
                limiter_off()
            )
            .frames
                < 1000
        );

        let project = make_project(
            json!([
                sample_raw(
                    json!({ "id": "lead", "envelope": { "attackMs": 0, "releaseMs": 0 }, "pitchMod": { "glideSemitones": -5, "glideMs": 1000 } })
                ),
                sample_raw(json!({ "id": "beep", "envelope": { "attackMs": 0, "releaseMs": 0 } }))
            ]),
            json!([track_raw(json!({
                "defaultSampleId": "lead", "noteSampleMap": { "64": "beep" },
                "notes": [
                    { "pitch": 60, "startSec": 0, "durationSec": 1, "velocity": 127 },
                    { "pitch": 64, "startSec": 0.5, "durationSec": 0.3, "velocity": 127 }
                ],
                "polyphony": { "maxVoices": 1, "priority": "oldest", "stopMode": "none" }
            }))]),
        );
        let mut b = HashMap::new();
        b.insert("lead".to_string(), const_source(1.0, 100));
        b.insert("beep".to_string(), const_source(0.5, 2000));
        assert_eq!(mix(&project, &b, limiter_off()).left[600], 0.0);
    }

    #[test]
    fn waveform_peaks_capture_bucket_maxima() {
        let pcm = PcmAudio {
            sample_rate: 1000.0,
            channels: vec![vec![0.1, 0.9, -0.5, 0.3]],
            frames: 4,
        };
        let peaks = build_waveform_peaks(&pcm, 2);
        assert_eq!(peaks.len(), 2);
        assert!(close(peaks[0] as f64, 0.9, 6));
        assert!(close(peaks[1] as f64, 0.5, 6));
    }

    #[test]
    fn waveform_peaks_handle_empty_and_oversampled() {
        let empty = PcmAudio {
            sample_rate: 1000.0,
            channels: vec![],
            frames: 0,
        };
        assert_eq!(build_waveform_peaks(&empty, 8), vec![0.0f32; 8]);

        let silent = PcmAudio {
            sample_rate: 1000.0,
            channels: vec![vec![]],
            frames: 0,
        };
        assert_eq!(build_waveform_peaks(&silent, 4), vec![0.0f32; 4]);

        let pcm = const_source(0.7, 3);
        let peaks = build_waveform_peaks(&pcm, 16);
        assert_eq!(peaks.len(), 16);
        assert!(peaks.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn pcm_duration_seconds() {
        assert!(close(const_source(1.0, 480).duration_sec(), 0.48, 9));
        let zero_rate = PcmAudio {
            sample_rate: 0.0,
            channels: vec![vec![0.0; 10]],
            frames: 10,
        };
        assert_eq!(zero_rate.duration_sec(), 0.0);
    }

    #[test]
    fn map_bank_wraps_hashmap() {
        let bank = MapBank(bank1("s1", const_source(1.0, 4)));
        assert!(AudioBank::get(&bank, "s1").is_some());
        assert!(AudioBank::get(&bank, "nope").is_none());

        let project = make_project(
            json!([sample_raw(json!({}))]),
            json!([track_raw(json!({}))]),
        );
        let m = mix_project(&project, &bank, &limiter_off());
        assert!(m.peak > 0.0);
    }

    #[test]
    fn master_gain_scales_output() {
        let make = |gain: f64| {
            parse_project(json!({
                "version": 1, "name": "g", "sampleRate": 1000, "masterGain": gain,
                "samples": [sample_raw(json!({}))], "tracks": [track_raw(json!({}))]
            }))
            .unwrap()
        };
        let full = mix(
            &make(1.0),
            &bank1("s1", const_source(1.0, 1000)),
            limiter_off(),
        );
        let half = mix(
            &make(0.5),
            &bank1("s1", const_source(1.0, 1000)),
            limiter_off(),
        );
        assert!(close(full.left[100] as f64, 1.0, 5));
        assert!(close(half.left[100] as f64, 0.5, 5));
    }

    #[test]
    fn multiple_tracks_sum() {
        let project = make_project(
            json!([sample_raw(json!({}))]),
            json!([
                track_raw(json!({ "id": "a", "pan": 0 })),
                track_raw(json!({ "id": "b", "pan": 0 }))
            ]),
        );
        let m = mix(
            &project,
            &bank1("s1", const_source(0.4, 1000)),
            limiter_off(),
        );
        assert!(close(m.left[100] as f64, 0.8, 4));
    }

    #[test]
    fn peak_is_measured_before_limiter() {
        let chord = json!([
            { "pitch": 60, "startSec": 0, "durationSec": 1, "velocity": 127 },
            { "pitch": 60, "startSec": 0, "durationSec": 1, "velocity": 127 },
            { "pitch": 60, "startSec": 0, "durationSec": 1, "velocity": 127 }
        ]);
        let project = make_project(
            json!([sample_raw(json!({}))]),
            json!([track_raw(
                json!({ "notes": chord, "polyphony": { "maxVoices": 0 } })
            )]),
        );
        let m = mix(
            &project,
            &bank1("s1", const_source(1.0, 1000)),
            MixOptions {
                tail_sec: None,
                limiter: Some(true),
            },
        );
        assert!(m.peak > 2.9);
        assert!(max_abs(&m.left) <= 1.0);
        assert!((m.peak as f32) > max_abs(&m.left) * 2.5);
    }

    #[test]
    fn source_rate_differs_from_project_rate() {
        let ch: Vec<f32> = (0..2000)
            .map(|i| ((i as f64) * 0.05).sin() as f32 * 0.5)
            .collect();
        let src = PcmAudio {
            sample_rate: 2000.0,
            channels: vec![ch],
            frames: 2000,
        };
        let project = make_project(
            json!([sample_raw(json!({}))]),
            json!([track_raw(json!({}))]),
        );
        let m = mix(&project, &bank1("s1", src), limiter_off());
        assert!(m.peak > 0.0);
        assert!(all_finite(&m.left));
    }

    #[test]
    fn velocity_scales_amplitude() {
        let make = |vel: i32| {
            make_project(
                json!([sample_raw(json!({}))]),
                json!([track_raw(json!({
                    "notes": [{ "pitch": 60, "startSec": 0, "durationSec": 0.5, "velocity": vel }]
                }))]),
            )
        };
        let loud = mix(
            &make(127),
            &bank1("s1", const_source(1.0, 1000)),
            limiter_off(),
        );
        let soft = mix(
            &make(64),
            &bank1("s1", const_source(1.0, 1000)),
            limiter_off(),
        );
        assert!(loud.left[50].abs() > soft.left[50].abs());
        assert!(close(soft.left[50] as f64, velocity_to_gain(64.0), 4));
    }

    #[test]
    fn buffer_bounds() {
        let mut project = make_project(
            json!([sample_raw(json!({}))]),
            json!([track_raw(json!({}))]),
        );
        project.tracks[0].notes[0].start_sec = -0.05;
        let m = mix(
            &project,
            &bank1("s1", const_source(1.0, 1000)),
            limiter_off(),
        );
        assert!(m.peak > 0.0);
        assert!(all_finite(&m.left));

        let project = make_project(
            json!([sample_raw(json!({}))]),
            json!([track_raw(json!({}))]),
        );
        let m = mix(
            &project,
            &bank1("s1", const_source(1.0, 1000)),
            MixOptions {
                tail_sec: Some(-0.4),
                limiter: Some(false),
            },
        );
        assert_eq!(m.frames, 101);
        assert!(all_finite(&m.left));

        let m = mix(
            &project,
            &bank1("s1", const_source(1.0, 1000)),
            MixOptions {
                tail_sec: Some(-10.0),
                limiter: Some(false),
            },
        );
        assert!(m.frames >= 1);
    }

    fn tone_src(freq: f64, rate: f64, frames: usize) -> PcmAudio {
        mono_source(frames, rate, |i| {
            ((2.0 * std::f64::consts::PI * freq * i as f64) / rate).sin() as f32
        })
    }

    fn zero_crossings(arr: &[f32], start: usize, end: usize) -> usize {
        let mut count = 0;
        for i in start + 1..end.min(arr.len()) {
            if (arr[i - 1] <= 0.0) != (arr[i] <= 0.0) {
                count += 1;
            }
        }
        count
    }

    #[test]
    fn one_shot_ignores_short_note_gate() {
        let short = json!([{ "pitch": 60, "startSec": 0, "durationSec": 0.05, "velocity": 127 }]);
        let gated = make_project(
            json!([sample_raw(
                json!({ "envelope": { "attackMs": 0, "releaseMs": 0 } })
            )]),
            json!([track_raw(json!({ "notes": short.clone() }))]),
        );
        let oneshot = make_project(
            json!([sample_raw(
                json!({ "envelope": { "attackMs": 0, "releaseMs": 0 }, "oneShot": true })
            )]),
            json!([track_raw(json!({ "notes": short }))]),
        );
        let g = mix(&gated, &bank1("s1", const_source(1.0, 1000)), limiter_off());
        let o = mix(
            &oneshot,
            &bank1("s1", const_source(1.0, 1000)),
            limiter_off(),
        );
        assert!(
            g.left[200].abs() < 0.05,
            "gated note should stop after its gate"
        );
        assert!(o.left[200].abs() > 0.5, "one-shot should keep playing");
        assert!(
            o.left[900].abs() > 0.5,
            "one-shot should play the full sample"
        );
    }

    #[test]
    fn granular_decouples_duration_from_pitch() {
        let high = json!([{ "pitch": 72, "startSec": 0, "durationSec": 0.05, "velocity": 127 }]);
        let make = |over: Value| {
            make_project(
                json!([sample_raw(merge(
                    json!({ "envelope": { "attackMs": 0, "releaseMs": 0 }, "oneShot": true }),
                    over,
                ))]),
                json!([track_raw(json!({ "notes": high.clone() }))]),
            )
        };
        let varispeed = mix(
            &make(json!({})),
            &bank1("s1", const_source(1.0, 1000)),
            limiter_off(),
        );
        let granular = mix(
            &make(json!({ "pitchMode": "granular", "speed": 1.0 })),
            &bank1("s1", const_source(1.0, 1000)),
            limiter_off(),
        );
        assert!(
            varispeed.left[600].abs() < 0.05,
            "+12st varispeed one-shot plays twice as fast and is done by frame 600"
        );
        assert!(
            granular.left[600].abs() > 0.5,
            "granular keeps the original tempo regardless of pitch"
        );
        assert!(
            granular.left[900].abs() > 0.5,
            "granular plays the full natural length"
        );
    }

    #[test]
    fn granular_shifts_pitch_up() {
        let make = |pitch: i64| {
            make_project(
                json!([sample_raw(
                    json!({ "envelope": { "attackMs": 0, "releaseMs": 0 }, "oneShot": true, "pitchMode": "granular", "speed": 1.0, "grainMs": 50 })
                )]),
                json!([track_raw(
                    json!({ "notes": [{ "pitch": pitch, "startSec": 0, "durationSec": 0.05, "velocity": 127 }] })
                )]),
            )
        };
        let base = mix(
            &make(60),
            &bank1("s1", tone_src(50.0, 1000.0, 1000)),
            limiter_off(),
        );
        let octave = mix(
            &make(72),
            &bank1("s1", tone_src(50.0, 1000.0, 1000)),
            limiter_off(),
        );
        let zc_base = zero_crossings(&base.left, 200, 700);
        let zc_octave = zero_crossings(&octave.left, 200, 700);
        assert!(zc_base > 0);
        assert!(
            zc_octave as f64 > zc_base as f64 * 1.5,
            "an octave up should roughly double the zero-crossing rate (base {zc_base}, octave {zc_octave})"
        );
    }

    #[test]
    fn reverse_reads_from_the_end_first() {
        let note = json!([{ "pitch": 60, "startSec": 0, "durationSec": 0.5, "velocity": 127 }]);
        let forward = make_project(
            json!([sample_raw(
                json!({ "envelope": { "attackMs": 0, "releaseMs": 0 } })
            )]),
            json!([track_raw(json!({ "notes": note.clone() }))]),
        );
        let reversed = make_project(
            json!([sample_raw(
                json!({ "envelope": { "attackMs": 0, "releaseMs": 0 }, "reverse": true })
            )]),
            json!([track_raw(json!({ "notes": note }))]),
        );
        let f = mix(
            &forward,
            &bank1("s1", mono_ramp_source(1000)),
            limiter_off(),
        );
        let r = mix(
            &reversed,
            &bank1("s1", mono_ramp_source(1000)),
            limiter_off(),
        );
        assert!(
            f.left[10].abs() < 0.2,
            "forward starts at the ramp's low head"
        );
        assert!(
            r.left[10].abs() > 0.8,
            "reverse starts at the ramp's high tail"
        );
    }
}
