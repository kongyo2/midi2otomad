use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static COUNTER: AtomicU64 = AtomicU64::new(0);

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

    #[test]
    fn suffix_is_16_lowercase_hex_digits() {
        let id = make_id("sample");
        let suffix = id.strip_prefix("sample_").expect("prefix");
        assert_eq!(suffix.len(), 16);
        assert!(suffix
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)));
    }

    #[test]
    fn handles_empty_prefix() {
        let id = make_id("");
        assert!(id.starts_with('_'));
        assert_eq!(id.len(), 17);
    }

    #[test]
    fn many_ids_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for _ in 0..10_000 {
            assert!(seen.insert(make_id("track")), "duplicate id generated");
        }
    }

    #[test]
    fn splitmix64_is_deterministic_and_mixes() {
        assert_eq!(splitmix64(12345), splitmix64(12345));
        assert_ne!(splitmix64(0), splitmix64(1));
        assert_ne!(splitmix64(0), 0);
    }
}
