//! 時間変化するピッチモジュレーション（半音単位）。一発のグライドとビブラート LFO を合成。

use super::curve::shape_curve;
use super::lfo::lfo_value;
use crate::schema::PitchMod;

fn glide_offset(params: &PitchMod, t: f64) -> f64 {
    if params.glide_ms <= 0.0 {
        return 0.0;
    }
    let progress = t / (params.glide_ms / 1000.0);
    params.glide_semitones * (1.0 - shape_curve(progress, params.glide_curve))
}

fn vibrato_fade(t: f64, delay_sec: f64, fade_sec: f64) -> f64 {
    if t <= delay_sec {
        return 0.0;
    }
    if fade_sec <= 0.0 {
        return 1.0;
    }
    let f = (t - delay_sec) / fade_sec;
    if f >= 1.0 {
        1.0
    } else {
        f
    }
}

fn vibrato_offset(params: &PitchMod, t: f64) -> f64 {
    let depth_semitones = params.vibrato_cents / 100.0;
    let fade = vibrato_fade(
        t,
        params.vibrato_delay_ms / 1000.0,
        params.vibrato_fade_ms / 1000.0,
    );
    depth_semitones * fade * lfo_value(params.vibrato_shape, t * params.vibrato_hz)
}

/// 時刻 `t`（ノートオンからの秒）におけるピッチオフセット（半音）。
pub fn pitch_offset_semitones(params: &PitchMod, t: f64) -> f64 {
    glide_offset(params, t) + vibrato_offset(params, t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::LfoShape;

    fn mod_params(f: impl FnOnce(&mut PitchMod)) -> PitchMod {
        let mut p = PitchMod {
            glide_semitones: 0.0,
            glide_ms: 0.0,
            glide_curve: 0.0,
            vibrato_cents: 0.0,
            vibrato_hz: 5.0,
            vibrato_delay_ms: 0.0,
            vibrato_fade_ms: 0.0,
            vibrato_shape: LfoShape::Sine,
        };
        f(&mut p);
        p
    }

    fn close(a: f64, b: f64, prec: i32) -> bool {
        (a - b).abs() < 10f64.powi(-prec) / 2.0
    }

    #[test]
    fn glide() {
        let params = mod_params(|p| {
            p.glide_semitones = 12.0;
            p.glide_ms = 100.0;
        });
        assert!(close(pitch_offset_semitones(&params, 0.0), 12.0, 9));
        assert!(close(pitch_offset_semitones(&params, 0.05), 6.0, 9));
        assert!(close(pitch_offset_semitones(&params, 0.1), 0.0, 9));
        assert!(close(pitch_offset_semitones(&params, 0.2), 0.0, 9));

        assert!(close(
            pitch_offset_semitones(
                &mod_params(|p| {
                    p.glide_semitones = 12.0;
                    p.glide_ms = 0.0;
                }),
                0.0
            ),
            0.0,
            9
        ));

        let value = pitch_offset_semitones(
            &mod_params(|p| {
                p.glide_semitones = 12.0;
                p.glide_ms = 100.0;
                p.glide_curve = 4.0;
            }),
            0.05,
        );
        assert!(value > 6.0);
    }

    #[test]
    fn vibrato() {
        let params = mod_params(|p| {
            p.vibrato_cents = 100.0;
            p.vibrato_hz = 5.0;
        });
        assert!(close(pitch_offset_semitones(&params, 0.05), 1.0, 6));
        assert!(close(pitch_offset_semitones(&params, 0.15), -1.0, 6));

        assert_eq!(
            pitch_offset_semitones(
                &mod_params(|p| {
                    p.vibrato_cents = 100.0;
                    p.vibrato_delay_ms = 100.0;
                }),
                0.05
            ),
            0.0
        );

        assert!(close(
            pitch_offset_semitones(
                &mod_params(|p| {
                    p.vibrato_cents = 100.0;
                    p.vibrato_fade_ms = 100.0;
                }),
                0.05
            ),
            0.5,
            6
        ));

        assert!(close(
            pitch_offset_semitones(
                &mod_params(|p| {
                    p.vibrato_cents = 100.0;
                    p.vibrato_fade_ms = 50.0;
                }),
                0.15
            ),
            -1.0,
            6
        ));
    }

    #[test]
    fn combined() {
        let params = mod_params(|p| {
            p.glide_semitones = 12.0;
            p.glide_ms = 100.0;
            p.vibrato_cents = 100.0;
            p.vibrato_hz = 5.0;
        });
        assert!(close(pitch_offset_semitones(&params, 0.05), 7.0, 6));
    }
}
