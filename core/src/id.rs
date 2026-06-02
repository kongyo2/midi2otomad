//! 安定した一意 ID の生成。`prefix_<16桁の hex>` という形を返す。

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// `prefix` を前置した一意な識別子を返す。エントロピー源を持たない環境でも
/// 衝突しないよう、起動時刻・単調増加カウンタ・アドレス由来の値を混ぜる。
pub fn make_id(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mixed = splitmix64(nanos ^ count.rotate_left(32) ^ (&COUNTER as *const _ as u64));
    format!("{prefix}_{mixed:016x}")
}

fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_prefix() {
        assert!(make_id("sample").starts_with("sample_"));
    }

    #[test]
    fn generates_distinct_ids() {
        let a = make_id("id");
        let b = make_id("id");
        assert_ne!(a, b);
    }
}
