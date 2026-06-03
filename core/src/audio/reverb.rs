#[derive(Debug, Clone, Copy)]
pub struct ReverbParams {
    pub room_size: f64,
    pub damping: f64,
    pub width: f64,
    pub wet: f64,
    pub dry: f64,
    pub pre_delay_ms: f64,
}

pub struct ReverbOutput {
    pub left: Vec<f32>,
    pub right: Vec<f32>,
}

const COMB_TUNINGS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASS_TUNINGS: [usize; 4] = [556, 441, 341, 225];
const STEREO_SPREAD: usize = 23;
const FIXED_GAIN: f64 = 0.015;
const SCALE_ROOM: f64 = 0.28;
const OFFSET_ROOM: f64 = 0.7;
const SCALE_DAMP: f64 = 0.4;
const ALLPASS_FEEDBACK: f64 = 0.5;

struct Comb {
    buffer: Vec<f32>,
    index: usize,
    store: f64,
    feedback: f64,
    damp1: f64,
}

impl Comb {
    fn new(size: usize, feedback: f64, damp1: f64) -> Self {
        Self {
            buffer: vec![0.0; size.max(1)],
            index: 0,
            store: 0.0,
            feedback,
            damp1,
        }
    }

    fn process(&mut self, input: f64) -> f64 {
        let output = self.buffer[self.index] as f64;
        self.store = output * (1.0 - self.damp1) + self.store * self.damp1;
        self.buffer[self.index] = (input + self.store * self.feedback) as f32;
        self.index += 1;
        if self.index >= self.buffer.len() {
            self.index = 0;
        }
        output
    }
}

struct Allpass {
    buffer: Vec<f32>,
    index: usize,
}

impl Allpass {
    fn new(size: usize) -> Self {
        Self {
            buffer: vec![0.0; size.max(1)],
            index: 0,
        }
    }

    fn process(&mut self, input: f64) -> f64 {
        let buffered = self.buffer[self.index] as f64;
        let output = -input + buffered;
        self.buffer[self.index] = (input + buffered * ALLPASS_FEEDBACK) as f32;
        self.index += 1;
        if self.index >= self.buffer.len() {
            self.index = 0;
        }
        output
    }
}

pub struct Freeverb {
    combs_l: Vec<Comb>,
    combs_r: Vec<Comb>,
    allpass_l: Vec<Allpass>,
    allpass_r: Vec<Allpass>,
    wet1: f64,
    wet2: f64,
    dry: f64,
    pre_delay: Vec<f32>,
    pre_delay_len: usize,
    pre_delay_index: usize,
}

impl Freeverb {
    pub fn new(sample_rate: f64, params: ReverbParams) -> Self {
        let scale = sample_rate / 44100.0;
        let feedback = params.room_size * SCALE_ROOM + OFFSET_ROOM;
        let damp1 = params.damping * SCALE_DAMP;
        let scaled = |t: usize| (t as f64 * scale).round() as usize;
        let combs_l = COMB_TUNINGS
            .iter()
            .map(|&t| Comb::new(scaled(t), feedback, damp1))
            .collect();
        let combs_r = COMB_TUNINGS
            .iter()
            .map(|&t| Comb::new(scaled(t + STEREO_SPREAD), feedback, damp1))
            .collect();
        let allpass_l = ALLPASS_TUNINGS
            .iter()
            .map(|&t| Allpass::new(scaled(t)))
            .collect();
        let allpass_r = ALLPASS_TUNINGS
            .iter()
            .map(|&t| Allpass::new(scaled(t + STEREO_SPREAD)))
            .collect();
        let pre_delay_len = ((params.pre_delay_ms / 1000.0) * sample_rate).round() as usize;
        Self {
            combs_l,
            combs_r,
            allpass_l,
            allpass_r,
            wet1: params.wet * (params.width / 2.0 + 0.5),
            wet2: params.wet * ((1.0 - params.width) / 2.0),
            dry: params.dry,
            pre_delay: vec![0.0; pre_delay_len.max(1)],
            pre_delay_len,
            pre_delay_index: 0,
        }
    }

