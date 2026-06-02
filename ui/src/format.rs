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
