//! 表示用のフォーマットヘルパー（renderer/util/format.ts の移植）。

pub fn format_time(seconds: f64) -> String {
    let s = if !seconds.is_finite() || seconds < 0.0 {
        0.0
    } else {
        seconds
    };
    let minutes = (s / 60.0).floor() as i64;
    let secs = (s % 60.0).floor() as i64;
    let millis = ((s - s.floor()) * 1000.0).floor() as i64;
    format!("{minutes}:{secs:02}.{millis:03}")
}

pub fn format_db(linear: f64) -> String {
    if linear <= 0.00001 {
        return "-∞".to_string();
    }
    let db = 20.0 * linear.log10();
    let sign = if db >= 0.0 { "+" } else { "" };
    format!("{sign}{db:.1} dB")
}

pub fn pct(value: f64) -> String {
    format!("{}%", (value * 100.0).round() as i64)
}

pub fn format_rate(hz: i32) -> String {
    if hz % 1000 == 0 {
        format!("{} kHz", hz / 1000)
    } else {
        format!("{:.1} kHz", hz as f64 / 1000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_time_basic() {
        assert_eq!(format_time(0.0), "0:00.000");
        assert_eq!(format_time(1.5), "0:01.500");
        assert_eq!(format_time(65.25), "1:05.250");
        assert_eq!(format_time(75.0), "1:15.000");
        assert_eq!(format_time(600.0), "10:00.000");
    }

    #[test]
    fn format_time_clamps_invalid() {
        assert_eq!(format_time(-1.0), "0:00.000");
        assert_eq!(format_time(f64::NAN), "0:00.000");
        assert_eq!(format_time(f64::INFINITY), "0:00.000");
    }

    #[test]
    fn format_db_levels() {
        assert_eq!(format_db(1.0), "+0.0 dB");
        assert_eq!(format_db(0.5), "-6.0 dB");
        assert_eq!(format_db(2.0), "+6.0 dB");
        assert_eq!(format_db(0.0), "-∞");
        assert_eq!(format_db(0.000001), "-∞");
    }

    #[test]
    fn pct_rounds_to_integer() {
        assert_eq!(pct(0.0), "0%");
        assert_eq!(pct(0.5), "50%");
        assert_eq!(pct(1.0), "100%");
        assert_eq!(pct(0.123), "12%");
        assert_eq!(pct(0.128), "13%");
    }

    #[test]
    fn format_rate_whole_and_fractional_khz() {
        assert_eq!(format_rate(48000), "48 kHz");
        assert_eq!(format_rate(96000), "96 kHz");
        assert_eq!(format_rate(16000), "16 kHz");
        assert_eq!(format_rate(44100), "44.1 kHz");
    }
}