    pub fn process_block(&mut self, left: &[f32], right: &[f32]) -> ReverbOutput {
        let n = left.len();
        let mut out_l = vec![0.0f32; n];
        let mut out_r = vec![0.0f32; n];
        for i in 0..n {
            let dry_l = left[i] as f64;
            let dry_r = right[i] as f64;
            let mut input = (dry_l + dry_r) * FIXED_GAIN;
            if self.pre_delay_len > 0 {
                let delayed = self.pre_delay[self.pre_delay_index] as f64;
                self.pre_delay[self.pre_delay_index] = input as f32;
                self.pre_delay_index += 1;
                if self.pre_delay_index >= self.pre_delay_len {
                    self.pre_delay_index = 0;
                }
                input = delayed;
            }
            let mut acc_l = 0.0;
            let mut acc_r = 0.0;
            for comb in &mut self.combs_l {
                acc_l += comb.process(input);
            }
            for comb in &mut self.combs_r {
                acc_r += comb.process(input);
            }
            for allpass in &mut self.allpass_l {
                acc_l = allpass.process(acc_l);
            }
            for allpass in &mut self.allpass_r {
                acc_r = allpass.process(acc_r);
            }
            out_l[i] = (acc_l * self.wet1 + acc_r * self.wet2 + dry_l * self.dry) as f32;
            out_r[i] = (acc_r * self.wet1 + acc_l * self.wet2 + dry_r * self.dry) as f32;
        }
        ReverbOutput {
            left: out_l,
            right: out_r,
        }
    }
}

pub fn create_reverb(sample_rate: f64, params: ReverbParams) -> Freeverb {
    Freeverb::new(sample_rate, params)
}

const SILENCE_THRESHOLD: f64 = 0.001;

fn longest_comb_seconds() -> f64 {
    (COMB_TUNINGS[COMB_TUNINGS.len() - 1] + STEREO_SPREAD) as f64 / 44100.0
}

