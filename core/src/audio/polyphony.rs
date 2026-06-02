//! ボイス割り当て。トラックの最大同時発音数・優先度・停止グループに従って、
//! どのノートがどれだけの長さ鳴るかを決める。

use crate::schema::{Polyphony, StopMode, VoicePriority};
use std::collections::HashMap;

/// ボイスを取り合う 1 ノート。
#[derive(Debug, Clone)]
pub struct VoiceRequest {
    pub pitch: i32,
    pub start_sec: f64,
    pub duration_sec: f64,
    pub sample_id: String,
}

/// 生き残ったボイス: 元リクエストの添字と、ゲートが開いている長さ。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoiceAllocation {
    pub index: usize,
    pub duration_sec: f64,
}

#[derive(Debug, Clone)]
struct ActiveVoice {
    index: usize,
    pitch: i32,
    start_sec: f64,
    sample_id: String,
    end_sec: f64,
}

/// JS の `a - b || a.index - b.index` 形を再現: 主キーが 0 なら副キーへ。
fn nonzero(primary: f64, secondary: f64) -> f64 {
    if primary != 0.0 {
        primary
    } else {
        secondary
    }
}

/// 値の低いボイスほど先に犠牲にする順序。戻り値 < 0 のとき a が b より犠牲向き。
fn victim_cmp(priority: VoicePriority, a: &ActiveVoice, b: &ActiveVoice) -> f64 {
    let ai = a.index as f64;
    let bi = b.index as f64;
    match priority {
        VoicePriority::Newest => nonzero(a.start_sec - b.start_sec, ai - bi),
        VoicePriority::Oldest => nonzero(b.start_sec - a.start_sec, bi - ai),
        VoicePriority::Highest => nonzero((a.pitch - b.pitch) as f64, ai - bi),
        VoicePriority::Lowest => nonzero((b.pitch - a.pitch) as f64, ai - bi),
    }
}

/// 保持中のボイスが、入ってきたノートと同じ停止グループに属するか。
fn shares_group(stop_mode: StopMode, held: &ActiveVoice, incoming: &VoiceRequest) -> bool {
    match stop_mode {
        StopMode::Pitch => held.pitch == incoming.pitch,
        StopMode::Sample => held.sample_id == incoming.sample_id,
        StopMode::Track => true,
        StopMode::None => false,
    }
}

