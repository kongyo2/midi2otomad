use std::f64::consts::PI;

pub const DEFAULT_OVERLAP: f64 = 4.0;

fn hann(phase: f64) -> f64 {
    if phase <= 0.0 || phase >= 1.0 {
        0.0
    } else {
        0.5 - 0.5 * (2.0 * PI * phase).cos()
    }
}

struct Grain {
    pos: f64,
    age: f64,
}

pub struct GrainCloud {
    grain_len: f64,
    hop: f64,
    norm: f64,
    grains: Vec<Grain>,
    frames_to_next: f64,
}

impl GrainCloud {
    pub fn new(grain_len: f64, overlap: f64) -> Self {
        let grain_len = grain_len.max(2.0);
        let overlap = overlap.max(1.0);
        let hop = (grain_len / overlap).max(1.0);
        let norm = (0.5 * grain_len / hop).max(1e-6);
        Self {
            grain_len,
            hop,
            norm,
            grains: Vec::with_capacity(overlap.ceil() as usize + 2),
            frames_to_next: 0.0,
        }
    }

    pub fn active(&self) -> bool {
        !self.grains.is_empty()
    }

    pub fn process<F>(&mut self, spawn_pos: f64, read_advance: f64, mut read: F) -> (f64, f64)
    where
        F: FnMut(f64) -> (f64, f64),
    {
        if self.frames_to_next <= 0.0 {
            self.grains.push(Grain {
                pos: spawn_pos,
                age: 0.0,
            });
            self.frames_to_next += self.hop;
        }
        self.frames_to_next -= 1.0;

        let grain_len = self.grain_len;
        let mut acc_l = 0.0;
        let mut acc_r = 0.0;
        self.grains.retain_mut(|g| {
            let w = hann(g.age / grain_len);
            let (l, r) = read(g.pos);
            acc_l += l * w;
            acc_r += r * w;
            g.pos += read_advance;
            g.age += 1.0;
            g.age < grain_len
        });
        (acc_l / self.norm, acc_r / self.norm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }

    #[test]
    fn hann_endpoints_and_peak() {
        assert_eq!(hann(0.0), 0.0);
        assert_eq!(hann(1.0), 0.0);
        assert!(close(hann(0.5), 1.0, 1e-9));
        assert!(close(hann(0.25), 0.5, 1e-9));
    }

    #[test]
    fn reproduces_constant_in_steady_state() {
        let mut cloud = GrainCloud::new(64.0, DEFAULT_OVERLAP);
        let mut last = 0.0;
        for i in 0..512 {
            let pos = i as f64;
            let (l, _r) = cloud.process(pos, 1.0, |_p| (0.8, 0.8));
            last = l;
        }
        assert!(close(last, 0.8, 0.02), "steady-state output was {last}");
    }

    #[test]
    fn stereo_channels_are_independent() {
        let mut cloud = GrainCloud::new(32.0, DEFAULT_OVERLAP);
        let mut out = (0.0, 0.0);
        for i in 0..256 {
            out = cloud.process(i as f64, 1.0, |_p| (0.5, -0.3));
        }
        assert!(close(out.0, 0.5, 0.03));
        assert!(close(out.1, -0.3, 0.03));
    }

    #[test]
    fn unity_speed_unity_pitch_tracks_a_ramp() {
        let data: Vec<f64> = (0..1024).map(|i| i as f64 / 1024.0).collect();
        let read = |p: f64| {
            let idx = (p.round() as usize).min(data.len() - 1);
            (data[idx], data[idx])
        };
        let mut cloud = GrainCloud::new(64.0, DEFAULT_OVERLAP);
        let mut time = 0.0;
        let mut max_err: f64 = 0.0;
        for _ in 0..400 {
            let (l, _r) = cloud.process(time, 1.0, read);
            let expected = time / 1024.0;
            if time > 80.0 {
                max_err = max_err.max((l - expected).abs());
            }
            time += 1.0;
        }
        assert!(max_err < 0.02, "ramp tracking error {max_err}");
    }

    #[test]
    fn grains_expire_after_their_lifetime() {
        let mut cloud = GrainCloud::new(16.0, DEFAULT_OVERLAP);
        for i in 0..200 {
            cloud.process(i as f64, 1.0, |_p| (0.0, 0.0));
        }
        assert!(cloud.grains.len() <= DEFAULT_OVERLAP as usize + 2);
        assert!(cloud.active());
    }
}
