//! DAHDSR エンベロープ（ディレイ/アタック/ホールド/ディケイ/サステイン/リリース）。
//! 各傾斜区間に独立したカーブ形状を持つ。

use super::curve::shape_curve;
use crate::schema::Envelope;

/// ノートが保持されている間のレベル（リリース尾部を無視）。
fn pre_release_level(env: &Envelope, t: f64) -> f64 {
    let delay = env.delay_ms / 1000.0;
    let attack = env.attack_ms / 1000.0;
    let hold = env.hold_ms / 1000.0;
    let decay = env.decay_ms / 1000.0;
    let attack_end = delay + attack;
    let hold_end = attack_end + hold;
    let decay_end = hold_end + decay;
    if t < delay {
        return 0.0;
    }
    if t < attack_end {
        return shape_curve((t - delay) / attack, env.attack_curve);
    }
    if t < hold_end {
        return 1.0;
    }
    if t < decay_end {
        return 1.0 - (1.0 - env.sustain) * shape_curve((t - hold_end) / decay, env.decay_curve);
    }
    env.sustain
}

/// `gate_sec` までノートが開いているときの、ノートオンからの経過 `t` 秒における
/// エンベロープゲイン `[0, 1]`。ゲートが閉じた後は到達レベルから 0 へ向けて減衰する。
pub fn envelope_level(env: &Envelope, t: f64, gate_sec: f64) -> f64 {
    if t < gate_sec {
        return pre_release_level(env, t);
    }
    let release = env.release_ms / 1000.0;
    if release <= 0.0 {
        return 0.0;
    }
    let r = (t - gate_sec) / release;
    if r >= 1.0 {
        return 0.0;
    }
    pre_release_level(env, gate_sec) * (1.0 - shape_curve(r, env.release_curve))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(f: impl FnOnce(&mut Envelope)) -> Envelope {
        let mut e = Envelope {
            delay_ms: 0.0,
            attack_ms: 100.0,
            attack_curve: 0.0,
            hold_ms: 0.0,
            decay_ms: 0.0,
            decay_curve: 0.0,
            sustain: 1.0,
            release_ms: 100.0,
            release_curve: 0.0,
        };
        f(&mut e);
        e
    }

    fn close(a: f64, b: f64, prec: i32) -> bool {
        (a - b).abs() < 10f64.powi(-prec) / 2.0
    }

    const HELD_OPEN: f64 = 1000.0;

    #[test]
    fn pre_release_stages() {
        assert_eq!(
            envelope_level(&env(|e| e.delay_ms = 50.0), 0.02, HELD_OPEN),
            0.0
        );
        assert!(close(envelope_level(&env(|_| {}), 0.05, HELD_OPEN), 0.5, 9));
        assert!(close(envelope_level(&env(|_| {}), 0.1, HELD_OPEN), 1.0, 9));
        assert!(close(
            envelope_level(
                &env(|e| {
                    e.hold_ms = 100.0;
                    e.sustain = 0.5;
                }),
                0.15,
                HELD_OPEN
            ),
            1.0,
            9
        ));
        assert!(close(
            envelope_level(
                &env(|e| {
                    e.decay_ms = 100.0;
                    e.sustain = 0.4;
                }),
                0.15,
                HELD_OPEN
            ),
            0.7,
            9
        ));
        assert!(close(
            envelope_level(
                &env(|e| {
                    e.decay_ms = 100.0;
                    e.sustain = 0.3;
                }),
                5.0,
                HELD_OPEN
            ),
            0.3,
            9
        ));
    }

    #[test]
    fn release_stage() {
        let e = env(|e| e.release_ms = 100.0);
        assert!(close(envelope_level(&e, 1.0, 1.0), 1.0, 9));
        assert!(close(envelope_level(&e, 1.05, 1.0), 0.5, 9));

        let e = env(|e| {
            e.attack_ms = 200.0;
            e.release_ms = 100.0;
        });
        assert!(close(envelope_level(&e, 0.1, 0.1), 0.5, 9));
        assert!(close(envelope_level(&e, 0.15, 0.1), 0.25, 9));

        assert_eq!(
            envelope_level(&env(|e| e.release_ms = 100.0), 1.5, 1.0),
            0.0
        );
    }

    #[test]
    fn degenerate_stages() {
        assert!(close(
            envelope_level(
                &env(|e| {
                    e.attack_ms = 0.0;
                    e.hold_ms = 50.0;
                    e.sustain = 0.5;
                }),
                0.0,
                HELD_OPEN
            ),
            1.0,
            9
        ));
        assert_eq!(envelope_level(&env(|e| e.release_ms = 0.0), 0.5, 0.5), 0.0);
    }

    #[test]
    fn curve_shaping() {
        assert!(envelope_level(&env(|e| e.attack_curve = 4.0), 0.05, HELD_OPEN) < 0.5);
    }
}