pub fn allocate_voices(requests: &[VoiceRequest], config: &Polyphony) -> Vec<VoiceAllocation> {
    let cap = if config.max_voices > 0 {
        config.max_voices as usize
    } else {
        usize::MAX
    };

    let mut events: Vec<(usize, &VoiceRequest)> = requests.iter().enumerate().collect();
    events.sort_by(|(ai, a), (bi, b)| {
        a.start_sec
            .partial_cmp(&b.start_sec)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(ai.cmp(bi))
    });

    let mut active: Vec<ActiveVoice> = Vec::new();
    let mut durations: HashMap<usize, f64> = HashMap::new();

    for (index, event) in events {
        let t = event.start_sec;
        active.retain(|v| v.end_sec > t);

        if config.stop_mode != StopMode::None {
            let mut i = active.len();
            while i > 0 {
                i -= 1;
                if active[i].start_sec < t && shares_group(config.stop_mode, &active[i], event) {
                    durations.insert(active[i].index, t - active[i].start_sec);
                    active.remove(i);
                }
            }
        }

        active.push(ActiveVoice {
            index,
            pitch: event.pitch,
            start_sec: event.start_sec,
            sample_id: event.sample_id.clone(),
            end_sec: event.start_sec + event.duration_sec,
        });
        durations.insert(index, event.duration_sec);

        if active.len() > cap {
            let mut pick = 0;
            for i in 1..active.len() {
                if victim_cmp(config.priority, &active[i], &active[pick]) < 0.0 {
                    pick = i;
                }
            }
            let victim = active.remove(pick);
            if victim.start_sec < t {
                durations.insert(victim.index, t - victim.start_sec);
            } else {
                durations.remove(&victim.index);
            }
        }
    }

    let mut result: Vec<VoiceAllocation> = durations
        .into_iter()
        .map(|(index, duration_sec)| VoiceAllocation {
            index,
            duration_sec,
        })
        .collect();
    result.sort_by_key(|a| a.index);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(pitch: i32, start_sec: f64, duration_sec: f64, sample_id: &str) -> VoiceRequest {
        VoiceRequest {
            pitch,
            start_sec,
            duration_sec,
            sample_id: sample_id.to_string(),
        }
    }

    fn d(pitch: i32, start: f64, dur: f64) -> VoiceRequest {
        req(pitch, start, dur, "s1")
    }

    fn poly(max_voices: i32, priority: VoicePriority, stop_mode: StopMode) -> Polyphony {
        Polyphony {
            max_voices,
            priority,
            stop_mode,
        }
    }

    fn alloc(rs: &[VoiceRequest], p: Polyphony) -> Vec<(usize, f64)> {
        allocate_voices(rs, &p)
            .into_iter()
            .map(|a| (a.index, a.duration_sec))
            .collect()
    }

    const DEF: Polyphony = Polyphony {
        max_voices: 0,
        priority: VoicePriority::Newest,
        stop_mode: StopMode::None,
    };

    #[test]
    fn pass_through() {
        let rs = [d(60, 0.0, 1.0), d(60, 1.0, 1.0), d(60, 2.0, 1.0)];
        assert_eq!(alloc(&rs, DEF), vec![(0, 1.0), (1, 1.0), (2, 1.0)]);

        let rs = [d(48, 0.0, 1.0), d(72, 0.0, 1.0)];
        assert_eq!(
            allocate_voices(&rs, &DEF)
                .iter()
                .map(|a| a.index)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );

        let rs = [d(60, 2.0, 1.0), d(60, 0.0, 1.0), d(60, 1.0, 1.0)];
        assert_eq!(
            allocate_voices(&rs, &DEF)
                .iter()
                .map(|a| a.index)
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
    }

    #[test]
    fn voice_cap() {
        let rs = [d(60, 0.0, 5.0), d(60, 1.0, 5.0), d(60, 2.0, 5.0)];
        assert_eq!(
            alloc(&rs, poly(0, VoicePriority::Newest, StopMode::None)),
            vec![(0, 5.0), (1, 5.0), (2, 5.0)]
        );

        let rs = [d(60, 0.0, 5.0), d(62, 1.0, 5.0), d(64, 2.0, 5.0)];
        assert_eq!(
            alloc(&rs, poly(2, VoicePriority::Newest, StopMode::None)),
            vec![(0, 2.0), (1, 5.0), (2, 5.0)]
        );

        let rs = [d(60, 0.0, 1.0), d(60, 2.0, 1.0)];
        assert_eq!(
            alloc(&rs, poly(1, VoicePriority::Newest, StopMode::None)),
            vec![(0, 1.0), (1, 1.0)]
        );
    }

    #[test]
    fn stealing_priority() {
        let rs = [d(60, 0.0, 5.0), d(60, 1.0, 5.0), d(60, 2.0, 5.0)];
        assert_eq!(
            alloc(&rs, poly(2, VoicePriority::Oldest, StopMode::None)),
            vec![(0, 5.0), (1, 5.0)]
        );

        let rs = [d(72, 0.0, 5.0), d(60, 1.0, 5.0), d(64, 2.0, 5.0)];
        assert_eq!(
            alloc(&rs, poly(2, VoicePriority::Highest, StopMode::None)),
            vec![(0, 5.0), (1, 1.0), (2, 5.0)]
        );

        let rs = [d(60, 0.0, 5.0), d(72, 1.0, 5.0), d(64, 2.0, 5.0)];
        assert_eq!(
            alloc(&rs, poly(2, VoicePriority::Lowest, StopMode::None)),
            vec![(0, 5.0), (1, 1.0), (2, 5.0)]
        );

        let rs = [d(60, 0.0, 5.0), d(62, 0.0, 5.0), d(64, 0.0, 5.0)];
        assert_eq!(
            alloc(&rs, poly(2, VoicePriority::Newest, StopMode::None)),
            vec![(1, 5.0), (2, 5.0)]
        );
    }

    #[test]
    fn stop_mode() {
        let rs = [d(60, 0.0, 5.0), d(60, 2.0, 5.0)];
        assert_eq!(
            alloc(&rs, poly(0, VoicePriority::Newest, StopMode::None)),
            vec![(0, 5.0), (1, 5.0)]
        );
        assert_eq!(
            alloc(&rs, poly(0, VoicePriority::Newest, StopMode::Pitch)),
            vec![(0, 2.0), (1, 5.0)]
        );

        let rs = [d(60, 0.0, 5.0), d(64, 2.0, 5.0)];
        assert_eq!(
            alloc(&rs, poly(0, VoicePriority::Newest, StopMode::Pitch)),
            vec![(0, 5.0), (1, 5.0)]
        );

        let rs = [req(60, 0.0, 5.0, "a"), req(64, 2.0, 5.0, "a")];
        assert_eq!(
            alloc(&rs, poly(0, VoicePriority::Newest, StopMode::Sample)),
            vec![(0, 2.0), (1, 5.0)]
        );

        let rs = [req(60, 0.0, 5.0, "a"), req(60, 2.0, 5.0, "b")];
        assert_eq!(
            alloc(&rs, poly(0, VoicePriority::Newest, StopMode::Sample)),
            vec![(0, 5.0), (1, 5.0)]
        );

        let rs = [
            req(60, 0.0, 5.0, "a"),
            req(64, 1.0, 5.0, "b"),
            req(67, 2.0, 5.0, "c"),
        ];
        assert_eq!(
            alloc(&rs, poly(0, VoicePriority::Newest, StopMode::Track)),
            vec![(0, 1.0), (1, 1.0), (2, 5.0)]
        );

        let rs = [d(60, 0.0, 5.0), d(60, 0.0, 5.0)];
        assert_eq!(
            alloc(&rs, poly(0, VoicePriority::Newest, StopMode::Pitch)),
            vec![(0, 5.0), (1, 5.0)]
        );
    }

    #[test]
    fn deterministic_tie_breaks() {
        let rs = [d(60, 0.0, 5.0), d(62, 0.0, 5.0), d(64, 0.0, 5.0)];
        assert_eq!(
            alloc(&rs, poly(2, VoicePriority::Oldest, StopMode::None)),
            vec![(0, 5.0), (1, 5.0)]
        );

        let rs = [d(60, 0.0, 5.0), d(60, 1.0, 5.0), d(60, 2.0, 5.0)];
        assert_eq!(
            alloc(&rs, poly(2, VoicePriority::Highest, StopMode::None)),
            vec![(0, 2.0), (1, 5.0), (2, 5.0)]
        );
        assert_eq!(
            alloc(&rs, poly(2, VoicePriority::Lowest, StopMode::None)),
            vec![(0, 2.0), (1, 5.0), (2, 5.0)]
        );
    }

    #[test]
    fn empty_requests_allocate_nothing() {
        assert!(allocate_voices(&[], &DEF).is_empty());
    }

    #[test]
    fn single_request_passes_through() {
        let rs = [d(60, 0.0, 1.5)];
        assert_eq!(alloc(&rs, DEF), vec![(0, 1.5)]);
        assert_eq!(
            alloc(&rs, poly(8, VoicePriority::Newest, StopMode::Pitch)),
            vec![(0, 1.5)]
        );
    }

    #[test]
    fn cap_above_demand_never_steals() {
        let rs = [d(60, 0.0, 5.0), d(62, 0.1, 5.0), d(64, 0.2, 5.0)];
        assert_eq!(
            alloc(&rs, poly(8, VoicePriority::Newest, StopMode::None)),
            vec![(0, 5.0), (1, 5.0), (2, 5.0)]
        );
    }

    #[test]
    fn non_overlapping_notes_reuse_one_voice() {
        // 重ならないノートは 1 ボイスでも全部鳴る。
        let rs = [d(60, 0.0, 0.5), d(62, 1.0, 0.5), d(64, 2.0, 0.5)];
        assert_eq!(
            alloc(&rs, poly(1, VoicePriority::Newest, StopMode::None)),
            vec![(0, 0.5), (1, 0.5), (2, 0.5)]
        );
    }

    #[test]
    fn track_stop_mode_chokes_any_overlap() {
        // StopMode::Track は音程・素材に関係なく直前のボイスを止める。
        let rs = [d(60, 0.0, 5.0), d(67, 1.0, 5.0)];
        assert_eq!(
            alloc(&rs, poly(0, VoicePriority::Newest, StopMode::Track)),
            vec![(0, 1.0), (1, 5.0)]
        );
    }
}