pub fn reverb_decay_seconds(room_size: f64) -> f64 {
    let feedback = room_size * SCALE_ROOM + OFFSET_ROOM;
    (longest_comb_seconds() * SILENCE_THRESHOLD.ln()) / feedback.ln()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FS: f64 = 48000.0;

    fn rev(f: impl FnOnce(&mut ReverbParams)) -> ReverbParams {
        let mut p = ReverbParams {
            room_size: 0.5,
            damping: 0.5,
            width: 1.0,
            wet: 1.0,
            dry: 0.0,
            pre_delay_ms: 0.0,
        };
        f(&mut p);
        p
    }

    fn impulse(n: usize) -> Vec<f32> {
        let mut b = vec![0.0f32; n];
        b[0] = 1.0;
        b
    }

    fn energy(arr: &[f32], start: usize, end: usize) -> f64 {
        arr[start..end]
            .iter()
            .map(|&v| (v as f64) * (v as f64))
            .sum()
    }

    fn first_audible(arr: &[f32], threshold: f64) -> usize {
        arr.iter()
            .position(|&v| (v as f64).abs() > threshold)
            .unwrap_or(arr.len())
    }

    fn close(a: f64, b: f64, prec: i32) -> bool {
        (a - b).abs() < 10f64.powi(-prec) / 2.0
    }

    #[test]
    fn decaying_tail() {
        let out = create_reverb(FS, rev(|_| {})).process_block(&impulse(16000), &impulse(16000));
        assert!(energy(&out.left, 1200, 16000) > 0.0);
        assert!(energy(&out.left, 8000, 16000) < energy(&out.left, 0, 8000));
    }

    #[test]
    fn larger_room_rings_longer() {
        let small = create_reverb(FS, rev(|p| p.room_size = 0.3))
            .process_block(&impulse(16000), &impulse(16000));
        let large = create_reverb(FS, rev(|p| p.room_size = 0.9))
            .process_block(&impulse(16000), &impulse(16000));
        assert!(energy(&large.left, 8000, 16000) > energy(&small.left, 8000, 16000));
    }

    #[test]
    fn damping_decays_faster() {
        let bright = create_reverb(FS, rev(|p| p.damping = 0.0))
            .process_block(&impulse(16000), &impulse(16000));
        let dark = create_reverb(FS, rev(|p| p.damping = 1.0))
            .process_block(&impulse(16000), &impulse(16000));
        assert!(energy(&dark.left, 0, 16000) < energy(&bright.left, 0, 16000));
    }

    #[test]
    fn dry_passthrough() {
        let out = create_reverb(
            FS,
            rev(|p| {
                p.wet = 0.0;
                p.dry = 1.0;
            }),
        )
        .process_block(&impulse(100), &impulse(100));
        assert!(close(out.left[0] as f64, 1.0, 9));
        assert!(close(out.left[50] as f64, 0.0, 9));
    }

    #[test]
    fn width_controls_correlation() {
        let mono =
            create_reverb(FS, rev(|p| p.width = 0.0)).process_block(&impulse(8000), &impulse(8000));
        for i in [2000, 4000, 6000] {
            assert!(close(mono.left[i] as f64, mono.right[i] as f64, 12));
        }
        let wide =
            create_reverb(FS, rev(|p| p.width = 1.0)).process_block(&impulse(8000), &impulse(8000));
        let differs =
            (1200..8000).any(|i| (wide.left[i] as f64 - wide.right[i] as f64).abs() > 1e-9);
        assert!(differs);
    }

    #[test]
    fn pre_delay_delays_onset() {
        let direct = create_reverb(FS, rev(|_| {})).process_block(&impulse(16000), &impulse(16000));
        let delayed = create_reverb(FS, rev(|p| p.pre_delay_ms = 50.0))
            .process_block(&impulse(16000), &impulse(16000));
        assert!(first_audible(&delayed.left, 1e-4) > first_audible(&direct.left, 1e-4) + 1000);
    }

    #[test]
    fn decay_seconds_scales() {
        assert!(reverb_decay_seconds(1.0) > 10.0);
        assert!(reverb_decay_seconds(0.0) < 1.0);
        assert!(reverb_decay_seconds(0.8) > reverb_decay_seconds(0.4));
        assert!(reverb_decay_seconds(0.4) > reverb_decay_seconds(0.1));
    }

    #[test]
    fn stays_bounded() {
        let out = create_reverb(FS, rev(|p| p.room_size = 0.95))
            .process_block(&impulse(16000), &impulse(16000));
        for &v in &out.left {
            assert!(v.is_finite());
            assert!((v as f64).abs() < 1.0);
        }
    }

    #[test]
    fn fully_dry_and_wet_zero_is_silent() {
        let out = create_reverb(
            FS,
            rev(|p| {
                p.wet = 0.0;
                p.dry = 0.0;
            }),
        )
        .process_block(&impulse(2000), &impulse(2000));
        assert!(out.left.iter().all(|&v| v == 0.0));
        assert!(out.right.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn dry_signal_scales_with_dry_level() {
        let half = create_reverb(
            FS,
            rev(|p| {
                p.wet = 0.0;
                p.dry = 0.5;
            }),
        )
        .process_block(&impulse(64), &impulse(64));
        assert!(close(half.left[0] as f64, 0.5, 9));
    }

    #[test]
    fn runs_at_different_sample_rates() {
        for rate in [22050.0, 44100.0, 48000.0, 96000.0] {
            let out = create_reverb(rate, rev(|p| p.room_size = 0.7))
                .process_block(&impulse(8000), &impulse(8000));
            assert!(out.left.iter().all(|v| v.is_finite()));
            assert!(energy(&out.left, 0, 8000) > 0.0);
        }
    }

    #[test]
    fn zero_pre_delay_has_no_extra_latency() {
        let out = create_reverb(FS, rev(|p| p.pre_delay_ms = 0.0))
            .process_block(&impulse(4000), &impulse(4000));
        let onset = first_audible(&out.left, 1e-4);
        assert!((1000..1300).contains(&onset), "onset was {onset}");
    }

    #[test]
    fn decay_seconds_is_monotonic_in_room_size() {
        let mut prev = 0.0;
        for rs in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let d = reverb_decay_seconds(rs);
            assert!(d > prev);
            prev = d;
        }
    }
}
