use midi2otomad_core::schema::{
    FilterType, InterpolationMode, LfoShape, PitchMode, StopMode, VoicePriority,
};

pub trait SelectValue: Copy + Default {
    fn as_value(self) -> &'static str;
    fn from_value(v: &str) -> Self;
}

macro_rules! select_value {
    ($ty:ty { $($variant:ident => $value:literal),+ $(,)? }) => {
        impl SelectValue for $ty {
            fn as_value(self) -> &'static str {
                match self {
                    $(<$ty>::$variant => $value),+
                }
            }
            fn from_value(v: &str) -> Self {
                match v {
                    $($value => <$ty>::$variant,)+
                    _ => Self::default(),
                }
            }
        }
    };
}

select_value!(InterpolationMode {
    Hermite => "hermite",
    Linear => "linear",
    Sinc => "sinc",
});

select_value!(PitchMode {
    Resample => "resample",
    Granular => "granular",
});

select_value!(FilterType {
    Lowpass => "lowpass",
    Highpass => "highpass",
    Bandpass => "bandpass",
    Notch => "notch",
    Peaking => "peaking",
    Lowshelf => "lowshelf",
    Highshelf => "highshelf",
    Allpass => "allpass",
});

select_value!(LfoShape {
    Sine => "sine",
    Triangle => "triangle",
    Square => "square",
    Saw => "saw",
});

select_value!(VoicePriority {
    Newest => "newest",
    Oldest => "oldest",
    Highest => "highest",
    Lowest => "lowest",
});

select_value!(StopMode {
    None => "none",
    Pitch => "pitch",
    Sample => "sample",
    Track => "track",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolation_round_trips_and_defaults() {
        assert_eq!(InterpolationMode::Hermite.as_value(), "hermite");
        assert_eq!(InterpolationMode::Linear.as_value(), "linear");
        assert_eq!(
            InterpolationMode::from_value("linear"),
            InterpolationMode::Linear
        );
        assert_eq!(
            InterpolationMode::from_value("hermite"),
            InterpolationMode::Hermite
        );
        assert_eq!(InterpolationMode::Sinc.as_value(), "sinc");
        assert_eq!(
            InterpolationMode::from_value("sinc"),
            InterpolationMode::Sinc
        );
        assert_eq!(
            InterpolationMode::from_value("???"),
            InterpolationMode::default()
        );
    }

    #[test]
    fn pitch_mode_round_trips_and_defaults() {
        for m in [PitchMode::Resample, PitchMode::Granular] {
            assert_eq!(PitchMode::from_value(m.as_value()), m);
        }
        assert_eq!(PitchMode::Resample.as_value(), "resample");
        assert_eq!(PitchMode::Granular.as_value(), "granular");
        assert_eq!(PitchMode::from_value("tape"), PitchMode::default());
    }

    #[test]
    fn filter_type_round_trips_and_defaults() {
        for kind in [
            FilterType::Lowpass,
            FilterType::Highpass,
            FilterType::Bandpass,
            FilterType::Notch,
            FilterType::Peaking,
            FilterType::Lowshelf,
            FilterType::Highshelf,
            FilterType::Allpass,
        ] {
            assert_eq!(FilterType::from_value(kind.as_value()), kind);
        }
        assert_eq!(FilterType::from_value("comb"), FilterType::default());
    }

    #[test]
    fn lfo_shape_round_trips_and_defaults() {
        for shape in [
            LfoShape::Sine,
            LfoShape::Triangle,
            LfoShape::Square,
            LfoShape::Saw,
        ] {
            assert_eq!(LfoShape::from_value(shape.as_value()), shape);
        }
        assert_eq!(LfoShape::from_value("ramp"), LfoShape::default());
    }

    #[test]
    fn voice_priority_round_trips_and_defaults() {
        for p in [
            VoicePriority::Newest,
            VoicePriority::Oldest,
            VoicePriority::Highest,
            VoicePriority::Lowest,
        ] {
            assert_eq!(VoicePriority::from_value(p.as_value()), p);
        }
        assert_eq!(
            VoicePriority::from_value("loudest"),
            VoicePriority::default()
        );
    }

    #[test]
    fn stop_mode_round_trips_and_defaults() {
        for m in [
            StopMode::None,
            StopMode::Pitch,
            StopMode::Sample,
            StopMode::Track,
        ] {
            assert_eq!(StopMode::from_value(m.as_value()), m);
        }
        assert_eq!(StopMode::from_value("channel"), StopMode::default());
    }
}
